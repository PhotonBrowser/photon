use std::{ffi::CStr, mem::MaybeUninit, os::unix::ffi::OsStrExt, path::PathBuf, sync::OnceLock};

use velocity_metrics::ProcessStartTime;

pub(super) fn process_start_time(pid: u32) -> Option<ProcessStartTime> {
    let info = bsd_info(pid)?;
    Some(ProcessStartTime {
        seconds: info.pbi_start_tvsec,
        microseconds: info.pbi_start_tvusec,
    })
}

pub(super) fn bsd_info(pid: u32) -> Option<libc::proc_bsdinfo> {
    let mut info = MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let result = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int,
        )
    };
    (result == std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int)
        .then(|| unsafe { info.assume_init() })
}

pub(super) fn process_path(pid: u32) -> Option<PathBuf> {
    let mut buffer = [0_i8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let result = unsafe {
        libc::proc_pidpath(
            pid as libc::c_int,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
        )
    };
    if result <= 0 {
        return None;
    }
    let path = unsafe { CStr::from_ptr(buffer.as_ptr()) };
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(path.to_bytes())))
}

pub(super) fn process_usage(pid: u32) -> Option<libc::rusage_info_v2> {
    let mut usage = MaybeUninit::<libc::rusage_info_v2>::uninit();
    let result = unsafe {
        libc::proc_pid_rusage(
            pid as libc::c_int,
            libc::RUSAGE_INFO_V2,
            usage.as_mut_ptr().cast(),
        )
    };
    (result == 0).then(|| unsafe { usage.assume_init() })
}

pub(super) fn process_session_id(pid: u32) -> Option<u32> {
    let session_id = unsafe { libc::getsid(pid as libc::pid_t) };
    (session_id > 0).then_some(session_id as u32)
}

/// Enumerates PID candidates using libproc. Callers still need to verify each
/// candidate's process start identity and session membership.
pub(super) fn all_process_ids() -> Result<Vec<u32>, String> {
    const INITIAL_CAPACITY: usize = 256;
    const MAX_CAPACITY: usize = 65_536;
    let mut pids = vec![0 as libc::pid_t; INITIAL_CAPACITY];
    loop {
        let buffer_size = std::mem::size_of_val(pids.as_slice());
        let count =
            unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), buffer_size as libc::c_int) };
        if count < 0 {
            return Err("libproc could not enumerate process IDs".to_owned());
        }
        let count = count as usize;
        if count < pids.len() {
            return Ok(pids
                .into_iter()
                .take(count)
                .filter(|pid| *pid > 0)
                .map(|pid| pid as u32)
                .collect());
        }
        if pids.len() >= MAX_CAPACITY {
            return Err(format!(
                "system process list exceeded the safety limit of {MAX_CAPACITY} entries"
            ));
        }
        pids.resize((pids.len() * 2).min(MAX_CAPACITY), 0);
    }
}

/// Returns argv only; callers must not log or serialize it because arbitrary
/// process arguments can contain private application data.
pub(super) fn process_arguments(pid: u32) -> Result<Vec<Vec<u8>>, String> {
    const MAX_ARGUMENT_BYTES: usize = 256 * 1024;
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as libc::c_int];
    let mut length = 0_usize;
    let size_result = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            std::ptr::null_mut(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if size_result != 0 || length < std::mem::size_of::<libc::c_int>() {
        return Err(format!(
            "kernel command-line metadata unavailable for PID {pid}"
        ));
    }
    if length > MAX_ARGUMENT_BYTES {
        return Err(format!(
            "kernel command-line metadata for PID {pid} exceeded the safety limit"
        ));
    }
    let mut buffer = vec![0_u8; length];
    let read_result = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            buffer.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if read_result != 0 || length > buffer.len() {
        return Err(format!("kernel command-line read failed for PID {pid}"));
    }
    buffer.truncate(length);
    parse_procargs2(&buffer)
}

fn parse_procargs2(buffer: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    const MAX_ARGUMENT_COUNT: usize = 4_096;
    if buffer.len() < std::mem::size_of::<libc::c_int>() {
        return Err("process argument buffer is truncated".to_owned());
    }
    let argument_count = i32::from_ne_bytes(buffer[..4].try_into().expect("four-byte argc"));
    let argument_count = usize::try_from(argument_count)
        .ok()
        .filter(|count| *count <= MAX_ARGUMENT_COUNT)
        .ok_or_else(|| {
            "process argument count is invalid or exceeds the safety limit".to_owned()
        })?;
    let executable_end = buffer[4..]
        .iter()
        .position(|byte| *byte == 0)
        .map(|offset| offset + 5)
        .ok_or_else(|| "process executable argument is unterminated".to_owned())?;
    let mut offset = executable_end;
    while offset < buffer.len() && buffer[offset] == 0 {
        offset += 1;
    }
    let mut arguments = Vec::with_capacity(argument_count);
    for _ in 0..argument_count {
        if offset > buffer.len() {
            return Err("process argument buffer is truncated".to_owned());
        }
        let end = buffer[offset..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|length| offset + length)
            .ok_or_else(|| "process argument is unterminated".to_owned())?;
        arguments.push(buffer[offset..end].to_vec());
        offset = end.saturating_add(1);
    }
    Ok(arguments)
}

