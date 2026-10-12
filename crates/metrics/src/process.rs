use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_int};
use std::time::Instant;

use crate::cf;
use crate::snapshot::Process;
use crate::sys;

/// A pid is reused after exit; the start time tells the two processes apart.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ProcessKey {
    pid: i32,
    start_sec: i64,
    start_usec: i32,
}

impl ProcessKey {
    fn read(entry: &[u8]) -> Self {
        Self {
            pid: read_i32(entry, sys::kinfo_proc::PID),
            start_sec: read_i64(entry, sys::kinfo_proc::START_SEC),
            start_usec: read_i32(entry, sys::kinfo_proc::START_USEC),
        }
    }

    /// Start time in microseconds since the epoch.
    fn started_at(self) -> u64 {
        self.start_sec.max(0) as u64 * 1_000_000 + self.start_usec.max(0) as u64
    }
}

struct Previous {
    cpu_ns: u64,
}

/// Lists every process and turns cumulative CPU and GPU time into rates.
pub struct ProcessTable {
    timebase: sys::mach_timebase_info,
    /// Task info is readable only for our own processes, unless we run as root.
    euid: u32,
    previous: HashMap<ProcessKey, Previous>,
    previous_gpu: HashMap<i32, u64>,
    previous_at: Option<Instant>,
    users: HashMap<u32, String>,
    names: HashMap<ProcessKey, String>,
}

impl ProcessTable {
    pub fn new() -> Self {
        let mut timebase = sys::mach_timebase_info::default();
        unsafe { sys::mach_timebase_info(&mut timebase) };
        if timebase.denom == 0 {
            timebase = sys::mach_timebase_info { numer: 1, denom: 1 };
        }
        Self {
            timebase,
            euid: unsafe { sys::geteuid() },
            previous: HashMap::new(),
            previous_gpu: HashMap::new(),
            previous_at: None,
            users: HashMap::new(),
            names: HashMap::new(),
        }
    }

    pub fn sample(&mut self, memory_total: u64) -> Vec<Process> {
        let now = Instant::now();
        let wall_ns = self
            .previous_at
            .map(|at| now.duration_since(at).as_nanos() as f64)
            .filter(|&ns| ns > 0.0);
        self.previous_at = Some(now);

        let gpu_now = gpu_time_by_pid();
        let mut previous = HashMap::with_capacity(self.previous.len());
        let mut names = HashMap::with_capacity(self.names.len());
        let mut processes = Vec::new();

        let mut mib = [sys::CTL_KERN, sys::KERN_PROC, sys::KERN_PROC_ALL];
        let table = sys::sysctl_table(&mut mib).unwrap_or_default();
        for entry in table.chunks_exact(sys::kinfo_proc::SIZE) {
            let key = ProcessKey::read(entry);
            if key.pid == 0 {
                continue;
            }
            let uid = read_i32(entry, sys::kinfo_proc::UID) as u32;
            let command = self
                .names
                .remove(&key)
                .unwrap_or_else(|| command_name(key.pid, entry));
            let user = self.user_name(uid);

            let mut process = Process {
                pid: key.pid,
                started_at: key.started_at(),
                user,
                command: command.clone(),
                ..Process::default()
            };

            // Other users' processes refuse task info without root, so don't ask; they keep zeros.
            let readable = self.euid == 0 || uid == self.euid;
            if let Some(task) = readable.then(|| task_info(key.pid)).flatten() {
                let cpu_ns = self.ticks_to_ns(task.pti_total_user + task.pti_total_system);
                if let (Some(wall_ns), Some(before)) = (wall_ns, self.previous.get(&key))
                    && cpu_ns >= before.cpu_ns
                {
                    process.cpu = ((cpu_ns - before.cpu_ns) as f64 / wall_ns * 100.0) as f32;
                }
                process.cpu_time = cpu_ns as f64 / 1e9;
                process.resident = task.pti_resident_size;
                if memory_total > 0 {
                    process.memory =
                        (task.pti_resident_size as f64 / memory_total as f64 * 100.0) as f32;
                }
                previous.insert(key, Previous { cpu_ns });
            }

            if let (Some(wall_ns), Some(&gpu_ns), Some(&before)) = (
                wall_ns,
                gpu_now.get(&key.pid),
                self.previous_gpu.get(&key.pid),
            ) && gpu_ns >= before
            {
                process.gpu =
                    (((gpu_ns - before) as f64 / wall_ns) * 100.0).clamp(0.0, 100.0) as f32;
            }

            names.insert(key, command);
            processes.push(process);
        }

        self.previous = previous;
        self.previous_gpu = gpu_now;
        self.names = names;
        processes
    }

    fn ticks_to_ns(&self, ticks: u64) -> u64 {
        (ticks as u128 * self.timebase.numer as u128 / self.timebase.denom as u128) as u64
    }

    fn user_name(&mut self, uid: u32) -> String {
        self.users
            .entry(uid)
            .or_insert_with(|| lookup_user(uid))
            .clone()
    }
}

