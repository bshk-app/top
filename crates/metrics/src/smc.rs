//! System Management Controller keys over the `AppleSMC` user client.

use crate::cf;
use crate::sys;

const KERNEL_INDEX_SMC: u32 = 2;
const CMD_READ_BYTES: u8 = 5;
const CMD_READ_INDEX: u8 = 8;
const CMD_READ_KEYINFO: u8 = 9;
const RESULT_KEY_NOT_FOUND: u8 = 132;
const TYPE_FLT: u32 = u32::from_be_bytes(*b"flt ");

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Version {
    major: u8,
    minor: u8,
    build: u8,
    reserved: u8,
    release: u16,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct PLimitData {
    version: u16,
    length: u16,
    cpu: u32,
    gpu: u32,
    mem: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct KeyInfo {
    data_size: u32,
    data_type: u32,
    data_attributes: u8,
}

/// `SMCKeyData_t`, 80 bytes.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct KeyData {
    key: u32,
    vers: Version,
    p_limit: PLimitData,
    key_info: KeyInfo,
    result: u8,
    status: u8,
    data8: u8,
    data32: u32,
    bytes: [u8; 32],
}

const _: () = assert!(size_of::<KeyData>() == 80);

pub struct Smc {
    connection: cf::io_connect_t,
    cpu_keys: Vec<(u32, KeyInfo)>,
    gpu_keys: Vec<(u32, KeyInfo)>,
    system_power: Option<(u32, KeyInfo)>,
}

impl Smc {
    /// Opens the SMC and indexes its temperature keys once; `None` when the
    /// user client cannot be opened.
    pub fn open() -> Option<Self> {
        let connection = unsafe {
            let service = cf::IOServiceGetMatchingService(
                cf::kIOMainPortDefault,
                cf::IOServiceMatching(c"AppleSMC".as_ptr()),
            );
            if service == 0 {
                return None;
            }
            let mut connection = 0;
            let rc = cf::IOServiceOpen(service, sys::mach_task_self_, 0, &mut connection);
            cf::IOObjectRelease(service);
            if rc != 0 {
                return None;
            }
            connection
        };
        let mut smc = Self {
            connection,
            cpu_keys: Vec::new(),
            gpu_keys: Vec::new(),
            system_power: None,
        };
        smc.index_keys();
        Some(smc)
    }

    /// Mean of the CPU (`Tp*`, `Te*`) and GPU (`Tg*`) die sensors, in °C.
    pub fn temperatures(&self) -> (Option<f32>, Option<f32>) {
        (self.mean(&self.cpu_keys), self.mean(&self.gpu_keys))
    }

    /// Whole-system power from key `PSTR`, in watts.
    pub fn system_power(&self) -> Option<f32> {
        let (key, info) = self.system_power?;
        self.read_float(key, info)
    }

    fn mean(&self, keys: &[(u32, KeyInfo)]) -> Option<f32> {
        let readings: Vec<f32> = keys
            .iter()
            .filter_map(|&(key, info)| self.read_float(key, info))
            .filter(|t| *t > 0.0 && *t <= 150.0)
            .collect();
        (!readings.is_empty()).then(|| readings.iter().sum::<f32>() / readings.len() as f32)
    }

    fn index_keys(&mut self) {
        let count = self
            .key_info(fourcc(b"#KEY"))
            .and_then(|info| self.read_bytes(fourcc(b"#KEY"), info))
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .unwrap_or(0);
        for ix in 0..count {
            let Some(key) = self.key_at(ix) else { continue };
            let name = key.to_be_bytes();
            let is_cpu = name.starts_with(b"Tp") || name.starts_with(b"Te");
            let is_gpu = name.starts_with(b"Tg");
            let is_power = &name == b"PSTR";
            if !(is_cpu || is_gpu || is_power) {
                continue;
            }
            let Some(info) = self
                .key_info(key)
                .filter(|i| i.data_type == TYPE_FLT && i.data_size == 4)
            else {
                continue;
            };
            if is_cpu {
                self.cpu_keys.push((key, info));
            } else if is_gpu {
                self.gpu_keys.push((key, info));
            } else {
                self.system_power = Some((key, info));
            }
        }
    }

    fn call(&self, input: &KeyData) -> Option<KeyData> {
        let mut output = KeyData::default();
        let mut size = size_of::<KeyData>();
        let rc = unsafe {
            cf::IOConnectCallStructMethod(
                self.connection,
                KERNEL_INDEX_SMC,
                (input as *const KeyData).cast(),
                size_of::<KeyData>(),
                (&mut output as *mut KeyData).cast(),
                &mut size,
            )
        };
        (rc == 0 && output.result != RESULT_KEY_NOT_FOUND).then_some(output)
    }

    fn key_at(&self, ix: u32) -> Option<u32> {
        let input = KeyData {
            data8: CMD_READ_INDEX,
            data32: ix,
            ..KeyData::default()
        };
        self.call(&input).map(|out| out.key)
    }

    fn key_info(&self, key: u32) -> Option<KeyInfo> {
        let input = KeyData {
            key,
            data8: CMD_READ_KEYINFO,
            ..KeyData::default()
        };
        self.call(&input).map(|out| out.key_info)
    }

    fn read_bytes(&self, key: u32, info: KeyInfo) -> Option<[u8; 32]> {
        let input = KeyData {
            key,
            key_info: KeyInfo {
                data_size: info.data_size,
                ..KeyInfo::default()
            },
            data8: CMD_READ_BYTES,
            ..KeyData::default()
        };
        self.call(&input).map(|out| out.bytes)
    }

    /// `flt ` values are little-endian on Apple Silicon.
    fn read_float(&self, key: u32, info: KeyInfo) -> Option<f32> {
        self.read_bytes(key, info)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

impl Drop for Smc {
    fn drop(&mut self) {
        unsafe { cf::IOServiceClose(self.connection) };
    }
}

fn fourcc(name: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*name)
}
