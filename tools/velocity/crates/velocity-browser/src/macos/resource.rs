use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use velocity_metrics::{
    AttributionEvidence, AttributionStatus, ManagedProcessTarget, ProcessAttributionDiagnostics,
    ProcessExclusionReason, ProcessLifecycleChange, ProcessLifecycleEvent, ProcessRegistrySnapshot,
    ProcessResource, ProcessResourceSnapshot, ProcessStartTime, ResourceCollectionError,
    ResourceCollector, ResourceErrorKind,
};

use super::process_info::{
    all_process_ids, bsd_info, child_processes, cpu_time_nanoseconds,
    has_exact_user_data_directory, process_arguments, process_path, process_session_id,
    process_usage, user_data_directory_argument,
};

const MAX_VERIFIED_PROCESSES: usize = 4_096;
const PROCESS_TABLE_SCAN_INTERVAL: Duration = Duration::from_millis(500);

enum ChromiumProfileAttribution {
    Verified { path: PathBuf, role: String },
    DifferentProfile,
    NotSelectedApplication,
    Uncertain(String),
}

#[derive(Clone, Debug)]
struct KnownProcess {
    start_time: ProcessStartTime,
    evidence: AttributionEvidence,
    executable_path: Option<PathBuf>,
    role: Option<String>,
}

/// Collects only the managed macOS session and Chromium helpers independently
/// verified by the benchmark's private profile argument and application bundle.
/// It never signals a process or attributes a process by name alone.
#[derive(Clone, Debug)]
pub struct MacOSResourceCollector {
    target: ManagedProcessTarget,
    known_processes: BTreeMap<u32, KnownProcess>,
    last_process_table_scan: Option<Instant>,
    photon_registry: Option<ProcessRegistrySnapshot>,
    photon_registry_error: Option<String>,
}

impl MacOSResourceCollector {
    pub fn new(target: ManagedProcessTarget) -> Self {
        Self {
            target,
            known_processes: BTreeMap::new(),
            last_process_table_scan: None,
            photon_registry: None,
            photon_registry_error: None,
        }
    }

    pub fn update_photon_process_registry(
        &mut self,
        registry: Option<ProcessRegistrySnapshot>,
        error: Option<String>,
    ) {
        self.photon_registry = registry;
        self.photon_registry_error = error;
    }
}

