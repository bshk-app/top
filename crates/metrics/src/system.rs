use std::time::{SystemTime, UNIX_EPOCH};

use crate::cf;
use crate::snapshot::{Battery, ChipInfo, ThermalPressure};
use crate::sys;

pub fn chip_info() -> ChipInfo {
    // perflevel0 is the performance cluster, perflevel1 the efficiency cluster.
    let performance_cores = sys::sysctl_value::<u32>("hw.perflevel0.logicalcpu").unwrap_or(0);
    let efficiency_cores = sys::sysctl_value::<u32>("hw.perflevel1.logicalcpu").unwrap_or(0);
    ChipInfo {
        name: sys::sysctl_string("machdep.cpu.brand_string")
            .unwrap_or_else(|| "Apple Silicon".into()),
        efficiency_cores,
        performance_cores,
        gpu_cores: gpu_core_count(),
        memory_bytes: sys::sysctl_value::<u64>("hw.memsize").unwrap_or(0),
    }
}

fn gpu_core_count() -> u32 {
    let mut cores = 0;
    cf::for_each_service("AGXAccelerator", |accelerator| {
        if let Some(count) = cf::property(accelerator, "gpu-core-count") {
            cores = cf::number_i64(count.as_ptr()).unwrap_or(0) as u32;
        }
    });
    cores
}

pub fn uptime_secs() -> u64 {
    let boot = sys::sysctl_value::<sys::timeval>("kern.boottime").map(|t| t.tv_sec);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    uptime_since(boot, now)
}

/// Zero when the boot time is unknown, rather than the time since 1970.
fn uptime_since(boot: Option<i64>, now: u64) -> u64 {
    boot.filter(|&secs| secs > 0)
        .map_or(0, |secs| now.saturating_sub(secs as u64))
}

/// Reads the kernel's thermal pressure level through libnotify.
pub fn thermal_pressure() -> ThermalPressure {
    let mut token = 0;
    let mut level = 0u64;
    unsafe {
        let name = c"com.apple.system.thermalpressurelevel";
        if sys::notify_register_check(name.as_ptr(), &mut token) != 0 {
            return ThermalPressure::Nominal;
        }
        sys::notify_get_state(token, &mut level);
        sys::notify_cancel(token);
    }
    match level {
        0 => ThermalPressure::Nominal,
        1 => ThermalPressure::Moderate,
        2 => ThermalPressure::Heavy,
        3 => ThermalPressure::Trapping,
        _ => ThermalPressure::Sleeping,
    }
}

pub fn battery() -> Option<Battery> {
    let mut battery = None;
    cf::for_each_service("AppleSmartBattery", |entry| {
        let number = |key| cf::property(entry, key).and_then(|n| cf::number_i64(n.as_ptr()));
        if let (Some(current), Some(max)) = (number("CurrentCapacity"), number("MaxCapacity"))
            && max > 0
        {
            battery = Some(Battery {
                percent: (current * 100 / max).clamp(0, 100) as u32,
                on_ac: cf::property(entry, "ExternalConnected")
                    .is_some_and(|value| cf::is_true(value.as_ptr())),
            });
        }
    });
    battery
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_boot_time_is_not_counted_from_1970() {
        assert_eq!(uptime_since(None, 1_760_000_000), 0);
        assert_eq!(uptime_since(Some(1_760_000_000 - 90), 1_760_000_000), 90);
    }
}