impl Default for ProcessTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Why a signal could not be delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalError {
    /// The process belongs to another user or to the system.
    NotPermitted,
    /// The process already exited.
    NoSuchProcess,
    Other(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Terminate,
    Kill,
}

/// Signals the process `pid` only while it is still the one that started at
/// `started_at` (see [`Process::started_at`]), so a reused pid is never hit.
pub fn send_signal(pid: i32, started_at: u64, signal: Signal) -> Result<(), SignalError> {
    if !is_running(pid, started_at) {
        return Err(SignalError::NoSuchProcess);
    }
    let sig = match signal {
        Signal::Terminate => sys::SIGTERM,
        Signal::Kill => sys::SIGKILL,
    };
    if unsafe { sys::kill(pid, sig) } == 0 {
        return Ok(());
    }
    Err(match sys::errno() {
        sys::EPERM => SignalError::NotPermitted,
        sys::ESRCH => SignalError::NoSuchProcess,
        other => SignalError::Other(other),
    })
}

/// Whether the process that started at `started_at` is still running as `pid`.
pub fn is_running(pid: i32, started_at: u64) -> bool {
    let mut mib = [sys::CTL_KERN, sys::KERN_PROC, sys::KERN_PROC_PID, pid];
    let mut entry = [0u8; sys::kinfo_proc::SIZE];
    let mut len = entry.len();
    let rc = unsafe {
        sys::sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            entry.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    rc == 0 && len == entry.len() && ProcessKey::read(&entry).started_at() == started_at
}

fn task_info(pid: c_int) -> Option<sys::proc_taskinfo> {
    let mut info = sys::proc_taskinfo::default();
    let size = size_of::<sys::proc_taskinfo>() as c_int;
    let read = unsafe {
        sys::proc_pidinfo(
            pid,
            sys::PROC_PIDTASKINFO,
            0,
            (&mut info as *mut sys::proc_taskinfo).cast(),
            size,
        )
    };
    (read == size).then_some(info)
}

fn command_name(pid: c_int, entry: &[u8]) -> String {
    let mut path = vec![0u8; sys::PROC_PIDPATHINFO_MAXSIZE];
    let len = unsafe { sys::proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32) };
    if len > 0 {
        let path = String::from_utf8_lossy(&path[..len as usize]);
        if let Some(name) = path.rsplit('/').next().filter(|n| !n.is_empty()) {
            return name.to_owned();
        }
    }
    let comm = &entry[sys::kinfo_proc::COMM..sys::kinfo_proc::COMM + sys::kinfo_proc::COMM_LEN];
    let end = comm.iter().position(|&b| b == 0).unwrap_or(comm.len());
    String::from_utf8_lossy(&comm[..end]).into_owned()
}

fn lookup_user(uid: u32) -> String {
    let mut pwd: sys::passwd = unsafe { std::mem::zeroed() };
    let mut buf = vec![0 as c_char; 1024];
    let mut result: *mut sys::passwd = std::ptr::null_mut();
    let rc = unsafe { sys::getpwuid_r(uid, &mut pwd, buf.as_mut_ptr(), buf.len(), &mut result) };
    if rc == 0 && !result.is_null() && !pwd.pw_name.is_null() {
        unsafe { CStr::from_ptr(pwd.pw_name) }
            .to_string_lossy()
            .into_owned()
    } else {
        uid.to_string()
    }
}

/// Cumulative GPU time in nanoseconds per pid, from the GPU driver's user clients.
fn gpu_time_by_pid() -> HashMap<i32, u64> {
    let mut totals = HashMap::new();
    cf::for_each_service("AGXAccelerator", |accelerator| {
        cf::for_each_child(accelerator, |client| {
            let Some(pid) = cf::property(client, "IOUserClientCreator")
                .and_then(|creator| cf::string(creator.as_ptr()))
                .and_then(|creator| parse_creator_pid(&creator))
            else {
                return;
            };
            let Some(usage) = cf::property(client, "AppUsage") else {
                return;
            };
            let ns: u64 = cf::array_items(usage.as_ptr())
                .filter_map(|item| cf::number_i64(cf::dict_get(item, "accumulatedGPUTime")))
                .map(|ns| ns.max(0) as u64)
                .sum();
            *totals.entry(pid).or_insert(0) += ns;
        });
    });
    totals
}

/// Parses `"pid 454, WindowServer"`.
fn parse_creator_pid(creator: &str) -> Option<i32> {
    creator
        .strip_prefix("pid ")?
        .split(',')
        .next()?
        .trim()
        .parse()
        .ok()
}

fn read_i32(bytes: &[u8], at: usize) -> i32 {
    i32::from_ne_bytes(bytes[at..at + 4].try_into().expect("4 bytes"))
}

fn read_i64(bytes: &[u8], at: usize) -> i64 {
    i64::from_ne_bytes(bytes[at..at + 8].try_into().expect("8 bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_user_client_creator() {
        assert_eq!(parse_creator_pid("pid 454, WindowServer"), Some(454));
        assert_eq!(parse_creator_pid("WindowServer"), None);
    }

    #[test]
    fn recognises_a_process_only_by_its_start_time() {
        let mut table = ProcessTable::new();
        let me = table
            .sample(1 << 34)
            .into_iter()
            .find(|p| p.pid == std::process::id() as i32)
            .expect("current process listed");
        assert!(is_running(me.pid, me.started_at));
        assert!(
            !is_running(me.pid, me.started_at + 1),
            "a reused pid has another start time"
        );
        assert_eq!(
            send_signal(me.pid, me.started_at + 1, Signal::Terminate),
            Err(SignalError::NoSuchProcess)
        );
    }

    #[test]
    fn lists_the_current_process_with_its_own_task_info() {
        let mut table = ProcessTable::new();
        let processes = table.sample(1 << 34);
        let me = processes
            .iter()
            .find(|p| p.pid == std::process::id() as i32)
            .expect("current process listed");
        assert!(me.resident > 0);
        assert!(!me.command.is_empty());
    }
}