impl ResourceCollector for MacOSResourceCollector {
    fn collect(
        &mut self,
        target: &ManagedProcessTarget,
    ) -> Result<ProcessResourceSnapshot, ResourceCollectionError> {
        if target.root_pid != self.target.root_pid
            || target.session_id != self.target.session_id
            || target.root_start_time != self.target.root_start_time
            || target.executable_path != self.target.executable_path
        {
            return Err(ResourceCollectionError::new(
                "collector cannot switch away from its managed session identity",
            ));
        }

        verify_root(target)?;
        let mut diagnostics = ProcessAttributionDiagnostics::default();
        let mut warnings = Vec::new();
        let mut process_roles = BTreeMap::<u32, String>::new();
        let mut discovered = BTreeMap::<u32, (libc::proc_bsdinfo, AttributionEvidence)>::new();

        let root_info = bsd_info(target.root_pid).ok_or_else(|| {
            ResourceCollectionError::with_kind(
                ResourceErrorKind::ProcessUnavailable,
                "managed browser process disappeared",
            )
        })?;
        if !process_identity_matches(&root_info, target.root_pid, target.root_start_time) {
            return Err(ResourceCollectionError::with_kind(
                ResourceErrorKind::ProcessIdentityMismatch,
                "managed browser PID changed identity during resource collection",
            ));
        }
        discovered.insert(
            target.root_pid,
            (root_info, AttributionEvidence::ManagedRootIdentity),
        );

        // Walk ancestry every sample so newly spawned in-session children are
        // visible immediately; the process table scan below also catches
        // reparented members that remain in Velocity's private session.
        let mut pending = vec![target.root_pid];
        let mut visited = BTreeSet::from([target.root_pid]);
        while let Some(parent_pid) = pending.pop() {
            let Some(expected_parent_start) = discovered
                .get(&parent_pid)
                .map(|(info, _)| start_time_of(info))
            else {
                diagnostics.uncertain_processes += 1;
                warnings.push(format!(
                    "parent PID {parent_pid} had no verified process identity during traversal"
                ));
                continue;
            };
            let Some(parent_info) = bsd_info(parent_pid) else {
                diagnostics.exited_processes += 1;
                record_exclusion(
                    &mut diagnostics,
                    ProcessExclusionReason::ExitedDuringDiscovery,
                );
                continue;
            };
            if !process_identity_matches(&parent_info, parent_pid, expected_parent_start) {
                diagnostics.identity_verification_failures += 1;
                diagnostics.uncertain_processes += 1;
                record_exclusion(&mut diagnostics, ProcessExclusionReason::IdentityChanged);
                warnings.push(format!(
                    "parent PID {parent_pid} changed identity during process-tree traversal"
                ));
                continue;
            }
            let children = match child_processes(parent_pid) {
                Ok(children) => children,
                Err(error) => {
                    diagnostics.uncertain_processes += 1;
                    warnings.push(error);
                    continue;
                }
            };
            for pid in children {
                if !visited.insert(pid) {
                    continue;
                }
                if discovered.len() >= MAX_VERIFIED_PROCESSES {
                    diagnostics.excluded_process_candidates += 1;
                    record_exclusion(
                        &mut diagnostics,
                        ProcessExclusionReason::ProcessLimitExceeded,
                    );
                    warnings.push(format!(
                        "managed process tree exceeded the safety limit of {MAX_VERIFIED_PROCESSES} processes"
                    ));
                    pending.clear();
                    break;
                }
                let Some(info) = bsd_info(pid) else {
                    diagnostics.exited_processes += 1;
                    diagnostics.excluded_process_candidates += 1;
                    record_exclusion(
                        &mut diagnostics,
                        ProcessExclusionReason::ExitedDuringDiscovery,
                    );
                    warnings.push(format!("child PID {pid} exited during discovery"));
                    continue;
                };
                if info.pbi_status == libc::SZOMB {
                    diagnostics.exited_processes += 1;
                    record_exclusion(
                        &mut diagnostics,
                        ProcessExclusionReason::ExitedDuringDiscovery,
                    );
                    continue;
                }
                if info.pbi_pid != pid || info.pbi_ppid != parent_pid {
                    diagnostics.identity_verification_failures += 1;
                    diagnostics.excluded_process_candidates += 1;
                    diagnostics.uncertain_processes += 1;
                    record_exclusion(&mut diagnostics, ProcessExclusionReason::IdentityChanged);
                    warnings.push(format!(
                        "child PID {pid} changed identity or ancestry during discovery and was excluded"
                    ));
                    continue;
                }
                if !belongs_to_private_session(pid, info.pbi_uid, target) {
                    diagnostics.identity_verification_failures += 1;
                    diagnostics.excluded_process_candidates += 1;
                    diagnostics.uncertain_processes += 1;
                    record_exclusion(&mut diagnostics, ProcessExclusionReason::SessionMismatch);
                    warnings.push(format!(
                        "child PID {pid} did not remain in the managed user's private process session and was excluded"
                    ));
                    continue;
                }
                discovered.insert(pid, (info, AttributionEvidence::AncestryAndPrivateSession));
                pending.push(pid);
            }
        }

        let should_scan_process_table = self
            .last_process_table_scan
            .is_none_or(|last| last.elapsed() >= PROCESS_TABLE_SCAN_INTERVAL);
        if should_scan_process_table {
            match all_process_ids() {
                Ok(pids) => {
                    for pid in pids {
                        if discovered.contains_key(&pid) {
                            continue;
                        }
                        let Some(info) = bsd_info(pid) else {
                            continue;
                        };
                        if info.pbi_status == libc::SZOMB {
                            continue;
                        }
                        if belongs_to_private_session(pid, info.pbi_uid, target) {
                            discovered
                                .insert(pid, (info, AttributionEvidence::PrivateSessionMembership));
                            continue;
                        }
                        if !target.allow_profile_argument_attribution {
                            continue;
                        }
                        if info.pbi_uid != unsafe { libc::geteuid() } {
                            continue;
                        }
                        let Some(path) = process_path(pid)
                            .filter(|path| is_within_application(path, &target.application_path))
                        else {
                            continue;
                        };
                        match classify_chromium_profile_process(pid, &info, target, path) {
                            ChromiumProfileAttribution::Verified { path, role } => {
                                discovered.insert(
                                    pid,
                                    (info, AttributionEvidence::ChromiumProfileArgumentAndBundle),
                                );
                                self.known_processes.insert(
                                    pid,
                                    KnownProcess {
                                        start_time: start_time_of(&info),
                                        evidence:
                                            AttributionEvidence::ChromiumProfileArgumentAndBundle,
                                        executable_path: Some(path),
                                        role: Some(role.clone()),
                                    },
                                );
                                process_roles.insert(pid, role);
                            }
                            ChromiumProfileAttribution::DifferentProfile => {
                                diagnostics.excluded_process_candidates += 1;
                                record_exclusion(
                                    &mut diagnostics,
                                    ProcessExclusionReason::DifferentIsolatedProfile,
                                );
                                warnings.push(format!(
                                    "browser-bundle candidate PID {pid} was excluded because it explicitly uses a different browser profile"
                                ));
                            }
                            ChromiumProfileAttribution::NotSelectedApplication => {}
                            ChromiumProfileAttribution::Uncertain(error) => {
                                diagnostics.excluded_process_candidates += 1;
                                diagnostics.uncertain_processes += 1;
                                record_exclusion(
                                    &mut diagnostics,
                                    ProcessExclusionReason::OwnershipEvidenceUnavailable,
                                );
                                warnings.push(error);
                            }
                        }
                    }
                    self.last_process_table_scan = Some(Instant::now());
                }
                Err(error) => {
                    diagnostics.uncertain_processes += 1;
                    warnings.push(error);
                }
            }
        }

        if target.allow_profile_argument_attribution {
            for &pid in discovered.keys() {
                if pid == target.root_pid {
                    process_roles.insert(pid, "browser".to_owned());
                    continue;
                }
                let Some(_path) = process_path(pid)
                    .filter(|path| is_within_application(path, &target.application_path))
                else {
                    continue;
                };
                if let Ok(arguments) = process_arguments(pid) {
                    let role = chromium_process_role(&arguments);
                    process_roles.insert(pid, role);
                } else {
                    warnings.push(format!(
                        "verified Chromium process PID {pid} has unavailable role arguments; process remains included"
                    ));
                }
            }
        }

        let mut registry_verified_pids = BTreeSet::new();
        let mut registry_entries_complete = false;
        if let Some(registry) = &self.photon_registry {
            diagnostics.registry_scope = Some(registry.scope.clone());
            diagnostics.registry_source = Some(registry.source.clone());
            diagnostics.registry_snapshot_complete = registry.complete;
            diagnostics.registered_processes = registry.processes.len();
            if registry.processes.len() > MAX_VERIFIED_PROCESSES {
                diagnostics.excluded_process_candidates += registry.processes.len();
                diagnostics.uncertain_processes += 1;
                record_exclusion(
                    &mut diagnostics,
                    ProcessExclusionReason::ProcessLimitExceeded,
                );
                warnings.push(format!(
                    "Photon registry exceeded the process limit of {MAX_VERIFIED_PROCESSES}; its snapshot was rejected"
                ));
            }
            if !registry.complete {
                warnings.push(format!(
                    "Photon process registry did not establish a complete snapshot for scope '{}'",
                    registry.scope
                ));
            }

            let mut registered_pids = BTreeSet::new();
            let registered_start_times = registry
                .processes
                .iter()
                .filter_map(|process| process.start_time.map(|start| (process.pid, start)))
                .collect::<BTreeMap<_, _>>();
            let mut entries_valid = registry.processes.len() <= MAX_VERIFIED_PROCESSES;
            let verification_limit = if registry.processes.len() > MAX_VERIFIED_PROCESSES {
                0
            } else {
                MAX_VERIFIED_PROCESSES
            };
            for registration in registry.processes.iter().take(verification_limit) {
                if registration.pid == 0 || registration.role.is_empty() {
                    diagnostics.excluded_process_candidates += 1;
                    diagnostics.uncertain_processes += 1;
                    entries_valid = false;
                    record_exclusion(
                        &mut diagnostics,
                        ProcessExclusionReason::RegistryIdentityMismatch,
                    );
                    warnings
                        .push("Photon process registry contained an invalid identity".to_owned());
                    continue;
                }
                if registration.role.len() > 64 || !known_photon_process_role(&registration.role) {
                    diagnostics.excluded_process_candidates += 1;
                    diagnostics.uncertain_processes += 1;
                    entries_valid = false;
                    record_exclusion(
                        &mut diagnostics,
                        ProcessExclusionReason::RegistryIdentityMismatch,
                    );
                    warnings.push(format!(
                        "Photon registry supplied an unsupported process role for PID {}",
                        registration.pid
                    ));
                    continue;
                }
                if !registered_pids.insert(registration.pid) {
                    diagnostics.excluded_process_candidates += 1;
                    diagnostics.uncertain_processes += 1;
                    entries_valid = false;
                    record_exclusion(
                        &mut diagnostics,
                        ProcessExclusionReason::DuplicateRegistration,
                    );
                    warnings.push(format!(
                        "Photon process registry reported PID {} more than once",
                        registration.pid
                    ));
                    continue;
                }

                let Some(info) = bsd_info(registration.pid) else {
                    diagnostics.exited_processes += 1;
                    diagnostics.excluded_process_candidates += 1;
                    entries_valid = false;
                    record_exclusion(
                        &mut diagnostics,
                        ProcessExclusionReason::ExitedDuringDiscovery,
                    );
                    warnings.push(format!(
                        "Photon-registered PID {} exited before independent verification",
                        registration.pid
                    ));
                    continue;
                };
                if info.pbi_status == libc::SZOMB
                    || !registered_process_identity_matches(
                        registration,
                        info.pbi_pid,
                        start_time_of(&info),
                    )
                    || (registration.pid == target.root_pid
                        && start_time_of(&info) != target.root_start_time)
                {
                    diagnostics.identity_verification_failures += 1;
                    diagnostics.excluded_process_candidates += 1;
                    diagnostics.uncertain_processes += 1;
                    entries_valid = false;
                    record_exclusion(
                        &mut diagnostics,
                        ProcessExclusionReason::RegistryIdentityMismatch,
                    );
                    warnings.push(format!(
                        "Photon-registered PID {} failed independent PID/start-time verification",
                        registration.pid
                    ));
                    continue;
                }
                if !belongs_to_private_session(registration.pid, info.pbi_uid, target) {
                    diagnostics.identity_verification_failures += 1;
                    diagnostics.excluded_process_candidates += 1;
                    diagnostics.uncertain_processes += 1;
                    entries_valid = false;
                    record_exclusion(&mut diagnostics, ProcessExclusionReason::SessionMismatch);
                    warnings.push(format!(
                        "Photon-registered PID {} is outside the managed private process session and was excluded",
                        registration.pid
                    ));
                    continue;
                }
                let Some(executable_path) = process_path(registration.pid) else {
                    diagnostics.identity_verification_failures += 1;
                    diagnostics.excluded_process_candidates += 1;
                    diagnostics.uncertain_processes += 1;
                    entries_valid = false;
                    record_exclusion(&mut diagnostics, ProcessExclusionReason::ExecutableMismatch);
                    warnings.push(format!(
                        "executable identity for Photon-registered PID {} is unavailable",
                        registration.pid
                    ));
                    continue;
                };
                let parent_start_time = registration.parent_pid.and_then(|parent_pid| {
                    bsd_info(parent_pid)
                        .filter(|parent| {
                            parent.pbi_status != libc::SZOMB && parent.pbi_pid == parent_pid
                        })
                        .map(|parent| start_time_of(&parent))
                });
                if !registered_parent_identity_matches(
                    registration,
                    target.root_pid,
                    info.pbi_ppid,
                    parent_start_time,
                    &registered_start_times,
                    target.root_start_time,
                ) {
                    diagnostics.uncertain_processes += 1;
                    diagnostics.identity_verification_failures += 1;
                    entries_valid = false;
                    record_exclusion(
                        &mut diagnostics,
                        ProcessExclusionReason::RegistryIdentityMismatch,
                    );
                    warnings.push(format!(
                        "Photon-registered PID {} has an unavailable or mismatched parent process identity",
                        registration.pid
                    ));
                }
                if registration.pid == target.root_pid
                    && !same_executable_path(&executable_path, &target.executable_path)
                {
                    diagnostics.identity_verification_failures += 1;
                    diagnostics.excluded_process_candidates += 1;
                    diagnostics.uncertain_processes += 1;
                    entries_valid = false;
                    record_exclusion(&mut diagnostics, ProcessExclusionReason::ExecutableMismatch);
                    warnings.push(
                        "Photon process registry root executable does not match the managed installation".to_owned(),
                    );
                    continue;
                }

                discovered.insert(
                    registration.pid,
                    (
                        info,
                        AttributionEvidence::PhotonEngineRegistryParentIdentityAndPrivateSession,
                    ),
                );
                process_roles.insert(registration.pid, registration.role.clone());
                registry_verified_pids.insert(registration.pid);
                diagnostics.independently_verified_registered_processes += 1;
            }
            let root_registered = registered_pids.contains(&target.root_pid)
                && registry_verified_pids.contains(&target.root_pid);
            let unregistered_session_processes = discovered
                .iter()
                .filter(|(pid, (info, _))| {
                    **pid != target.root_pid
                        && belongs_to_private_session(**pid, info.pbi_uid, target)
                        && !registered_pids.contains(pid)
                })
                .count();
            if unregistered_session_processes > 0 {
                diagnostics.uncertain_processes += unregistered_session_processes;
                entries_valid = false;
                warnings.push(format!(
                    "{} managed-session process(es) were not present in Photon::ProcessManager's snapshot",
                    unregistered_session_processes
                ));
            }
            if !root_registered {
                diagnostics.uncertain_processes += 1;
                entries_valid = false;
                warnings.push(
                    "Photon process registry did not include the independently verified application process".to_owned(),
                );
            }
            registry_entries_complete = registry.complete && entries_valid && root_registered;
        } else if let Some(error) = self.photon_registry_error.as_ref() {
            diagnostics.registry_source = Some("Photon Automation Protocol".to_owned());
            diagnostics.uncertain_processes += 1;
            warnings.push(format!("Photon process registry query failed: {error}"));
        }

        // Retain only previously verified identities whose start time and
        // ownership evidence still match. This handles a child reparented to
        // launchd without carrying counters across PID reuse or exec changes.
        for (&pid, known) in &self.known_processes {
            if discovered.contains_key(&pid) {
                continue;
            }
            let Some(info) = bsd_info(pid) else {
                diagnostics.exited_processes += 1;
                record_exclusion(
                    &mut diagnostics,
                    ProcessExclusionReason::ExitedDuringDiscovery,
                );
                continue;
            };
            if info.pbi_status == libc::SZOMB {
                diagnostics.exited_processes += 1;
                record_exclusion(
                    &mut diagnostics,
                    ProcessExclusionReason::ExitedDuringDiscovery,
                );
                continue;
            }
            if info.pbi_pid != pid || start_time_of(&info) != known.start_time {
                diagnostics.identity_verification_failures += 1;
                diagnostics.excluded_process_candidates += 1;
                diagnostics.uncertain_processes += 1;
                record_exclusion(&mut diagnostics, ProcessExclusionReason::IdentityChanged);
                warnings.push(format!(
                    "PID {pid} was reused or changed process identity; the new process was excluded"
                ));
                continue;
            }
            let retained_evidence = match known.evidence {
                AttributionEvidence::ChromiumProfileArgumentAndBundle => process_path(pid)
                    .filter(|path| {
                        known
                            .executable_path
                            .as_ref()
                            .is_some_and(|expected| same_executable_path(path, expected))
                    })
                    .map(|_| AttributionEvidence::ChromiumProfileArgumentAndBundle),
                evidence => retained_session_evidence(
                    evidence,
                    belongs_to_private_session(pid, info.pbi_uid, target),
                ),
            };
            if let Some(evidence) = retained_evidence {
                if let Some(role) = &known.role {
                    process_roles.insert(pid, role.clone());
                }
                discovered.insert(pid, (info, evidence));
            } else {
                diagnostics.identity_verification_failures += 1;
                diagnostics.excluded_process_candidates += 1;
                diagnostics.uncertain_processes += 1;
                record_exclusion(
                    &mut diagnostics,
                    ProcessExclusionReason::OwnershipEvidenceUnavailable,
                );
                warnings.push(format!(
                    "previously verified PID {pid} no longer satisfies its ownership evidence and was excluded"
                ));
            }
        }

        if discovered.len() > MAX_VERIFIED_PROCESSES {
            diagnostics.excluded_process_candidates += discovered.len() - MAX_VERIFIED_PROCESSES;
            let excess = discovered.len() - MAX_VERIFIED_PROCESSES;
            *diagnostics
                .exclusion_reasons
                .entry(ProcessExclusionReason::ProcessLimitExceeded)
                .or_default() += excess;
            diagnostics.uncertain_processes += excess;
            discovered.retain(|pid, _| *pid == target.root_pid);
            warnings.push(format!(
                "verified process set exceeded the safety limit of {MAX_VERIFIED_PROCESSES}; non-root processes were excluded"
            ));
        }

        match process_path(target.root_pid) {
            Some(path) if !same_executable_path(&path, &target.executable_path) => {
                warnings.push(format!(
                    "managed root executable path changed from {} to {}; PID, precise start time, and private session still match",
                    target.executable_path.display(), path.display()
                ));
            }
            None => warnings.push(format!(
                "executable path unavailable for verified root PID {}",
                target.root_pid
            )),
            Some(_) => {}
        }

        let mut current_known = BTreeMap::new();
        let mut processes = Vec::with_capacity(discovered.len());
        for (&pid, (info, evidence)) in &discovered {
            let start_time = start_time_of(info);
            let executable_path = process_path(pid);
            current_known.insert(
                pid,
                KnownProcess {
                    start_time,
                    evidence: *evidence,
                    executable_path: executable_path.clone(),
                    role: process_roles.get(&pid).cloned(),
                },
            );
            *diagnostics.evidence.entry(*evidence).or_default() += 1;
            let parent_start_time = (info.pbi_ppid > 0)
                .then(|| {
                    discovered
                        .get(&info.pbi_ppid)
                        .map(|(parent, _)| start_time_of(parent))
                })
                .flatten();
            let usage = process_usage(pid);
            let (cpu_time_ns, physical_footprint_bytes, resident_bytes) = match usage {
                Some(usage) => {
                    let cpu_time_ns = cpu_time_nanoseconds(&usage);
                    if cpu_time_ns.is_none() {
                        warnings.push(format!(
                            "CPU counter or Mach timebase unavailable for verified PID {pid}"
                        ));
                    }
                    (
                        cpu_time_ns,
                        Some(usage.ri_phys_footprint),
                        Some(usage.ri_resident_size),
                    )
                }
                None => {
                    warnings.push(format!(
                        "libproc resource counters unavailable for verified PID {pid}"
                    ));
                    (None, None, None)
                }
            };
            processes.push(ProcessResource {
                pid,
                start_time,
                parent_pid: (info.pbi_ppid > 0).then_some(info.pbi_ppid),
                parent_start_time,
                executable_path,
                process_group_id: Some(info.pbi_pgid),
                process_session_id: process_session_id(pid),
                attribution_evidence: Some(*evidence),
                role: process_roles.get(&pid).cloned(),
                cpu_time_ns,
                physical_footprint_bytes,
                resident_bytes,
            });
        }
        for (&pid, previous) in &self.known_processes {
            match current_known.get(&pid) {
                Some(current) if current.start_time != previous.start_time => {
                    diagnostics.lifecycle_events.push(ProcessLifecycleEvent {
                        change: ProcessLifecycleChange::Replaced,
                        pid,
                        start_time: current.start_time,
                        role: current.role.clone(),
                    });
                }
                None => diagnostics.lifecycle_events.push(ProcessLifecycleEvent {
                    change: ProcessLifecycleChange::Exited,
                    pid,
                    start_time: previous.start_time,
                    role: previous.role.clone(),
                }),
                _ => {}
            }
        }
        for (&pid, current) in &current_known {
            if !self.known_processes.contains_key(&pid) {
                diagnostics.lifecycle_events.push(ProcessLifecycleEvent {
                    change: ProcessLifecycleChange::Started,
                    pid,
                    start_time: current.start_time,
                    role: current.role.clone(),
                });
            }
        }
        diagnostics.lifecycle_events.sort();
        self.known_processes = current_known;
        processes.sort_by_key(|process| process.pid);
        diagnostics.verified_processes = processes.len();
        diagnostics.registry_snapshot_complete = registry_entries_complete;
        diagnostics.status = attribution_status(&diagnostics);
        if diagnostics.status != AttributionStatus::VerifiedComplete {
            warnings.push(
                "process-set completeness is not established for the collected browser scope"
                    .to_owned(),
            );
        }
        diagnostics.warnings = warnings.clone();
        warnings.extend(diagnostics.warnings.iter().cloned());
        warnings.sort();
        warnings.dedup();
        Ok(ProcessResourceSnapshot {
            processes,
            warnings,
            attribution: diagnostics,
        })
    }
}

