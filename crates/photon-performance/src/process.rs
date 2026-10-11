use std::time::Instant;

/// The Photon shell process's current CPU and memory usage.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ProcessDiagnostics {
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<u64>,
}

/// Samples the current process without starting a helper process.
#[derive(Default)]
pub struct ProcessSampler {
    previous: Option<(Instant, u64)>,
}

impl ProcessSampler {
    pub fn sample(&mut self) -> ProcessDiagnostics {
        let now = Instant::now();
        let Some(usage) = current_process_usage() else {
            self.previous = None;
            return ProcessDiagnostics::default();
        };

        let cpu_percent = self.previous.and_then(|(previous_at, previous_cpu)| {
            let elapsed_us = now.duration_since(previous_at).as_secs_f64() * 1_000_000.0;
            usage
                .cpu_time_microseconds
                .checked_sub(previous_cpu)
                .filter(|_| elapsed_us > 0.0)
                .map(|delta| 100.0 * delta as f64 / elapsed_us)
        });
        self.previous = Some((now, usage.cpu_time_microseconds));

        ProcessDiagnostics {
            cpu_percent,
            memory_bytes: Some(usage.memory_bytes),
        }
    }
}

struct ProcessUsage {
    cpu_time_microseconds: u64,
    memory_bytes: u64,
}

#[cfg(target_os = "macos")]
fn current_process_usage() -> Option<ProcessUsage> {
    use mach2::{
        kern_return::KERN_SUCCESS,
        task::task_info,
        task_info::{
            MACH_TASK_BASIC_INFO, MACH_TASK_BASIC_INFO_COUNT, TASK_THREAD_TIMES_INFO,
            TASK_THREAD_TIMES_INFO_COUNT, TASK_VM_INFO, mach_task_basic_info,
            task_thread_times_info, task_vm_info,
        },
        traps::mach_task_self,
        vm_types::natural_t,
    };

    // SAFETY: This returns the current process's task port.
    let task = unsafe { mach_task_self() };
    let mut basic = mach_task_basic_info::default();
    let mut basic_count = MACH_TASK_BASIC_INFO_COUNT;
    let mut threads = task_thread_times_info::default();
    let mut threads_count = TASK_THREAD_TIMES_INFO_COUNT;
    let mut vm = task_vm_info::default();
    let mut vm_count =
        (std::mem::size_of::<task_vm_info>() / std::mem::size_of::<natural_t>()) as u32;

    // SAFETY: Each buffer has the matching Mach task_info structure and count.
    let basic_result = unsafe {
        task_info(
            task,
            MACH_TASK_BASIC_INFO,
            (&mut basic as *mut mach_task_basic_info).cast(),
            &mut basic_count,
        )
    };
    // SAFETY: Each buffer has the matching Mach task_info structure and count.
    let threads_result = unsafe {
        task_info(
            task,
            TASK_THREAD_TIMES_INFO,
            (&mut threads as *mut task_thread_times_info).cast(),
            &mut threads_count,
        )
    };
    // SAFETY: Each buffer has the matching Mach task_info structure and count.
    let vm_result = unsafe {
        task_info(
            task,
            TASK_VM_INFO,
            (&mut vm as *mut task_vm_info).cast(),
            &mut vm_count,
        )
    };
    if basic_result != KERN_SUCCESS || threads_result != KERN_SUCCESS || vm_result != KERN_SUCCESS {
        return None;
    }

    let basic_cpu = time_value_microseconds(basic.user_time)
        .checked_add(time_value_microseconds(basic.system_time))?;
    let live_thread_cpu = time_value_microseconds(threads.user_time)
        .checked_add(time_value_microseconds(threads.system_time))?;
    Some(ProcessUsage {
        cpu_time_microseconds: basic_cpu.checked_add(live_thread_cpu)?,
        memory_bytes: vm.phys_footprint,
    })
}

#[cfg(target_os = "macos")]
fn time_value_microseconds(value: mach2::time_value::time_value_t) -> u64 {
    (value.seconds.max(0) as u64)
        .saturating_mul(1_000_000)
        .saturating_add(value.microseconds.max(0) as u64)
}

#[cfg(target_os = "linux")]
fn current_process_usage() -> Option<ProcessUsage> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let fields = stat
        .rsplit_once(')')?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    let user_ticks = fields.get(11)?.parse::<u64>().ok()?;
    let system_ticks = fields.get(12)?.parse::<u64>().ok()?;
    // SAFETY: sysconf reads process-independent constants and has no pointers.
    let ticks_per_second = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    // SAFETY: sysconf reads process-independent constants and has no pointers.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if ticks_per_second <= 0 || page_size <= 0 {
        return None;
    }

    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages = statm.split_whitespace().collect::<Vec<_>>();
    let resident_pages = pages.get(1)?.parse::<u64>().ok()?;
    let shared_pages = pages.get(2)?.parse::<u64>().ok()?;
    let private_pages = resident_pages.checked_sub(shared_pages)?;
    let cpu_ticks = user_ticks.checked_add(system_ticks)?;
    let cpu_time_microseconds = cpu_ticks
        .checked_mul(1_000_000)?
        .checked_div(ticks_per_second as u64)?;
    let memory_bytes = private_pages.checked_mul(page_size as u64)?;

    Some(ProcessUsage {
        cpu_time_microseconds,
        memory_bytes,
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn current_process_usage() -> Option<ProcessUsage> {
    None
}
