use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use serde_json::Value;

use crate::support::{invoke, output, path, stage, success, with_progress};

const PROJECT_PATH: &str = "ios/PhotonBrowser.xcodeproj";
const SCHEME: &str = "PhotonBrowser";
const APP_NAME: &str = "PhotonBrowser.app";
const DERIVED_DATA_PATH: &str = "build/ios/DerivedData";

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum DeviceKind {
    All,
    Simulator,
    Model,
    Physical,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum BuildConfiguration {
    Debug,
    Release,
}

impl BuildConfiguration {
    fn xcode_name(self) -> &'static str {
        match self {
            Self::Debug => "Debug",
            Self::Release => "Release",
        }
    }

    fn product_directory(self, platform: DevicePlatform) -> &'static str {
        match (self, platform) {
            (Self::Debug, DevicePlatform::Simulator) => "Debug-iphonesimulator",
            (Self::Release, DevicePlatform::Simulator) => "Release-iphonesimulator",
            (Self::Debug, DevicePlatform::Physical) => "Debug-iphoneos",
            (Self::Release, DevicePlatform::Physical) => "Release-iphoneos",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DeviceSelection {
    pub(crate) device: Option<String>,
    pub(crate) model: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DevicePlatform {
    Simulator,
    Physical,
}

#[derive(Clone, Debug)]
struct Simulator {
    name: String,
    identifier: String,
    device_type_identifier: String,
    runtime_identifier: String,
    state: String,
}

#[derive(Clone, Debug)]
struct PhysicalDevice {
    name: String,
    model: String,
    identifier: String,
    paired: bool,
    connected: bool,
    state: String,
}

#[derive(Clone, Debug)]
struct SimulatorModel {
    name: String,
    identifier: String,
    runtime: Option<SimulatorRuntime>,
}

#[derive(Clone, Debug)]
struct SimulatorRuntime {
    identifier: String,
    name: String,
    version: String,
}

#[derive(Clone, Debug)]
enum Device {
    Simulator(Simulator),
    Physical(PhysicalDevice),
}

impl Device {
    fn platform(&self) -> DevicePlatform {
        match self {
            Self::Simulator(_) => DevicePlatform::Simulator,
            Self::Physical(_) => DevicePlatform::Physical,
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Simulator(device) => &device.name,
            Self::Physical(device) => &device.name,
        }
    }

    fn identifier(&self) -> &str {
        match self {
            Self::Simulator(device) => &device.identifier,
            Self::Physical(device) => &device.identifier,
        }
    }

    fn destination(&self) -> String {
        let platform = match self.platform() {
            DevicePlatform::Simulator => "iOS Simulator",
            DevicePlatform::Physical => "iOS",
        };
        format!("platform={platform},id={}", self.identifier())
    }
}

#[derive(Clone, Debug)]
enum BuildTarget {
    GenericSimulator,
    Device(Device),
}

impl BuildTarget {
    fn destination(&self) -> String {
        match self {
            Self::GenericSimulator => "generic/platform=iOS Simulator".to_owned(),
            Self::Device(device) => device.destination(),
        }
    }

    fn platform(&self) -> DevicePlatform {
        match self {
            Self::GenericSimulator => DevicePlatform::Simulator,
            Self::Device(device) => device.platform(),
        }
    }
}

pub(crate) fn list_devices(
    root: &Path,
    kind: DeviceKind,
    paired_only: bool,
    connected_only: bool,
) -> Result<(), String> {
    if paired_only && !matches!(kind, DeviceKind::All | DeviceKind::Physical) {
        return Err("option --paired can only be used with --kind all or --kind physical".into());
    }
    if connected_only && kind == DeviceKind::Model {
        return Err("option --connected cannot be used with --kind model".into());
    }

    let mut rows = Vec::new();
    if matches!(kind, DeviceKind::All | DeviceKind::Simulator) {
        rows.extend(load_simulators(root)?.into_iter().map(|device| DeviceRow {
            kind: "simulator",
            name: device.name.clone(),
            state: format!(
                "{} · {}",
                device.state,
                runtime_label(&device.runtime_identifier)
            ),
            identifier: device.identifier.clone(),
            paired: false,
            connected: device.state.eq_ignore_ascii_case("booted"),
        }));
    }

    if matches!(kind, DeviceKind::All | DeviceKind::Model) {
        rows.extend(
            load_simulator_models(root)?
                .into_iter()
                .filter_map(|model| {
                    let runtime = model.runtime?;
                    Some(DeviceRow {
                        kind: "model",
                        name: model.name,
                        state: format!("available · {}", runtime.name),
                        identifier: model.identifier,
                        paired: false,
                        connected: false,
                    })
                }),
        );
    }

    if matches!(kind, DeviceKind::All | DeviceKind::Physical) {
        rows.extend(
            load_physical_devices(root)?
                .into_iter()
                .map(|device| DeviceRow {
                    kind: "physical",
                    name: format!("{} ({})", device.name, device.model),
                    state: device.state,
                    identifier: device.identifier,
                    paired: device.paired,
                    connected: device.connected,
                }),
        );
    }

    rows.retain(|row| {
        (!paired_only || (row.kind == "physical" && row.paired))
            && (!connected_only || row.connected)
    });
    rows.sort_by(|left, right| {
        left.kind
            .cmp(right.kind)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });

    if rows.is_empty() {
        println!("No iPhones match the selected filters.");
        return Ok(());
    }

    println!("{:<11} {:<36} {:<28} IDENTIFIER", "KIND", "IPHONE", "STATE");
    println!("{:-<11} {:-<36} {:-<28} {:-<40}", "", "", "", "");
    for row in rows {
        println!(
            "{:<11} {:<36} {:<28} {}",
            row.kind, row.name, row.state, row.identifier
        );
    }
    Ok(())
}

struct DeviceRow {
    kind: &'static str,
    name: String,
    state: String,
    identifier: String,
    paired: bool,
    connected: bool,
}

pub(crate) fn build(
    root: &Path,
    selection: DeviceSelection,
    configuration: BuildConfiguration,
    team_id: Option<&str>,
    allow_provisioning_updates: bool,
    allow_device_registration: bool,
    verbose: bool,
) -> Result<(), String> {
    validate_team_id(team_id)?;
    let target = resolve_build_target(root, &selection, verbose)?;
    build_target(
        root,
        &target,
        configuration,
        team_id,
        allow_provisioning_updates,
        allow_device_registration,
        verbose,
    )?;
    Ok(())
}

pub(crate) fn launch(
    root: &Path,
    selection: DeviceSelection,
    configuration: BuildConfiguration,
    verbose: bool,
) -> Result<(), String> {
    let target = resolve_launch_target(root, &selection, verbose)?;
    launch_target(root, &target, configuration, verbose)
}

pub(crate) fn run(
    root: &Path,
    selection: DeviceSelection,
    configuration: BuildConfiguration,
    team_id: Option<&str>,
    allow_provisioning_updates: bool,
    allow_device_registration: bool,
    verbose: bool,
) -> Result<(), String> {
    validate_team_id(team_id)?;
    let target = resolve_launch_target(root, &selection, verbose)?;
    ensure_launchable(&target)?;
    build_target(
        root,
        &BuildTarget::Device(target.clone()),
        configuration,
        team_id,
        allow_provisioning_updates,
        allow_device_registration,
        verbose,
    )?;
    launch_target(root, &target, configuration, verbose)
}

fn resolve_build_target(
    root: &Path,
    selection: &DeviceSelection,
    verbose: bool,
) -> Result<BuildTarget, String> {
    if let Some(model) = &selection.model {
        return ensure_simulator_for_model(root, model, verbose)
            .map(|device| BuildTarget::Device(Device::Simulator(device)));
    }
    if let Some(device) = &selection.device {
        return resolve_existing_device(root, device).map(BuildTarget::Device);
    }
    Ok(BuildTarget::GenericSimulator)
}

fn resolve_launch_target(
    root: &Path,
    selection: &DeviceSelection,
    verbose: bool,
) -> Result<Device, String> {
    if let Some(model) = &selection.model {
        return ensure_simulator_for_model(root, model, verbose).map(Device::Simulator);
    }
    if let Some(device) = &selection.device {
        return resolve_existing_device(root, device);
    }
    default_simulator(root).map(Device::Simulator)
}

fn default_simulator(root: &Path) -> Result<Simulator, String> {
    let simulators = load_simulators(root)?;
    let booted: Vec<_> = simulators
        .iter()
        .filter(|device| device.state.eq_ignore_ascii_case("booted"))
        .cloned()
        .collect();
    if booted.len() == 1 {
        return Ok(booted[0].clone());
    }
    if booted.len() > 1 {
        return Err("multiple iPhone simulators are booted; pass --model or --device".into());
    }
    if simulators.len() == 1 {
        return Ok(simulators[0].clone());
    }
    Err("select a target with --model or --device; see photon ios devices".into())
}

fn resolve_existing_device(root: &Path, query: &str) -> Result<Device, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("device name or UDID cannot be empty".into());
    }

    let simulators = load_simulators(root)?;
    if let Some(device) = simulators
        .iter()
        .find(|device| device.identifier.eq_ignore_ascii_case(query))
    {
        return Ok(Device::Simulator(device.clone()));
    }
    let physical = load_physical_devices(root)?;
    if let Some(device) = physical
        .iter()
        .find(|device| device.identifier.eq_ignore_ascii_case(query))
    {
        return Ok(Device::Physical(device.clone()));
    }

    let mut matches: Vec<Device> = simulators
        .iter()
        .filter(|device| device.name.eq_ignore_ascii_case(query))
        .cloned()
        .map(Device::Simulator)
        .collect();
    matches.extend(
        physical
            .iter()
            .filter(|device| device.name.eq_ignore_ascii_case(query))
            .cloned()
            .map(Device::Physical),
    );
    match matches.as_slice() {
        [device] => Ok(device.clone()),
        [] => Err(format!(
            "no iPhone named or identified by {query}; run photon ios devices to list targets"
        )),
        _ => {
            let identifiers = matches
                .iter()
                .map(|device| format!("{} ({})", device.name(), device.identifier()))
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "{query} matches multiple iPhones: {identifiers}; use a UDID"
            ))
        }
    }
}