fn verify_root(target: &ManagedProcessTarget) -> Result<(), ResourceCollectionError> {
    let info = bsd_info(target.root_pid).ok_or_else(|| {
        ResourceCollectionError::with_kind(
            ResourceErrorKind::ProcessUnavailable,
            "managed browser process is no longer available",
        )
    })?;
    if info.pbi_status == libc::SZOMB {
        return Err(ResourceCollectionError::with_kind(
            ResourceErrorKind::ProcessUnavailable,
            format!("managed browser process {} has exited", target.root_pid),
        ));
    }
    if !process_identity_matches(&info, target.root_pid, target.root_start_time) {
        return Err(ResourceCollectionError::with_kind(
            ResourceErrorKind::ProcessIdentityMismatch,
            format!(
                "managed PID {} no longer matches its recorded process start identity",
                target.root_pid
            ),
        ));
    }
    if !belongs_to_private_session(target.root_pid, info.pbi_uid, target) {
        return Err(ResourceCollectionError::with_kind(
            ResourceErrorKind::SessionMismatch,
            format!(
                "managed PID {} is no longer in private session {}; refusing to sample it",
                target.root_pid, target.session_id
            ),
        ));
    }
    Ok(())
}

fn process_identity_matches(
    info: &libc::proc_bsdinfo,
    pid: u32,
    start_time: ProcessStartTime,
) -> bool {
    info.pbi_pid == pid && start_time_of(info) == start_time
}

