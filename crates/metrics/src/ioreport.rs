//! Power, DVFS residency and Neural Engine traffic from the private
//! `libIOReport`, as mactop reads them.
#![allow(non_snake_case)]

use std::ffi::c_void;
use std::ptr;
use std::time::Instant;

use crate::cf::{self, CFDictionaryRef, CFMutableDictionaryRef, CFOwned, CFStringRef, CFTypeRef};

type IOReportSubscriptionRef = *const c_void;

#[link(name = "IOReport")]
unsafe extern "C" {
    fn IOReportCopyChannelsInGroup(
        group: CFStringRef,
        subgroup: CFStringRef,
        a: u64,
        b: u64,
        c: u64,
    ) -> CFDictionaryRef;
    fn IOReportMergeChannels(a: CFDictionaryRef, b: CFDictionaryRef, nil: CFTypeRef);
    fn IOReportCreateSubscription(
        allocator: *const c_void,
        channels: CFMutableDictionaryRef,
        subscribed: *mut CFMutableDictionaryRef,
        id: u64,
        nil: CFTypeRef,
    ) -> IOReportSubscriptionRef;
    fn IOReportCreateSamples(
        subscription: IOReportSubscriptionRef,
        channels: CFMutableDictionaryRef,
        nil: CFTypeRef,
    ) -> CFDictionaryRef;
    fn IOReportCreateSamplesDelta(
        previous: CFDictionaryRef,
        current: CFDictionaryRef,
        nil: CFTypeRef,
    ) -> CFDictionaryRef;
    fn IOReportChannelGetGroup(item: CFDictionaryRef) -> CFStringRef;
    fn IOReportChannelGetSubGroup(item: CFDictionaryRef) -> CFStringRef;
    fn IOReportChannelGetChannelName(item: CFDictionaryRef) -> CFStringRef;
    fn IOReportChannelGetUnitLabel(item: CFDictionaryRef) -> CFStringRef;
    fn IOReportSimpleGetIntegerValue(item: CFDictionaryRef, index: i32) -> i64;
    fn IOReportStateGetCount(item: CFDictionaryRef) -> i32;
    fn IOReportStateGetNameForIndex(item: CFDictionaryRef, index: i32) -> CFStringRef;
    fn IOReportStateGetResidency(item: CFDictionaryRef, index: i32) -> i64;
}

/// Watts per block, averaged over the interval between two samples; `None`
/// for a block with no channel on this chip and OS.
#[derive(Debug, Default, Clone, Copy)]
pub struct Energy {
    pub cpu: Option<f32>,
    pub gpu: Option<f32>,
    pub gpu_sram: Option<f32>,
    pub ane: Option<f32>,
    pub dram: Option<f32>,
}

