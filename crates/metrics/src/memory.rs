use crate::snapshot::Memory;
use crate::sys;

pub fn read(total: u64) -> Memory {
    let page_size = sys::sysctl_value::<u64>("hw.pagesize").unwrap_or(16_384);
    let mut stats = sys::vm_statistics64::default();
    let mut count =
        (size_of::<sys::vm_statistics64>() / size_of::<sys::integer_t>()) as sys::natural_t;
    let rc = unsafe {
        sys::host_statistics64(
            sys::host_port(),
            sys::HOST_VM_INFO64,
            (&mut stats as *mut sys::vm_statistics64).cast(),
            &mut count,
        )
    };
    // mactop (via gopsutil): everything except free and inactive pages is used.
    let used = if rc == sys::KERN_SUCCESS {
        let available = (stats.free_count as u64 + stats.inactive_count as u64) * page_size;
        total.saturating_sub(available)
    } else {
        0
    };
    let swap = sys::sysctl_value::<sys::xsw_usage>("vm.swapusage").unwrap_or_default();
    Memory {
        used: used.min(total),
        total,
        swap_used: swap.xsu_used,
        swap_total: swap.xsu_total,
    }
}