fn registered_process_identity_matches(
    registration: &velocity_metrics::RegisteredProcess,
    pid: u32,
    start_time: ProcessStartTime,
) -> bool {
    let Some(generation) = start_time
        .seconds
        .checked_mul(1_000_000)
        .and_then(|seconds| seconds.checked_add(start_time.microseconds))
        .map(|value| value.to_string())
    else {
        return false;
    };
    registration.pid == pid
        && registration.start_time == Some(start_time)
        && registration.launch_generation.as_deref() == Some(generation.as_str())
}

fn registered_parent_identity_matches(
    registration: &velocity_metrics::RegisteredProcess,
    root_pid: u32,
    current_parent_pid: u32,
    current_parent_start_time: Option<ProcessStartTime>,
    registered_start_times: &BTreeMap<u32, ProcessStartTime>,
    root_start_time: ProcessStartTime,
) -> bool {
    let (Some(parent_pid), Some(parent_start_time)) =
        (registration.parent_pid, registration.parent_start_time)
    else {
        return false;
    };
    if current_parent_start_time != Some(parent_start_time) {
        return false;
    }
    if registration.pid == root_pid {
        return current_parent_pid == parent_pid;
    }
    registered_start_times.get(&parent_pid) == Some(&parent_start_time)
        && (parent_pid != root_pid || parent_start_time == root_start_time)
}

