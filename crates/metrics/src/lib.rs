//! Apple Silicon system metrics, sampled the way mactop does, through raw
//! FFI to IOKit, CoreFoundation, libIOReport and libSystem. No crate
//! dependencies and no root: IOReport, the SMC, libproc and sysctl are all
//! readable by an ordinary user outside an app sandbox.
#![cfg(target_os = "macos")]

mod cf;
mod cpu;
mod disk;
mod ioreport;
mod memory;
mod network;
mod process;
mod smc;
mod snapshot;
mod sys;
mod system;
mod volumes;

use std::time::{Duration, Instant};

pub use process::{Signal, SignalError, is_running, send_signal};
pub use snapshot::*;

/// mactop's ANE scale: the engine's power draw at full load, in watts. A
/// guess inherited from asitop; nothing measures the real figure.
pub const ANE_FULL_LOAD_WATTS: f32 = 8.0;

/// Die temperatures change slowly, while reading every sensor costs one SMC
/// call each (about 8 ms for the 41 sensors of an M1 Max); reuse a reading
/// for this long.
const TEMPERATURE_MAX_AGE: Duration = Duration::from_secs(5);

/// Owns every counter whose rate needs a previous reading. Rates cover the
/// time between two `sample` calls; the first call returns zeros for them.
pub struct Sampler {
    chip: ChipInfo,
    cpu: cpu::CpuTicks,
    ioreport: Option<ioreport::IOReport>,
    smc: Option<smc::Smc>,
    temperatures: Option<(Instant, Temperatures)>,
    /// Whether the per-block energy counters read zero; see `energy_counters_dead`.
    energy_dead: bool,
    processes: process::ProcessTable,
    lists_processes: bool,
    network: network::NetworkCounters,
    disk: disk::DiskCounters,
}

impl Sampler {
    /// Opens IOReport and the SMC. Indexing SMC keys takes a few hundred
    /// milliseconds, so construct this off the UI thread.
    pub fn new() -> Self {
        let mut sampler = Self {
            chip: system::chip_info(),
            cpu: cpu::CpuTicks::default(),
            ioreport: ioreport::IOReport::new(),
            smc: smc::Smc::open(),
            temperatures: None,
            energy_dead: false,
            processes: process::ProcessTable::new(),
            lists_processes: true,
            network: network::NetworkCounters::default(),
            disk: disk::DiskCounters::default(),
        };
        // Record baselines so the first visible sample already has rates.
        sampler.sample();
        sampler
    }

    pub fn chip(&self) -> &ChipInfo {
        &self.chip
    }

    /// Whether `sample` lists processes, its most expensive part.
    pub fn lists_processes(&self) -> bool {
        self.lists_processes
    }

    /// Starts or stops listing processes. Starting records a baseline for
    /// their CPU and GPU rates, so wait a moment before the next `sample`.
    pub fn set_lists_processes(&mut self, lists: bool) {
        if lists && !self.lists_processes {
            self.processes.sample(self.chip.memory_bytes);
        }
        self.lists_processes = lists;
    }

    /// Whether power, frequency and GPU readings are available.
    pub fn has_ioreport(&self) -> bool {
        self.ioreport.is_some()
    }