fn ensure_simulator_for_model(
    root: &Path,
    query: &str,
    verbose: bool,
) -> Result<Simulator, String> {
    let models = load_simulator_models(root)?;
    let Some(model) = models.iter().find(|model| {
        model.name.eq_ignore_ascii_case(query.trim())
            || model.identifier.eq_ignore_ascii_case(query.trim())
    }) else {
        return Err(format!(
            "iPhone simulator model {query} is not available in this Xcode installation; run photon ios devices --kind model"
        ));
    };

    let simulators = load_simulators(root)?;
    let model_simulators: Vec<_> = simulators
        .iter()
        .filter(|device| device.device_type_identifier == model.identifier)
        .cloned()
        .collect();
    if let Some(device) = choose_model_simulator(&model_simulators, &model.name)? {
        return Ok(device);
    }

    let Some(runtime) = &model.runtime else {
        return Err(format!(
            "no installed iOS Simulator runtime supports {}; install an iOS runtime in Xcode Settings > Platforms",
            model.name
        ));
    };

    let name = model.name.as_str();
    let device_type = model.identifier.as_str();
    let runtime_id = runtime.identifier.as_str();
    stage(&format!(
        "Create simulator {} ({})",
        model.name, runtime.name
    ));
    let identifier = with_progress("Create iPhone simulator", !verbose, || {
        output(
            "xcrun",
            &["simctl", "create", name, device_type, runtime_id],
            root,
        )
    })?
    .trim()
    .to_owned();
    if identifier.is_empty() {
        return Err(format!(
            "simctl created {} without returning a UDID",
            model.name
        ));
    }

    Ok(Simulator {
        name: name.to_owned(),
        identifier,
        device_type_identifier: device_type.to_owned(),
        runtime_identifier: runtime_id.to_owned(),
        state: "Shutdown".to_owned(),
    })
}