fn attribution_status(diagnostics: &ProcessAttributionDiagnostics) -> AttributionStatus {
    if diagnostics.uncertain_processes > 0
        || diagnostics.identity_verification_failures > 0
        || diagnostics
            .exclusion_reasons
            .contains_key(&ProcessExclusionReason::ProcessLimitExceeded)
    {
        AttributionStatus::VerifiedPartial
    } else if diagnostics.registry_snapshot_complete
        && diagnostics.registered_processes > 0
        && diagnostics.independently_verified_registered_processes
            == diagnostics.registered_processes
    {
        AttributionStatus::VerifiedComplete
    } else {
        // No observed uncertainty is not proof that every browser process was
        // discovered, so collection cannot promote itself to complete.
        AttributionStatus::Unknown
    }
}

fn known_photon_process_role(role: &str) -> bool {
    matches!(
        role,
        "application"
            | "compositor"
            | "web_content"
            | "web_worker"
            | "request_server"
            | "image_decoder"
            | "media_server"
            | "audio_server"
            | "wasm_compiler"
            | "process_reaper"
    )
}

fn retained_session_evidence(
    previous_evidence: AttributionEvidence,
    still_in_private_session: bool,
) -> Option<AttributionEvidence> {
    (still_in_private_session
        && previous_evidence != AttributionEvidence::ChromiumProfileArgumentAndBundle)
        .then_some(AttributionEvidence::PreviouslyVerifiedSessionMembership)
}

