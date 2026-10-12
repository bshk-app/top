use std::collections::HashSet;

use crate::snapshot::Volume;
use crate::sys;

/// System volumes macOS mounts under `/Volumes` that a user never browses.
const HIDDEN: [&str; 7] = [
    "Recovery",
    "Preboot",
    "VM",
    "Update",
    "xarts",
    "iSCPreboot",
    "Hardware",
];

/// The startup volume and user-visible volumes under `/Volumes`.
pub fn read() -> Vec<Volume> {
    let mut buffer: *mut sys::statfs = std::ptr::null_mut();
    let count = unsafe { sys::getmntinfo_r_np(&mut buffer, sys::MNT_NOWAIT) };
    if count <= 0 || buffer.is_null() {
        return Vec::new();
    }
    let mounts = unsafe { std::slice::from_raw_parts(buffer, count as usize) };
    let mut seen = HashSet::new();
    let volumes = mounts
        .iter()
        .filter_map(|m| {
            let mount_point = sys::c_array_to_string(&m.f_mntonname);
            let name = match mount_point.as_str() {
                "/" => "Macintosh HD".to_owned(),
                path => {
                    let name = path.strip_prefix("/Volumes/")?;
                    if name.contains('/') || HIDDEN.contains(&name) {
                        return None;
                    }
                    name.to_owned()
                }
            };
            if !seen.insert(sys::c_array_to_string(&m.f_mntfromname)) {
                return None;
            }
            let block = m.f_bsize as u64;
            Some(Volume {
                name,
                mount_point,
                total: m.f_blocks * block,
                available: m.f_bavail * block,
            })
        })
        .filter(|v| v.total > 0)
        .collect();
    unsafe { sys::free(buffer.cast()) };
    volumes
}

#[cfg(test)]
mod tests {
    #[test]
    fn lists_the_startup_volume() {
        let volumes = super::read();
        assert!(volumes.iter().any(|v| v.mount_point == "/" && v.total > 0));
    }
}