    pub fn sample(&mut self) -> Snapshot {
        let core_usage = self.cpu.sample();
        let cores = cpu::label_cores(&core_usage, self.chip.efficiency_cores);
        let cpu_usage = if core_usage.is_empty() {
            0.0
        } else {
            core_usage.iter().sum::<f32>() / core_usage.len() as f32
        };

        let reading = self.ioreport.as_mut().and_then(|io| io.sample());
        if let Some(reading) = &reading {
            self.energy_dead = energy_counters_dead(&reading.energy, self.energy_dead);
        }
        let energy = reading.map(|reading| reading.energy);
        let live = energy.filter(|_| !self.energy_dead);
        let reading = reading.unwrap_or_default();
        let power = Power {
            cpu: live.and_then(|e| e.cpu),
            gpu: energy.and_then(|e| Some(e.gpu? + e.gpu_sram.unwrap_or(0.0))),
            ane: live.and_then(|e| e.ane),
            dram: live.and_then(|e| e.dram),
            system: 0.0,
        };
        let package: f32 = [power.cpu, power.gpu, power.ane, power.dram]
            .into_iter()
            .flatten()
            .sum();
        let temperatures = self.temperatures();
        let measured_system = self
            .smc
            .as_ref()
            .and_then(|smc| smc.system_power())
            .unwrap_or(0.0);

        Snapshot {
            cpu: Cpu {
                usage: cpu_usage,
                cores,
                efficiency_mhz: reading.efficiency_mhz,
                performance_mhz: reading.performance_mhz,
            },
            gpu: Gpu {
                usage: reading.gpu_usage,
                mhz: reading.gpu_mhz,
            },
            ane: power
                .ane
                .map(|watts| (watts / ANE_FULL_LOAD_WATTS * 100.0).clamp(0.0, 100.0)),
            ane_traffic: reading.ane_traffic,
            power: Power {
                system: measured_system.max(package),
                ..power
            },
            temperatures,
            thermal: system::thermal_pressure(),
            memory: memory::read(self.chip.memory_bytes),
            network: self.network.sample(),
            disk: self.disk.sample(),
            volumes: volumes::read(),
            battery: system::battery(),
            uptime_secs: system::uptime_secs(),
            processes: if self.lists_processes {
                self.processes.sample(self.chip.memory_bytes)
            } else {
                Vec::new()
            },
        }
    }
}

impl Sampler {
    fn temperatures(&mut self) -> Temperatures {
        if let Some((at, temperatures)) = self.temperatures
            && at.elapsed() < TEMPERATURE_MAX_AGE
        {
            return temperatures;
        }
        let (cpu, gpu) = self
            .smc
            .as_ref()
            .map(|smc| smc.temperatures())
            .unwrap_or_default();
        let temperatures = Temperatures { cpu, gpu };
        self.temperatures = Some((Instant::now(), temperatures));
        temperatures
    }
}

/// Whether the per-block energy counters are dead, as macOS 27 betas leave
/// them: on a working system CPU and DRAM never both read zero, since the
/// sampler itself runs on a CPU and DRAM refreshes, while GPU energy still
/// flowing proves the sample arrived. A missing channel proves nothing, and a
/// sample that settles neither way keeps `previous`.
fn energy_counters_dead(energy: &ioreport::Energy, previous: bool) -> bool {
    let reads = |watts: Option<f32>| watts.is_some_and(|watts| watts > 0.0);
    if reads(energy.cpu) || reads(energy.dram) {
        false
    } else if energy.cpu.is_some() && energy.dram.is_some() && reads(energy.gpu) {
        true
    } else {
        previous
    }
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::energy_counters_dead;
    use crate::ioreport::Energy;

    #[test]
    fn spots_dead_energy_counters() {
        let energy = |cpu, gpu, dram| Energy {
            cpu,
            gpu,
            dram,
            ..Energy::default()
        };
        let (zero, gpu) = (Some(0.0), Some(7.5));
        assert!(energy_counters_dead(&energy(zero, gpu, zero), false));
        assert!(
            !energy_counters_dead(&energy(Some(0.4), gpu, Some(0.3)), true),
            "counters came back"
        );
        assert!(
            !energy_counters_dead(&energy(zero, zero, Some(0.3)), true),
            "DRAM alone proves them alive"
        );
        assert!(
            energy_counters_dead(&energy(zero, zero, zero), true),
            "an empty sample keeps the verdict"
        );
        assert!(!energy_counters_dead(&energy(zero, zero, zero), false));
        assert!(
            !energy_counters_dead(&energy(None, gpu, None), false),
            "channels this chip lacks are not dead ones"
        );
    }
}
