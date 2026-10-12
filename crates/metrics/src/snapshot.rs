/// Static facts about the machine, read once.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ChipInfo {
    /// Marketing name, for example `Apple M1 Max`.
    pub name: String,
    pub efficiency_cores: u32,
    pub performance_cores: u32,
    pub gpu_cores: u32,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreKind {
    Efficiency,
    Performance,
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct CoreUsage {
    pub kind: CoreKind,
    /// Busy share of the interval, 0–100.
    pub usage: f32,
}

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Cpu {
    /// Mean busy share across all cores, 0–100.
    pub usage: f32,
    pub cores: Vec<CoreUsage>,
    pub efficiency_mhz: u32,
    pub performance_mhz: u32,
}

#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct Gpu {
    /// Active residency, 0–100.
    pub usage: f32,
    pub mhz: u32,
}

/// Power draw in watts, averaged over the sample interval. A block is `None`
/// without IOReport, and CPU, ANE and DRAM also while their energy counters
/// are dead, as macOS 27 betas leave them.
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct Power {
    pub cpu: Option<f32>,
    pub gpu: Option<f32>,
    pub ane: Option<f32>,
    pub dram: Option<f32>,
    /// Whole-system draw when the SMC reports it, otherwise the sum of the
    /// measured blocks.
    pub system: f32,
}

#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct Temperatures {
    pub cpu: Option<f32>,
    pub gpu: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ThermalPressure {
    #[default]
    Nominal,
    Moderate,
    Heavy,
    Trapping,
    Sleeping,
}

impl ThermalPressure {
    pub fn label(self) -> &'static str {
        match self {
            Self::Nominal => "Nominal",
            Self::Moderate => "Moderate",
            Self::Heavy => "Heavy",
            Self::Trapping => "Trapping",
            Self::Sleeping => "Sleeping",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct Memory {
    pub used: u64,
    pub total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
}

/// Byte rates per second.
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct Throughput {
    pub read: f64,
    pub write: f64,
}

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Volume {
    pub name: String,
    pub mount_point: String,
    pub total: u64,
    pub available: u64,
}

#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct Battery {
    pub percent: u32,
    pub on_ac: bool,
}

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Process {
    pub pid: i32,
    /// Start time in microseconds since the epoch; with `pid`, the process identity.
    pub started_at: u64,
    pub user: String,
    pub command: String,
    /// Share of one core, so a busy multi-threaded process can exceed 100.
    pub cpu: f32,
    /// Share of the GPU's busy time, 0–100.
    pub gpu: f32,
    /// Resident memory as a share of physical memory, 0–100.
    pub memory: f32,
    pub resident: u64,
    /// Total CPU time consumed, in seconds.
    pub cpu_time: f64,
}

/// Everything measured over one sample interval.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct Snapshot {
    pub cpu: Cpu,
    pub gpu: Gpu,
    /// Neural Engine load, 0–100, estimated from its power draw: Apple
    /// exposes no occupancy counter. `None` without a power reading.
    pub ane: Option<f32>,
    /// Bytes per second the Neural Engine moved through memory; `None` where
    /// the chip has no such counter.
    pub ane_traffic: Option<f64>,
    pub power: Power,
    pub temperatures: Temperatures,
    pub thermal: ThermalPressure,
    pub memory: Memory,
    /// `read` is received, `write` is sent.
    pub network: Throughput,
    pub disk: Throughput,
    pub volumes: Vec<Volume>,
    pub battery: Option<Battery>,
    pub uptime_secs: u64,
    pub processes: Vec<Process>,
}