fn choose_model_simulator(
    devices: &[Simulator],
    model_name: &str,
) -> Result<Option<Simulator>, String> {
    let exact_name: Vec<_> = devices
        .iter()
        .filter(|device| device.name.eq_ignore_ascii_case(model_name))
        .cloned()
        .collect();
    if exact_name.len() == 1 {
        return Ok(exact_name.into_iter().next());
    }

    let booted: Vec<_> = devices
        .iter()
        .filter(|device| device.state.eq_ignore_ascii_case("booted"))
        .cloned()
        .collect();
    if booted.len() == 1 {
        return Ok(booted.into_iter().next());
    }
    if devices.len() == 1 {
        return Ok(devices.first().cloned());
    }
    if devices.is_empty() {
        return Ok(None);
    }

    let identifiers = devices
        .iter()
        .map(|device| format!("{} ({})", device.name, device.identifier))
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "multiple simulators use the {model_name} model: {identifiers}; pass a simulator UDID with --device"
    ))
}

fn build_target(
    root: &Path,
    target: &BuildTarget,
    configuration: BuildConfiguration,
    team_id: Option<&str>,
    allow_provisioning_updates: bool,
    allow_device_registration: bool,
    verbose: bool,
) -> Result<PathBuf, String> {
    let project = path(&root.join(PROJECT_PATH));
    let derived_data = path(&root.join(DERIVED_DATA_PATH));
    let destination = target.destination();
    let configuration_name = configuration.xcode_name();
    let mut arguments = vec![
        "-project".to_owned(),
        project,
        "-scheme".to_owned(),
        SCHEME.to_owned(),
        "-configuration".to_owned(),
        configuration_name.to_owned(),
        "-destination".to_owned(),
        destination,
        "-derivedDataPath".to_owned(),
        derived_data,
    ];
    if let Some(team_id) = team_id {
        arguments.push(format!("DEVELOPMENT_TEAM={team_id}"));
    }
    if allow_provisioning_updates {
        arguments.push("-allowProvisioningUpdates".to_owned());
    }
    if allow_device_registration {
        arguments.push("-allowProvisioningDeviceRegistration".to_owned());
    }
    arguments.push("build".to_owned());
    let args: Vec<_> = arguments.iter().map(String::as_str).collect();

    stage(&format!(
        "Build iOS app ({configuration_name}, {})",
        match target.platform() {
            DevicePlatform::Simulator => "simulator",
            DevicePlatform::Physical => "device",
        }
    ));
    with_progress("Build PhotonBrowser for iOS", !verbose, || {
        invoke("xcodebuild", &args, root, verbose)
    })?;

    let app = app_bundle_path(root, target.platform(), configuration);
    if !app.is_dir() {
        return Err(format!(
            "Xcode completed the build but the app bundle was not found at {}",
            app.display()
        ));
    }
    success(&format!("Built {}", app.display()));
    Ok(app)
}

