//! Raw declarations for the macOS system libraries this crate reads.
//!
//! Layouts follow the arm64 macOS SDK headers named next to each item.
#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]

use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_void};
use std::ptr;

pub type kern_return_t = c_int;
pub type mach_port_t = c_uint;
pub type natural_t = c_uint;
pub type integer_t = c_int;

// <mach/*.h>
pub const KERN_SUCCESS: kern_return_t = 0;
pub const PROCESSOR_CPU_LOAD_INFO: c_int = 2;
pub const CPU_STATE_USER: usize = 0;
pub const CPU_STATE_SYSTEM: usize = 1;
pub const CPU_STATE_IDLE: usize = 2;
pub const CPU_STATE_NICE: usize = 3;
pub const HOST_VM_INFO64: c_int = 4;

/// `struct vm_statistics64` from <mach/vm_statistics.h>.
#[repr(C)]
#[derive(Default)]
pub struct vm_statistics64 {
    pub free_count: natural_t,
    pub active_count: natural_t,
    pub inactive_count: natural_t,
    pub wire_count: natural_t,
    pub zero_fill_count: u64,
    pub reactivations: u64,
    pub pageins: u64,
    pub pageouts: u64,
    pub faults: u64,
    pub cow_faults: u64,
    pub lookups: u64,
    pub hits: u64,
    pub purges: u64,
    pub purgeable_count: natural_t,
    pub speculative_count: natural_t,
    pub decompressions: u64,
    pub compressions: u64,
    pub swapins: u64,
    pub swapouts: u64,
    pub compressor_page_count: natural_t,
    pub throttled_count: natural_t,
    pub external_page_count: natural_t,
    pub internal_page_count: natural_t,
    pub total_uncompressed_pages_in_compressor: u64,
}

/// `struct mach_timebase_info` from <mach/mach_time.h>.
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct mach_timebase_info {
    pub numer: u32,
    pub denom: u32,
}

unsafe extern "C" {
    pub static mach_task_self_: mach_port_t;
    pub fn mach_host_self() -> mach_port_t;
    pub fn host_processor_info(
        host: mach_port_t,
        flavor: c_int,
        out_processor_count: *mut natural_t,
        out_processor_info: *mut *mut integer_t,
        out_processor_info_count: *mut natural_t,
    ) -> kern_return_t;
    pub fn host_statistics64(
        host: mach_port_t,
        flavor: c_int,
        info: *mut integer_t,
        count: *mut natural_t,
    ) -> kern_return_t;
    pub fn vm_deallocate(task: mach_port_t, address: usize, size: usize) -> kern_return_t;
    pub fn mach_timebase_info(info: *mut mach_timebase_info) -> kern_return_t;
    pub fn mach_absolute_time() -> u64;
}

pub const MACH_PORT_RIGHT_SEND: u32 = 0;

unsafe extern "C" {
    pub fn mach_port_get_refs(
        task: mach_port_t,
        name: mach_port_t,
        right: u32,
        refs: *mut u32,
    ) -> kern_return_t;
}

/// The host port. Each `mach_host_self()` call adds a send-right reference,
/// so take one for the life of the process instead of one per sample.
pub fn host_port() -> mach_port_t {
    static HOST: std::sync::OnceLock<mach_port_t> = std::sync::OnceLock::new();
    *HOST.get_or_init(|| unsafe { mach_host_self() })
}

// <sys/sysctl.h>
pub const CTL_KERN: c_int = 1;
pub const CTL_NET: c_int = 4;
pub const KERN_PROC: c_int = 14;
pub const KERN_PROC_ALL: c_int = 0;
pub const KERN_PROC_PID: c_int = 1;
pub const PF_ROUTE: c_int = 17;
pub const NET_RT_IFLIST2: c_int = 6;
pub const RTM_IFINFO2: u8 = 0x12;
pub const ENOMEM: c_int = 12;