pub(super) fn user_data_directory_argument(arguments: &[Vec<u8>]) -> Option<&[u8]> {
    arguments
        .iter()
        .find_map(|argument| argument.strip_prefix(b"--user-data-dir="))
        .or_else(|| {
            arguments
                .windows(2)
                .find(|pair| pair[0] == b"--user-data-dir")
                .map(|pair| pair[1].as_slice())
        })
}

pub(super) fn has_exact_user_data_directory(arguments: &[Vec<u8>], path: &std::path::Path) -> bool {
    user_data_directory_argument(arguments) == Some(path.as_os_str().as_bytes())
}

/// `ri_user_time` and `ri_system_time` are Mach time units, not nanoseconds.
/// Convert their sum using the timebase reported by `mach_timebase_info`.
pub(super) fn cpu_time_nanoseconds(usage: &libc::rusage_info_v2) -> Option<u64> {
    let timebase = MACH_TIMEBASE
        .get_or_init(read_mach_timebase)
        .as_ref()
        .copied()?;
    let ticks = usage.ri_user_time.checked_add(usage.ri_system_time)?;
    mach_ticks_to_nanoseconds(ticks, timebase.numer, timebase.denom)
}

#[derive(Clone, Copy)]
struct MachTimebase {
    numer: u32,
    denom: u32,
}

static MACH_TIMEBASE: OnceLock<Option<MachTimebase>> = OnceLock::new();

#[repr(C)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

#[link(name = "System")]
unsafe extern "C" {
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> libc::c_int;
}

fn read_mach_timebase() -> Option<MachTimebase> {
    let mut info = MachTimebaseInfo { numer: 0, denom: 0 };
    let result = unsafe { mach_timebase_info(&mut info) };
    (result == 0 && info.numer > 0 && info.denom > 0).then_some(MachTimebase {
        numer: info.numer,
        denom: info.denom,
    })
}

fn mach_ticks_to_nanoseconds(ticks: u64, numer: u32, denom: u32) -> Option<u64> {
    if numer == 0 || denom == 0 {
        return None;
    }
    let nanoseconds = u128::from(ticks) * u128::from(numer) / u128::from(denom);
    u64::try_from(nanoseconds).ok()
}

pub(super) fn child_processes(parent_pid: u32) -> Result<Vec<u32>, String> {
    const INITIAL_CAPACITY: usize = 64;
    const MAX_CAPACITY: usize = 16_384;
    let mut pids = vec![0 as libc::pid_t; INITIAL_CAPACITY];
    loop {
        let count = unsafe {
            libc::proc_listchildpids(
                parent_pid as libc::pid_t,
                pids.as_mut_ptr().cast(),
                std::mem::size_of_val(pids.as_slice()) as libc::c_int,
            )
        };
        if count < 0 {
            return Err(format!(
                "libproc could not enumerate children of PID {parent_pid}"
            ));
        }
        let count = count as usize;
        if count < pids.len() {
            return Ok(pids
                .into_iter()
                .take(count)
                .filter(|pid| *pid > 0)
                .map(|pid| pid as u32)
                .collect());
        }
        if pids.len() >= MAX_CAPACITY {
            return Err(format!(
                "child list for PID {parent_pid} exceeded {MAX_CAPACITY} entries"
            ));
        }
        pids.resize((pids.len() * 2).min(MAX_CAPACITY), 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libproc_reads_current_process_identity_and_resource_usage() {
        let pid = std::process::id();
        let info = bsd_info(pid).expect("libproc should report the current test process");
        assert_eq!(info.pbi_pid, pid);
        assert!(process_start_time(pid).is_some());
        assert!(process_path(pid).is_some());
        assert!(process_usage(pid).is_some());
        assert!(child_processes(pid).is_ok());
    }

    #[test]
    fn mach_cpu_time_is_converted_to_nanoseconds_using_timebase() {
        assert_eq!(mach_ticks_to_nanoseconds(3, 125, 3), Some(125));
        assert_eq!(mach_ticks_to_nanoseconds(10, 1, 3), Some(3));
        assert_eq!(mach_ticks_to_nanoseconds(10, 0, 1), None);
        assert_eq!(mach_ticks_to_nanoseconds(10, 1, 0), None);
    }

    #[test]
    fn procargs2_parser_reads_only_bounded_argument_vector() {
        let mut data = Vec::new();
        data.extend_from_slice(&2_i32.to_ne_bytes());
        data.extend_from_slice(b"/Applications/Browser\0\0");
        data.extend_from_slice(b"Browser\0--user-data-dir=/private/profile\0");
        let arguments = parse_procargs2(&data).unwrap();
        assert_eq!(
            arguments,
            [
                b"Browser".to_vec(),
                b"--user-data-dir=/private/profile".to_vec()
            ]
        );
        assert!(has_exact_user_data_directory(
            &arguments,
            std::path::Path::new("/private/profile")
        ));
        assert!(!has_exact_user_data_directory(
            &arguments,
            std::path::Path::new("/private/other")
        ));
        assert_eq!(
            user_data_directory_argument(&arguments),
            Some(b"/private/profile".as_slice())
        );
        let separated = vec![
            b"Browser".to_vec(),
            b"--user-data-dir".to_vec(),
            b"/private/separated".to_vec(),
        ];
        assert_eq!(
            user_data_directory_argument(&separated),
            Some(b"/private/separated".as_slice())
        );
        assert_eq!(user_data_directory_argument(&[b"Browser".to_vec()]), None);
    }
}