fn validate_team_id(team_id: Option<&str>) -> Result<(), String> {
    if let Some(team_id) = team_id
        && (team_id.len() != 10 || !team_id.bytes().all(|byte| byte.is_ascii_alphanumeric()))
    {
        return Err("Apple Developer Team ID must be 10 ASCII letters or digits".into());
    }
    Ok(())
}

fn launch_target(
    root: &Path,
    target: &Device,
    configuration: BuildConfiguration,
    verbose: bool,
) -> Result<(), String> {
    ensure_launchable(target)?;
    let app = app_bundle_path(root, target.platform(), configuration);
    if !app.is_dir() {
        return Err(format!(
            "{} has not been built; run photon ios build first",
            app.display()
        ));
    }
    let app_path = path(&app);
    let info_plist = path(&app.join("Info.plist"));
    let bundle_identifier = output(
        "plutil",
        &[
            "-extract",
            "CFBundleIdentifier",
            "raw",
            "-o",
            "-",
            info_plist.as_str(),
        ],
        root,
    )?
    .trim()
    .to_owned();

    match target {
        Device::Simulator(device) => {
            if !device.state.eq_ignore_ascii_case("booted") {
                invoke(
                    "xcrun",
                    &["simctl", "boot", device.identifier.as_str()],
                    root,
                    verbose,
                )?;
            }
            invoke(
                "xcrun",
                &["simctl", "bootstatus", device.identifier.as_str(), "-b"],
                root,
                verbose,
            )?;
            let developer_directory = output("xcode-select", &["-p"], root)?;
            let simulator_app = Path::new(developer_directory.trim())
                .join("Applications")
                .join("Simulator.app");
            if simulator_app.is_dir() {
                let simulator_app = path(&simulator_app);
                invoke(
                    "open",
                    &[
                        "-a",
                        simulator_app.as_str(),
                        "--args",
                        "-CurrentDeviceUDID",
                        device.identifier.as_str(),
                    ],
                    root,
                    verbose,
                )?;
            } else {
                stage("Simulator.app is unavailable; launching through simctl");
            }
            invoke(
                "xcrun",
                &[
                    "simctl",
                    "install",
                    device.identifier.as_str(),
                    app_path.as_str(),
                ],
                root,
                verbose,
            )?;
            invoke(
                "xcrun",
                &[
                    "simctl",
                    "launch",
                    device.identifier.as_str(),
                    bundle_identifier.as_str(),
                ],
                root,
                verbose,
            )?;
        }
        Device::Physical(device) => {
            invoke(
                "xcrun",
                &[
                    "devicectl",
                    "device",
                    "install",
                    "app",
                    "--device",
                    device.identifier.as_str(),
                    app_path.as_str(),
                ],
                root,
                verbose,
            )?;
            invoke(
                "xcrun",
                &[
                    "devicectl",
                    "device",
                    "process",
                    "launch",
                    "--device",
                    device.identifier.as_str(),
                    bundle_identifier.as_str(),
                ],
                root,
                verbose,
            )?;
        }
    }

    success(&format!("Launched PhotonBrowser on {}", target.name()));
    Ok(())
}