unsafe extern "C" {
    pub fn sysctl(
        name: *mut c_int,
        namelen: c_uint,
        oldp: *mut c_void,
        oldlenp: *mut usize,
        newp: *mut c_void,
        newlen: usize,
    ) -> c_int;
    pub fn sysctlbyname(
        name: *const c_char,
        oldp: *mut c_void,
        oldlenp: *mut usize,
        newp: *mut c_void,
        newlen: usize,
    ) -> c_int;
}

/// Reads a fixed-size `sysctl` value.
pub fn sysctl_value<T: Default>(name: &str) -> Option<T> {
    let name = CString::new(name).ok()?;
    let mut value = T::default();
    let mut len = size_of::<T>();
    let rc = unsafe {
        sysctlbyname(
            name.as_ptr(),
            (&mut value as *mut T).cast(),
            &mut len,
            ptr::null_mut(),
            0,
        )
    };
    (rc == 0 && len == size_of::<T>()).then_some(value)
}

/// Reads a variable-size `sysctl` table, growing the buffer while it races new entries.
pub fn sysctl_table(mib: &mut [c_int]) -> Option<Vec<u8>> {
    for _ in 0..4 {
        let mut len = 0usize;
        let rc = unsafe {
            sysctl(
                mib.as_mut_ptr(),
                mib.len() as c_uint,
                ptr::null_mut(),
                &mut len,
                ptr::null_mut(),
                0,
            )
        };
        if rc != 0 {
            return None;
        }
        len += len / 8;
        let mut buf = vec![0u8; len];
        let rc = unsafe {
            sysctl(
                mib.as_mut_ptr(),
                mib.len() as c_uint,
                buf.as_mut_ptr().cast(),
                &mut len,
                ptr::null_mut(),
                0,
            )
        };
        if rc == 0 {
            buf.truncate(len);
            return Some(buf);
        }
        if errno() != ENOMEM {
            return None;
        }
    }
    None
}

/// Reads a string `sysctl` value.
pub fn sysctl_string(name: &str) -> Option<String> {
    let name = CString::new(name).ok()?;
    let mut len = 0usize;
    unsafe {
        if sysctlbyname(name.as_ptr(), ptr::null_mut(), &mut len, ptr::null_mut(), 0) != 0 {
            return None;
        }
        let mut buf = vec![0u8; len];
        if sysctlbyname(
            name.as_ptr(),
            buf.as_mut_ptr().cast(),
            &mut len,
            ptr::null_mut(),
            0,
        ) != 0
        {
            return None;
        }
        Some(
            CStr::from_bytes_until_nul(&buf)
                .ok()?
                .to_string_lossy()
                .into_owned(),
        )
    }
}

/// `struct xsw_usage` from <sys/sysctl.h>, the value of `vm.swapusage`.
#[repr(C)]
#[derive(Default)]
pub struct xsw_usage {
    pub xsu_total: u64,
    pub xsu_avail: u64,
    pub xsu_used: u64,
    pub xsu_pagesize: u32,
    pub xsu_encrypted: u32,
}

/// `struct timeval` from <sys/_types/_timeval.h>, the value of `kern.boottime`.
#[repr(C)]
#[derive(Default)]
pub struct timeval {
    pub tv_sec: i64,
    pub tv_usec: i32,
}

/// Byte offsets into `struct kinfo_proc` (648 bytes on arm64).
pub mod kinfo_proc {
    pub const SIZE: usize = 648;
    /// `kp_proc.p_starttime`, a `timeval`.
    pub const START_SEC: usize = 0;
    pub const START_USEC: usize = 8;
    pub const PID: usize = 40;
    pub const COMM: usize = 243;
    pub const COMM_LEN: usize = 17;
    /// `kp_eproc.e_ucred.cr_uid`.
    pub const UID: usize = 420;
}