fn belongs_to_private_session(pid: u32, uid: libc::uid_t, target: &ManagedProcessTarget) -> bool {
    uid == unsafe { libc::geteuid() }
        && process_session_id(pid) == Some(target.session_id)
        && target.session_id == target.root_pid
}

fn classify_chromium_profile_process(
    pid: u32,
    info: &libc::proc_bsdinfo,
    target: &ManagedProcessTarget,
    executable_path: PathBuf,
) -> ChromiumProfileAttribution {
    if !target.allow_profile_argument_attribution {
        return ChromiumProfileAttribution::Uncertain(format!(
            "browser-bundle candidate PID {pid} cannot use profile-argument attribution for this browser"
        ));
    }
    let Some(profile_path) = target.isolated_profile_path.as_deref() else {
        return ChromiumProfileAttribution::Uncertain(format!(
            "browser-bundle candidate PID {pid} has no isolated profile identity to verify"
        ));
    };
    if !is_within_application(&executable_path, &target.application_path) {
        return ChromiumProfileAttribution::NotSelectedApplication;
    }
    if info.pbi_uid != unsafe { libc::geteuid() } {
        return ChromiumProfileAttribution::NotSelectedApplication;
    }
    let arguments = match process_arguments(pid) {
        Ok(arguments) => arguments,
        Err(error) => return ChromiumProfileAttribution::Uncertain(error),
    };
    match user_data_directory_argument(&arguments) {
        Some(_) if has_exact_user_data_directory(&arguments, profile_path) => {
            ChromiumProfileAttribution::Verified {
                path: executable_path,
                role: chromium_process_role(&arguments),
            }
        }
        Some(_) => ChromiumProfileAttribution::DifferentProfile,
        None => ChromiumProfileAttribution::Uncertain(format!(
            "browser-bundle candidate PID {pid} has no verifiable isolated-profile argument"
        )),
    }
}

