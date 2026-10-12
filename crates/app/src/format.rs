//! Text for measured values, in the compact notation of `top` and mactop.

const KB: f64 = 1024.0;
const MB: f64 = KB * 1024.0;
const GB: f64 = MB * 1024.0;
const TB: f64 = GB * 1024.0;

/// `843.7 GB` or `1.0 TB` for volume sizes.
pub fn capacity(bytes: u64) -> String {
    let bytes = bytes as f64;
    if bytes >= TB {
        format!("{:.1} TB", bytes / TB)
    } else {
        format!("{:.1} GB", bytes / GB)
    }
}

/// `178M`, `592K`, `1.4G` for resident memory.
pub fn resident(bytes: u64) -> String {
    let bytes = bytes as f64;
    if bytes >= GB {
        format!("{:.1}G", bytes / GB)
    } else if bytes >= MB {
        format!("{:.0}M", bytes / MB)
    } else {
        format!("{:.0}K", bytes / KB)
    }
}

/// `1.24 W`, or `– W` for a block that reports no power.
pub fn watts(watts: Option<f32>) -> String {
    watts.map_or("– W".to_owned(), |watts| {
        format!("{:.2} W", watts.max(0.0))
    })
}

/// `15.4 KB/s`, `25.1 MB/s`.
pub fn rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= GB {
        format!("{:.1} GB/s", bytes_per_sec / GB)
    } else if bytes_per_sec >= MB {
        format!("{:.1} MB/s", bytes_per_sec / MB)
    } else {
        format!("{:.1} KB/s", bytes_per_sec / KB)
    }
}

/// CPU time as `8d10h`, `2h35m` or `3m12s`.
pub fn cpu_time(secs: f64) -> String {
    let secs = secs.max(0.0) as u64;
    let (days, hours, minutes, seconds) = (
        secs / 86_400,
        secs % 86_400 / 3600,
        secs % 3600 / 60,
        secs % 60,
    );
    if days > 0 {
        format!("{days}d{hours:02}h")
    } else if hours > 0 {
        format!("{hours}h{minutes:02}m")
    } else {
        format!("{minutes}m{seconds:02}s")
    }
}

/// Uptime as `39d 08h 43m`.
pub fn uptime(secs: u64) -> String {
    format!(
        "{}d {:02}h {:02}m",
        secs / 86_400,
        secs % 86_400 / 3600,
        secs % 3600 / 60
    )
}

/// `2.1 GHz` from MHz.
pub fn gigahertz(mhz: u32) -> String {
    format!("{:.1} GHz", mhz as f64 / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_cpu_time_like_the_design() {
        assert_eq!(cpu_time(727_000.0), "8d09h");
        assert_eq!(cpu_time(38_458.0), "10h40m");
        assert_eq!(cpu_time(260.0), "4m20s");
    }

    #[test]
    fn formats_resident_memory_compactly() {
        assert_eq!(resident(178 * 1024 * 1024), "178M");
        assert_eq!(resident(592 * 1024), "592K");
        assert_eq!(resident(1_503_238_553), "1.4G");
    }

    #[test]
    fn formats_uptime() {
        assert_eq!(uptime(3_403_949), "39d 09h 32m");
    }
}
