use crate::snapshot::{CoreKind, CoreUsage};
use crate::sys;

/// Per-core tick counters; usage is the busy share of the delta between reads.
#[derive(Default)]
pub struct CpuTicks {
    previous: Vec<[u32; 4]>,
}

impl CpuTicks {
    /// Returns busy shares, 0–100, in logical CPU order.
    pub fn sample(&mut self) -> Vec<f32> {
        let current = read_ticks();
        let usage = current
            .iter()
            .enumerate()
            .map(|(ix, now)| {
                let before = self.previous.get(ix).copied().unwrap_or_default();
                busy_share(before, *now)
            })
            .collect();
        self.previous = current;
        usage
    }
}

/// Busy share, 0–100, between two reads of one core's tick counters.
/// The kernel counters are 32-bit, so deltas wrap rather than saturate.
fn busy_share(before: [u32; 4], now: [u32; 4]) -> f32 {
    let delta = |state: usize| now[state].wrapping_sub(before[state]) as f64;
    let busy =
        delta(sys::CPU_STATE_USER) + delta(sys::CPU_STATE_SYSTEM) + delta(sys::CPU_STATE_NICE);
    let total = busy + delta(sys::CPU_STATE_IDLE);
    if total > 0.0 {
        (busy / total * 100.0) as f32
    } else {
        0.0
    }
}

/// Labels logical CPUs: Apple Silicon numbers efficiency cores first.
pub fn label_cores(usage: &[f32], efficiency_cores: u32) -> Vec<CoreUsage> {
    usage
        .iter()
        .enumerate()
        .map(|(ix, &usage)| CoreUsage {
            kind: if (ix as u32) < efficiency_cores {
                CoreKind::Efficiency
            } else {
                CoreKind::Performance
            },
            usage,
        })
        .collect()
}

fn read_ticks() -> Vec<[u32; 4]> {
    let mut cpu_count: sys::natural_t = 0;
    let mut info: *mut sys::integer_t = std::ptr::null_mut();
    let mut info_count: sys::natural_t = 0;
    let rc = unsafe {
        sys::host_processor_info(
            sys::host_port(),
            sys::PROCESSOR_CPU_LOAD_INFO,
            &mut cpu_count,
            &mut info,
            &mut info_count,
        )
    };
    if rc != sys::KERN_SUCCESS || info.is_null() {
        return Vec::new();
    }
    let ticks = unsafe { std::slice::from_raw_parts(info as *const u32, info_count as usize) };
    let result = ticks
        .chunks_exact(4)
        .take(cpu_count as usize)
        .map(|t| [t[0], t[1], t[2], t[3]])
        .collect();
    unsafe {
        sys::vm_deallocate(
            sys::mach_task_self_,
            info as usize,
            info_count as usize * size_of::<sys::integer_t>(),
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_counter_wraparound_is_not_read_as_full_load() {
        let before = [100, 50, u32::MAX - 9, 0];
        let after = [110, 50, 90, 0];
        // 10 busy ticks against 100 idle ticks across the wrap.
        assert!((busy_share(before, after) - 9.09).abs() < 0.01);
    }

    #[test]
    fn sampling_does_not_leak_host_port_references() {
        let host = sys::host_port();
        let refs = || {
            let mut refs = 0;
            unsafe {
                sys::mach_port_get_refs(
                    sys::mach_task_self_,
                    host,
                    sys::MACH_PORT_RIGHT_SEND,
                    &mut refs,
                )
            };
            refs
        };
        let mut ticks = CpuTicks::default();
        let before = refs();
        for _ in 0..50 {
            ticks.sample();
            crate::memory::read(1 << 34);
        }
        // Tests running in parallel can hold a reference for a moment; a leak
        // would add one per call, a hundred here.
        assert!(refs() < before + 50, "{} → {}", before, refs());
    }
}