// <libproc.h>, <sys/proc_info.h>
pub const PROC_PIDTASKINFO: c_int = 4;
pub const PROC_PIDPATHINFO_MAXSIZE: usize = 4096;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct proc_taskinfo {
    pub pti_virtual_size: u64,
    pub pti_resident_size: u64,
    /// Mach absolute time units, not nanoseconds, on Apple Silicon.
    pub pti_total_user: u64,
    pub pti_total_system: u64,
    pub pti_threads_user: u64,
    pub pti_threads_system: u64,
    pub pti_policy: i32,
    pub pti_faults: i32,
    pub pti_pageins: i32,
    pub pti_cow_faults: i32,
    pub pti_messages_sent: i32,
    pub pti_messages_received: i32,
    pub pti_syscalls_mach: i32,
    pub pti_syscalls_unix: i32,
    pub pti_csw: i32,
    pub pti_threadnum: i32,
    pub pti_numrunning: i32,
    pub pti_priority: i32,
}

unsafe extern "C" {
    pub fn proc_pidinfo(
        pid: c_int,
        flavor: c_int,
        arg: u64,
        buffer: *mut c_void,
        buffersize: c_int,
    ) -> c_int;
    pub fn proc_pidpath(pid: c_int, buffer: *mut c_void, buffersize: u32) -> c_int;
}

// <notify.h>
unsafe extern "C" {
    pub fn notify_register_check(name: *const c_char, out_token: *mut c_int) -> u32;
    pub fn notify_get_state(token: c_int, state: *mut u64) -> u32;
    pub fn notify_cancel(token: c_int) -> u32;
}

// <pwd.h>, <signal.h>, <errno.h>
#[repr(C)]
pub struct passwd {
    pub pw_name: *mut c_char,
    pub pw_passwd: *mut c_char,
    pub pw_uid: u32,
    pub pw_gid: u32,
    pub pw_change: i64,
    pub pw_class: *mut c_char,
    pub pw_gecos: *mut c_char,
    pub pw_dir: *mut c_char,
    pub pw_shell: *mut c_char,
    pub pw_expire: i64,
}

pub const SIGTERM: c_int = 15;
pub const SIGKILL: c_int = 9;
pub const EPERM: c_int = 1;
pub const ESRCH: c_int = 3;

unsafe extern "C" {
    pub fn getpwuid_r(
        uid: u32,
        pwd: *mut passwd,
        buf: *mut c_char,
        buflen: usize,
        result: *mut *mut passwd,
    ) -> c_int;
    pub fn kill(pid: c_int, sig: c_int) -> c_int;
    pub fn geteuid() -> u32;
    pub fn __error() -> *mut c_int;
}

pub fn errno() -> c_int {
    unsafe { *__error() }
}

pub const IFF_LOOPBACK: i32 = 0x8;

// <sys/mount.h>; arm64 only has the 64-bit inode layout.
pub const MFSNAMELEN: usize = 16;
pub const MAXPATHLEN: usize = 1024;
pub const MNT_NOWAIT: c_int = 2;
pub const MNT_LOCAL: u32 = 0x0000_1000;
pub const MNT_DONTBROWSE: u32 = 0x0010_0000;

#[repr(C)]
pub struct statfs {
    pub f_bsize: u32,
    pub f_iosize: i32,
    pub f_blocks: u64,
    pub f_bfree: u64,
    pub f_bavail: u64,
    pub f_files: u64,
    pub f_ffree: u64,
    pub f_fsid: [i32; 2],
    pub f_owner: u32,
    pub f_type: u32,
    pub f_flags: u32,
    pub f_fssubtype: u32,
    pub f_fstypename: [c_char; MFSNAMELEN],
    pub f_mntonname: [c_char; MAXPATHLEN],
    pub f_mntfromname: [c_char; MAXPATHLEN],
    pub f_flags_ext: u32,
    pub f_reserved: [u32; 7],
}

unsafe extern "C" {
    /// Thread-safe `getmntinfo`: the buffer belongs to the caller, who frees it.
    pub fn getmntinfo_r_np(mntbufp: *mut *mut statfs, flags: c_int) -> c_int;
    pub fn free(ptr: *mut c_void);
}

/// Converts a fixed, NUL-padded C array to a `String`.
pub fn c_array_to_string(chars: &[c_char]) -> String {
    let bytes: Vec<u8> = chars
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}