fn chromium_process_role(arguments: &[Vec<u8>]) -> String {
    let process_type = arguments.iter().enumerate().find_map(|(index, argument)| {
        argument.strip_prefix(b"--type=").or_else(|| {
            (argument.as_slice() == b"--type")
                .then(|| arguments.get(index + 1).map(Vec::as_slice))
                .flatten()
        })
    });
    let Some(process_type) = process_type else {
        return "browser".to_owned();
    };
    if process_type == b"utility"
        && let Some(subtype) = arguments
            .iter()
            .find_map(|argument| argument.strip_prefix(b"--utility-sub-type="))
    {
        let subtype = String::from_utf8_lossy(subtype);
        let safe_subtype = subtype
            .chars()
            .take(64)
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>();
        return format!("utility:{safe_subtype}");
    }
    String::from_utf8_lossy(process_type)
        .chars()
        .take(64)
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn record_exclusion(
    diagnostics: &mut ProcessAttributionDiagnostics,
    reason: ProcessExclusionReason,
) {
    *diagnostics.exclusion_reasons.entry(reason).or_default() += 1;
}

fn is_within_application(executable: &Path, application: &Path) -> bool {
    executable.starts_with(application)
        || executable
            .canonicalize()
            .ok()
            .zip(application.canonicalize().ok())
            .is_some_and(|(executable, application)| executable.starts_with(application))
}

fn same_executable_path(observed: &Path, expected: &Path) -> bool {
    observed == expected
        || observed
            .canonicalize()
            .ok()
            .zip(expected.canonicalize().ok())
            .is_some_and(|(observed, expected)| observed == expected)
}

fn start_time_of(info: &libc::proc_bsdinfo) -> ProcessStartTime {
    ProcessStartTime {
        seconds: info.pbi_start_tvsec,
        microseconds: info.pbi_start_tvusec,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_identity_uses_pid_and_start_time() {
        // SAFETY: `proc_bsdinfo` is a plain C metadata record and an all-zero
        // instance is valid for this focused identity predicate test.
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        info.pbi_pid = 41;
        info.pbi_start_tvsec = 100;
        info.pbi_start_tvusec = 7;
        let identity = ProcessStartTime {
            seconds: 100,
            microseconds: 7,
        };
        assert!(process_identity_matches(&info, 41, identity));
        assert!(!process_identity_matches(&info, 42, identity,));
        assert!(!process_identity_matches(
            &info,
            41,
            ProcessStartTime {
                seconds: 101,
                microseconds: 7,
            },
        ));
    }

    #[test]
    fn photon_parent_identity_uses_start_time_and_tolerates_reparenting() {
        let root_start = ProcessStartTime {
            seconds: 100,
            microseconds: 1,
        };
        let child_registration = velocity_metrics::RegisteredProcess {
            pid: 42,
            role: "web_content".to_owned(),
            start_time: Some(ProcessStartTime {
                seconds: 101,
                microseconds: 2,
            }),
            launch_generation: Some("101000002".to_owned()),
            parent_pid: Some(41),
            parent_start_time: Some(root_start),
        };
        let registered = BTreeMap::from([(41, root_start)]);
        assert!(registered_parent_identity_matches(
            &child_registration,
            41,
            1,
            Some(root_start),
            &registered,
            root_start
        ));
        assert!(!registered_parent_identity_matches(
            &child_registration,
            41,
            1,
            Some(ProcessStartTime {
                seconds: 100,
                microseconds: 2,
            }),
            &registered,
            root_start
        ));
        assert!(!registered_parent_identity_matches(
            &child_registration,
            41,
            1,
            Some(root_start),
            &BTreeMap::new(),
            root_start
        ));
    }

    #[test]
    fn photon_registration_identity_rejects_missing_or_reused_process_start() {
        let start_time = ProcessStartTime {
            seconds: 12,
            microseconds: 345,
        };
        let registration = velocity_metrics::RegisteredProcess {
            pid: 42,
            role: "web_content".to_owned(),
            start_time: Some(start_time),
            launch_generation: Some("12000345".to_owned()),
            parent_pid: Some(41),
            parent_start_time: Some(ProcessStartTime {
                seconds: 12,
                microseconds: 344,
            }),
        };
        assert!(registered_process_identity_matches(
            &registration,
            42,
            start_time
        ));
        assert!(!registered_process_identity_matches(
            &registration,
            43,
            start_time
        ));
        assert!(!registered_process_identity_matches(
            &registration,
            42,
            ProcessStartTime {
                seconds: 12,
                microseconds: 346,
            }
        ));
        let missing_generation = velocity_metrics::RegisteredProcess {
            launch_generation: None,
            ..registration
        };
        assert!(!registered_process_identity_matches(
            &missing_generation,
            42,
            start_time
        ));
    }

    #[test]
    fn reparented_session_members_are_retained_only_with_prior_proof() {
        assert_eq!(
            retained_session_evidence(AttributionEvidence::AncestryAndPrivateSession, true),
            Some(AttributionEvidence::PreviouslyVerifiedSessionMembership)
        );
        assert_eq!(
            retained_session_evidence(AttributionEvidence::PrivateSessionMembership, false),
            None
        );
        assert_eq!(
            retained_session_evidence(AttributionEvidence::ChromiumProfileArgumentAndBundle, true),
            None
        );
    }

    #[test]
    fn uncertain_candidates_make_attribution_partial_but_no_candidate_is_not_complete() {
        let mut diagnostics = ProcessAttributionDiagnostics::default();
        assert_eq!(attribution_status(&diagnostics), AttributionStatus::Unknown);

        diagnostics.excluded_process_candidates = 1;
        diagnostics
            .exclusion_reasons
            .insert(ProcessExclusionReason::DifferentIsolatedProfile, 1);
        assert_eq!(attribution_status(&diagnostics), AttributionStatus::Unknown);

        diagnostics.uncertain_processes = 1;
        diagnostics
            .exclusion_reasons
            .insert(ProcessExclusionReason::OwnershipEvidenceUnavailable, 1);
        assert_eq!(
            attribution_status(&diagnostics),
            AttributionStatus::VerifiedPartial
        );
    }

    #[test]
    fn photon_registry_is_complete_only_with_authoritative_and_verified_records() {
        let mut diagnostics = ProcessAttributionDiagnostics {
            registry_snapshot_complete: true,
            registered_processes: 2,
            independently_verified_registered_processes: 2,
            ..ProcessAttributionDiagnostics::default()
        };
        assert_eq!(
            attribution_status(&diagnostics),
            AttributionStatus::VerifiedComplete
        );
        diagnostics.independently_verified_registered_processes = 1;
        assert_eq!(attribution_status(&diagnostics), AttributionStatus::Unknown);
        diagnostics.uncertain_processes = 1;
        assert_eq!(
            attribution_status(&diagnostics),
            AttributionStatus::VerifiedPartial
        );
    }

    #[test]
    fn chromium_roles_come_from_verified_process_type_arguments() {
        assert_eq!(chromium_process_role(&[]), "browser");
        assert_eq!(
            chromium_process_role(&[b"--type=renderer".to_vec()]),
            "renderer"
        );
        assert_eq!(
            chromium_process_role(&[
                b"--type".to_vec(),
                b"utility".to_vec(),
                b"--utility-sub-type=network.mojom.NetworkService".to_vec(),
            ]),
            "utility:network.mojom.NetworkService"
        );
    }

    #[test]
    fn chromium_executable_must_be_inside_the_selected_application_bundle() {
        let application = Path::new("/Applications/Google Chrome.app");
        assert!(is_within_application(
            Path::new("/Applications/Google Chrome.app/Contents/Helpers/Chrome Helper"),
            application
        ));
        assert!(!is_within_application(
            Path::new("/Applications/Google Chrome Canary.app/Contents/Helpers/Chrome Helper"),
            application
        ));
    }
}