impl Energy {
    /// Adds an `Energy Model` channel's watts to its block. Exact names
    /// only: sub-component channels would double count.
    fn add(&mut self, name: &str, watts: f32) {
        let block = if name.contains("CPU Energy") {
            &mut self.cpu
        } else if name == "GPU Energy" {
            &mut self.gpu
        } else if name.starts_with("GPU SRAM") {
            &mut self.gpu_sram
        } else if name.starts_with("ANE") {
            &mut self.ane
        } else if name.starts_with("DRAM") {
            &mut self.dram
        } else {
            return;
        };
        *block.get_or_insert(0.0) += watts;
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Reading {
    pub energy: Energy,
    /// Bytes per second the Neural Engine moved through memory; `None`
    /// where the chip has no such counter.
    pub ane_traffic: Option<f64>,
    pub gpu_usage: f32,
    pub gpu_mhz: u32,
    pub efficiency_mhz: u32,
    pub performance_mhz: u32,
}

/// DVFS tables in MHz, from the `pmgr` node's `voltage-states*` properties.
#[derive(Debug, Default)]
struct FrequencyTables {
    efficiency: Vec<u32>,
    performance: Vec<u32>,
    gpu: Vec<u32>,
}

pub struct IOReport {
    subscription: IOReportSubscriptionRef,
    channels: CFOwned,
    /// Released on drop; samples read `channels`.
    _subscribed: Option<CFOwned>,
    previous: Option<(Instant, CFOwned)>,
    tables: FrequencyTables,
}

// The subscription and dictionaries are only touched by the thread that owns the sampler.
unsafe impl Send for IOReport {}

impl IOReport {
    /// Subscribes to energy, GPU and CPU performance-state and memory traffic
    /// channels; `None` when IOReport is unavailable (for example inside an
    /// app sandbox).
    pub fn new() -> Option<Self> {
        unsafe {
            let energy = copy_channels("Energy Model", None)?;
            if let Some(traffic) = copy_channels("AMC Stats", Some("Perf Counters")) {
                IOReportMergeChannels(energy.as_ptr(), traffic.as_ptr(), ptr::null());
            }
            if let Some(gpu) = copy_channels("GPU Stats", Some("GPU Performance States")) {
                IOReportMergeChannels(energy.as_ptr(), gpu.as_ptr(), ptr::null());
            }
            if let Some(cpu) = copy_channels("CPU Stats", Some("CPU Core Performance States")) {
                IOReportMergeChannels(energy.as_ptr(), cpu.as_ptr(), ptr::null());
            }
            let count = cf::CFDictionaryGetCount(energy.as_ptr());
            let channels = CFOwned::new(cf::CFDictionaryCreateMutableCopy(
                cf::kCFAllocatorDefault,
                count,
                energy.as_ptr(),
            ))?;
            let mut subscribed: CFMutableDictionaryRef = ptr::null_mut();
            let subscription = IOReportCreateSubscription(
                ptr::null(),
                channels.as_ptr() as CFMutableDictionaryRef,
                &mut subscribed,
                0,
                ptr::null(),
            );
            if subscription.is_null() {
                return None;
            }
            Some(Self {
                subscription,
                channels,
                _subscribed: CFOwned::new(subscribed),
                previous: None,
                tables: FrequencyTables::read(),
            })
        }
    }

    /// Reads the channels accumulated since the previous call; the first call
    /// only records a baseline.
    pub fn sample(&mut self) -> Option<Reading> {
        let now = Instant::now();
        let current = CFOwned::new(unsafe {
            IOReportCreateSamples(
                self.subscription,
                self.channels.as_ptr() as CFMutableDictionaryRef,
                ptr::null(),
            )
        })?;
        let (at, previous) = self.previous.replace((now, current))?;
        let current = &self.previous.as_ref()?.1;
        let delta = CFOwned::new(unsafe {
            IOReportCreateSamplesDelta(previous.as_ptr(), current.as_ptr(), ptr::null())
        })?;
        let secs = now.duration_since(at).as_secs_f64().max(1e-3);
        Some(self.read_delta(delta.as_ptr(), secs))
    }

    fn read_delta(&self, delta: CFDictionaryRef, secs: f64) -> Reading {
        let mut reading = Reading::default();
        let mut efficiency = ClusterResidency::default();
        let mut performance = ClusterResidency::default();
        let mut ane_bytes: Option<i64> = None;
        for item in cf::array_items(cf::dict_get(delta, "IOReportChannels")) {
            let group = cf::string(unsafe { IOReportChannelGetGroup(item) }).unwrap_or_default();
            let subgroup =
                cf::string(unsafe { IOReportChannelGetSubGroup(item) }).unwrap_or_default();
            let name =
                cf::string(unsafe { IOReportChannelGetChannelName(item) }).unwrap_or_default();
            match (group.as_str(), subgroup.as_str()) {
                ("Energy Model", _) => {
                    let unit = cf::string(unsafe { IOReportChannelGetUnitLabel(item) })
                        .unwrap_or_default();
                    let joules = unsafe { IOReportSimpleGetIntegerValue(item, 0) } as f64
                        / unit_divisor(unit.trim());
                    let watts = (joules / secs) as f32;
                    reading.energy.add(&name, watts);
                }
                ("AMC Stats", _) => {
                    let unit = cf::string(unsafe { IOReportChannelGetUnitLabel(item) })
                        .unwrap_or_default();
                    if is_ane_traffic(&name, unit.trim()) {
                        let bytes = unsafe { IOReportSimpleGetIntegerValue(item, 0) };
                        *ane_bytes.get_or_insert(0) += bytes.max(0);
                    }
                }
                ("GPU Stats", "GPU Performance States") if name == "GPUPH" => {
                    let states = states(item);
                    (reading.gpu_usage, reading.gpu_mhz) = gpu_residency(&states, &self.tables.gpu);
                }
                ("CPU Stats", "CPU Core Performance States") => {
                    let states = states(item);
                    if name.contains("ECPU") {
                        efficiency.add_core(&states, &self.tables.efficiency);
                    } else if name.contains("PCPU") || name.contains("MCPU") {
                        performance.add_core(&states, &self.tables.performance);
                    }
                }
                _ => {}
            }
        }
        reading.ane_traffic = ane_bytes.map(|bytes| bytes as f64 / secs);
        reading.efficiency_mhz = efficiency.mhz(&self.tables.efficiency);
        reading.performance_mhz = performance.mhz(&self.tables.performance);
        reading
    }
}

impl Drop for IOReport {
    fn drop(&mut self) {
        unsafe { cf::CFRelease(self.subscription) };
    }
}

unsafe fn copy_channels(group: &str, subgroup: Option<&str>) -> Option<CFOwned> {
    let group = CFOwned::string(group);
    let subgroup = subgroup.map(CFOwned::string);
    let subgroup_ptr = subgroup.as_ref().map_or(ptr::null(), |s| s.as_ptr());
    CFOwned::new(unsafe { IOReportCopyChannelsInGroup(group.as_ptr(), subgroup_ptr, 0, 0, 0) })
}

/// The Neural Engine's own reads and writes in `AMC Stats`, such as
/// `ANE0 RD`. The `DCS` channels see the same traffic at the DRAM scheduler,
/// and `PMP` repeats these names in `events` that read `INT64_MIN`, so the
/// group, name and unit must all match.
fn is_ane_traffic(name: &str, unit: &str) -> bool {
    unit == "B"
        && name.starts_with("ANE")
        && !name.contains("DCS")
        && (name.ends_with(" RD") || name.ends_with(" WR"))
}

fn unit_divisor(unit: &str) -> f64 {
    match unit {
        "mJ" => 1e3,
        "uJ" => 1e6,
        "nJ" => 1e9,
        _ => 1e6,
    }
}

/// `(state name, residency)` pairs of a state channel.
fn states(item: CFDictionaryRef) -> Vec<(String, i64)> {
    let count = unsafe { IOReportStateGetCount(item) };
    (0..count)
        .map(|ix| unsafe {
            let name = cf::string(IOReportStateGetNameForIndex(item, ix)).unwrap_or_default();
            (name, IOReportStateGetResidency(item, ix))
        })
        .collect()
}

fn is_inactive(state: &str) -> bool {
    matches!(state, "OFF" | "IDLE" | "DOWN")
}

/// Busy share (0–100) and residency-weighted MHz of the GPU.
fn gpu_residency(states: &[(String, i64)], freqs: &[u32]) -> (f32, u32) {
    let total: i64 = states.iter().map(|(_, r)| r).sum();
    let mut active = 0i64;
    let mut weighted = 0f64;
    for (k, (_, residency)) in states
        .iter()
        .filter(|(name, _)| !is_inactive(name))
        .enumerate()
    {
        active += residency;
        if let Some(&mhz) = freqs.get(k) {
            weighted += mhz as f64 * *residency as f64;
        }
    }
    if total <= 0 || active <= 0 {
        return (0.0, 0);
    }
    (
        (active as f64 / total as f64 * 100.0) as f32,
        (weighted / active as f64) as u32,
    )
}

/// Per-core residency-weighted frequency, averaged over a cluster.
#[derive(Default)]
struct ClusterResidency {
    mhz_sum: f64,
    active_cores: u32,
}

impl ClusterResidency {
    fn add_core(&mut self, states: &[(String, i64)], freqs: &[u32]) {
        let Some(offset) = states.iter().position(|(name, _)| !is_inactive(name)) else {
            return;
        };
        let active = &states[offset..states.len().min(offset + freqs.len())];
        let usage: i64 = active.iter().map(|(_, r)| r).sum();
        if usage <= 0 {
            return;
        }
        let mhz: f64 = active
            .iter()
            .zip(freqs)
            .map(|((_, r), &f)| *r as f64 / usage as f64 * f as f64)
            .sum();
        self.mhz_sum += mhz;
        self.active_cores += 1;
    }

    fn mhz(&self, freqs: &[u32]) -> u32 {
        let floor = freqs.first().copied().unwrap_or(0);
        if self.active_cores == 0 {
            return floor;
        }
        ((self.mhz_sum / self.active_cores as f64) as u32).max(floor)
    }
}

impl FrequencyTables {
    fn read() -> Self {
        let mut tables = Self::default();
        cf::for_each_service("AppleARMIODevice", |entry| {
            let mut name = [0 as std::ffi::c_char; 128];
            unsafe { cf::IORegistryEntryGetName(entry, name.as_mut_ptr()) };
            let name = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }.to_string_lossy();
            if name != "pmgr" {
                return;
            }
            let Some(props) = cf::properties(entry) else {
                return;
            };
            let table = |key: &str| {
                cf::data_bytes(cf::dict_get(props.as_ptr(), key))
                    .map(parse_voltage_states)
                    .unwrap_or_default()
            };
            tables.efficiency = table("voltage-states1-sram");
            tables.performance = table("voltage-states5-sram");
            tables.gpu = table("voltage-states9-sram");
            if tables.gpu.is_empty() {
                tables.gpu = table("voltage-states9");
            }
        });
        tables
    }
}

/// Decodes `[u32 frequency][u32 voltage]` pairs, skipping the OFF state.
/// M1–M3 store Hz; M4 and later store kHz.
fn parse_voltage_states(bytes: &[u8]) -> Vec<u32> {
    let raw: Vec<u64> = bytes
        .chunks_exact(8)
        .map(|pair| u32::from_le_bytes(pair[..4].try_into().expect("4 bytes")) as u64)
        .filter(|&f| f > 0)
        .collect();
    let divisor = if raw.iter().any(|&f| f > 100_000_000) {
        1_000_000
    } else {
        1_000
    };
    raw.into_iter().map(|f| (f / divisor) as u32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_hz_and_khz_tables() {
        let hz: Vec<u8> = [0u32, 0, 600_000_000, 1, 2_064_000_000, 1]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(parse_voltage_states(&hz), vec![600, 2064]);
        let khz: Vec<u8> = [1_020_000u32, 1, 4_512_000, 1]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(parse_voltage_states(&khz), vec![1020, 4512]);
    }

    #[test]
    fn weights_gpu_frequency_by_active_residency() {
        let states = vec![
            ("OFF".to_owned(), 50),
            ("P1".to_owned(), 25),
            ("P2".to_owned(), 25),
        ];
        assert_eq!(gpu_residency(&states, &[400, 800]), (50.0, 600));
    }

    #[test]
    fn a_block_without_a_channel_reports_nothing() {
        let mut energy = Energy::default();
        energy.add("ECPU0 CPU Energy", 0.25);
        energy.add("PCPU CPU Energy", 1.0);
        energy.add("GPU Energy", 2.0);
        energy.add("DISP", 0.5);
        assert_eq!(energy.cpu, Some(1.25));
        assert_eq!(energy.gpu, Some(2.0));
        assert_eq!(energy.ane, None, "no ANE channel, not 0 W");
        assert_eq!(energy.dram, None);
    }

    #[test]
    fn counts_only_the_neural_engine_byte_channels() {
        assert!(is_ane_traffic("ANE0 RD", "B"));
        assert!(is_ane_traffic("ANE1 WR", "B"));
        assert!(!is_ane_traffic("ANE0 DCS RD", "B"), "the same bytes again");
        assert!(
            !is_ane_traffic("ANE0 RD+WR", "events"),
            "PMP's uninitialized twin"
        );
        assert!(!is_ane_traffic("GFX RD", "B"));
    }
}