fn ensure_launchable(target: &Device) -> Result<(), String> {
    if let Device::Physical(device) = target {
        if !device.paired {
            return Err(format!(
                "{} is not paired with this Mac; pair it in Xcode before launching",
                device.name
            ));
        }
        if !device.connected {
            return Err(format!(
                "{} is paired but disconnected; connect it to this Mac and retry",
                device.name
            ));
        }
    }
    Ok(())
}

fn app_bundle_path(
    root: &Path,
    platform: DevicePlatform,
    configuration: BuildConfiguration,
) -> PathBuf {
    root.join(DERIVED_DATA_PATH)
        .join("Build")
        .join("Products")
        .join(configuration.product_directory(platform))
        .join(APP_NAME)
}

fn load_simulators(root: &Path) -> Result<Vec<Simulator>, String> {
    let json = output(
        "xcrun",
        &["simctl", "list", "--json", "devices", "available"],
        root,
    )?;
    let value = parse_json("simctl device list", &json)?;
    let Some(runtimes) = value.get("devices").and_then(Value::as_object) else {
        return Err("simctl returned device data without a devices object".into());
    };

    let mut devices = Vec::new();
    for (runtime_identifier, entries) in runtimes {
        let Some(entries) = entries.as_array() else {
            continue;
        };
        for entry in entries {
            let Some(device_type_identifier) =
                entry.get("deviceTypeIdentifier").and_then(Value::as_str)
            else {
                continue;
            };
            if !device_type_identifier.starts_with("com.apple.CoreSimulator.SimDeviceType.iPhone-")
                || entry.get("isAvailable").and_then(Value::as_bool) == Some(false)
            {
                continue;
            }
            let Some(name) = entry.get("name").and_then(Value::as_str) else {
                continue;
            };
            let Some(identifier) = entry.get("udid").and_then(Value::as_str) else {
                continue;
            };
            devices.push(Simulator {
                name: name.to_owned(),
                identifier: identifier.to_owned(),
                device_type_identifier: device_type_identifier.to_owned(),
                runtime_identifier: runtime_identifier.clone(),
                state: entry
                    .get("state")
                    .and_then(Value::as_str)
                    .unwrap_or("Unknown")
                    .to_owned(),
            });
        }
    }
    devices.sort_by_key(|device| device.name.to_lowercase());
    Ok(devices)
}

fn load_simulator_models(root: &Path) -> Result<Vec<SimulatorModel>, String> {
    let device_types_json = output("xcrun", &["simctl", "list", "--json", "devicetypes"], root)?;
    let runtimes_json = output("xcrun", &["simctl", "list", "--json", "runtimes"], root)?;
    let device_types = parse_json("simctl device type list", &device_types_json)?;
    let runtime_data = parse_json("simctl runtime list", &runtimes_json)?;
    let runtimes: Vec<SimulatorRuntime> = runtime_data
        .get("runtimes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|runtime| runtime.get("isAvailable").and_then(Value::as_bool) == Some(true))
        .filter_map(|runtime| {
            let identifier = runtime.get("identifier")?.as_str()?.to_owned();
            let name = runtime.get("name")?.as_str()?.to_owned();
            let version = runtime.get("version")?.as_str()?.to_owned();
            name.starts_with("iOS ").then_some(SimulatorRuntime {
                identifier,
                name,
                version,
            })
        })
        .collect();

    let mut models = Vec::new();
    for device_type in device_types
        .get("devicetypes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if device_type.get("productFamily").and_then(Value::as_str) != Some("iPhone") {
            continue;
        }
        let (Some(name), Some(identifier)) = (
            device_type.get("name").and_then(Value::as_str),
            device_type.get("identifier").and_then(Value::as_str),
        ) else {
            continue;
        };
        if !name.starts_with("iPhone ") {
            continue;
        }
        let minimum_runtime = device_type
            .get("minRuntimeVersionString")
            .and_then(Value::as_str)
            .unwrap_or("0");
        let maximum_runtime = device_type
            .get("maxRuntimeVersionString")
            .and_then(Value::as_str)
            .unwrap_or("65535.255.255");
        let runtime = runtimes
            .iter()
            .filter(|runtime| {
                version_at_least(&runtime.version, minimum_runtime)
                    && version_at_least(maximum_runtime, &runtime.version)
            })
            .max_by(|left, right| compare_versions(&left.version, &right.version))
            .cloned();
        models.push(SimulatorModel {
            name: name.to_owned(),
            identifier: identifier.to_owned(),
            runtime,
        });
    }
    models.sort_by_key(|model| model.name.to_lowercase());
    Ok(models)
}

