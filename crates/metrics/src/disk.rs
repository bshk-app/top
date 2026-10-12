use std::time::Instant;

use crate::cf;
use crate::snapshot::Throughput;

/// Read/write byte counters of every block storage driver; rates come from deltas.
#[derive(Default)]
pub struct DiskCounters {
    previous: Option<(Instant, u64, u64)>,
}

impl DiskCounters {
    pub fn sample(&mut self) -> Throughput {
        let now = Instant::now();
        let (read, written) = read_totals();
        let rates = match self.previous {
            Some((at, r, w)) => {
                let secs = now.duration_since(at).as_secs_f64().max(1e-3);
                Throughput {
                    read: read.saturating_sub(r) as f64 / secs,
                    write: written.saturating_sub(w) as f64 / secs,
                }
            }
            None => Throughput::default(),
        };
        self.previous = Some((now, read, written));
        rates
    }
}

fn read_totals() -> (u64, u64) {
    let (mut read, mut written) = (0u64, 0u64);
    cf::for_each_service("IOBlockStorageDriver", |driver| {
        let Some(stats) = cf::property(driver, "Statistics") else {
            return;
        };
        let stat = |key| cf::number_i64(cf::dict_get(stats.as_ptr(), key)).unwrap_or(0) as u64;
        read += stat("Bytes (Read)");
        written += stat("Bytes (Write)");
    });
    (read, written)
}