fn load_physical_devices(root: &Path) -> Result<Vec<PhysicalDevice>, String> {
    let json = output(
        "xcrun",
        &["devicectl", "list", "devices", "--json-output", "-"],
        root,
    )?;
    let value = parse_json("devicectl device list", &json)?;
    let Some(devices) = value.pointer("/result/devices").and_then(Value::as_array) else {
        return Err("devicectl returned device data without result.devices".into());
    };

    let mut physical = Vec::new();
    for device in devices {
        let reality = string_at(device, &["properties", "hardware", "reality"])
            .or_else(|| string_at(device, &["hardwareProperties", "reality"]));
        let device_type = string_at(device, &["properties", "hardware", "deviceType"])
            .or_else(|| string_at(device, &["hardwareProperties", "deviceType"]));
        if reality != Some("physical") || device_type != Some("iPhone") {
            continue;
        }

        let name = string_at(device, &["properties", "state", "name"])
            .or_else(|| string_at(device, &["deviceProperties", "name"]))
            .unwrap_or("Unnamed iPhone");
        let model = string_at(device, &["properties", "hardware", "marketingName"])
            .or_else(|| string_at(device, &["hardwareProperties", "marketingName"]))
            .or_else(|| string_at(device, &["properties", "hardware", "productType"]))
            .or_else(|| string_at(device, &["hardwareProperties", "productType"]))
            .unwrap_or("iPhone");
        let identifier = string_at(device, &["properties", "hardware", "udid"])
            .or_else(|| string_at(device, &["hardwareProperties", "udid"]))
            .or_else(|| device.get("identifier").and_then(Value::as_str))
            .ok_or_else(|| format!("devicectl returned {name} without a device identifier"))?;
        let pairing_state = string_at(device, &["properties", "connection", "pairingState"])
            .or_else(|| string_at(device, &["connectionProperties", "pairingState"]));
        let connection_state = string_at(device, &["properties", "connection", "state"])
            .or_else(|| string_at(device, &["connectionProperties", "tunnelState"]));
        let paired = pairing_state.is_some_and(|state| state.eq_ignore_ascii_case("paired"));
        let connected =
            connection_state.is_some_and(|state| state.eq_ignore_ascii_case("connected"));
        let state = match (connected, paired) {
            (true, true) => "connected · paired",
            (true, false) => "connected · not paired",
            (false, true) => "disconnected · paired",
            (false, false) => "disconnected · not paired",
        };

        physical.push(PhysicalDevice {
            name: name.to_owned(),
            model: model.to_owned(),
            identifier: identifier.to_owned(),
            paired,
            connected,
            state: state.to_owned(),
        });
    }
    physical.sort_by_key(|device| device.name.to_lowercase());
    Ok(physical)
}

fn parse_json(command: &str, json: &str) -> Result<Value, String> {
    serde_json::from_str(json).map_err(|error| format!("cannot parse {command} JSON: {error}"))
}

fn string_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    path.iter()
        .try_fold(value, |value, key| value.get(*key))
        .and_then(Value::as_str)
}

fn runtime_label(identifier: &str) -> String {
    let label = identifier
        .strip_prefix("com.apple.CoreSimulator.SimRuntime.")
        .unwrap_or(identifier)
        .replace('-', ".");
    label.replacen("iOS.", "iOS ", 1)
}

fn version_at_least(version: &str, minimum: &str) -> bool {
    compare_versions(version, minimum) != Ordering::Less
}

fn compare_versions(left: &str, right: &str) -> Ordering {
    let left = version_components(left);
    let right = version_components(right);
    let component_count = left.len().max(right.len());
    (0..component_count)
        .map(|index| {
            left.get(index)
                .copied()
                .unwrap_or(0)
                .cmp(&right.get(index).copied().unwrap_or(0))
        })
        .find(|ordering| *ordering != Ordering::Equal)
        .unwrap_or(Ordering::Equal)
}

fn version_components(version: &str) -> Vec<u64> {
    version
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect()
}
