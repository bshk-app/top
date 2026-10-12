use std::time::Instant;

use crate::snapshot::Throughput;
use crate::sys;

/// 64-bit byte counters summed over non-loopback interfaces; rates come from deltas.
#[derive(Default)]
pub struct NetworkCounters {
    previous: Option<(Instant, u64, u64)>,
}

impl NetworkCounters {
    /// `read` is received bytes per second, `write` is sent.
    pub fn sample(&mut self) -> Throughput {
        let now = Instant::now();
        let Some((received, sent)) = read_totals() else {
            return Throughput::default();
        };
        let rates = match self.previous {
            Some((at, rx, tx)) => {
                let secs = now.duration_since(at).as_secs_f64().max(1e-3);
                Throughput {
                    read: received.saturating_sub(rx) as f64 / secs,
                    write: sent.saturating_sub(tx) as f64 / secs,
                }
            }
            None => Throughput::default(),
        };
        self.previous = Some((now, received, sent));
        rates
    }
}

// `struct if_msghdr2` from <net/if.h>: msglen u16 @0, type u8 @3, flags i32 @8,
// then `struct if_data64` @32 with ibytes u64 @+64 and obytes u64 @+72.
const MSG_LEN: usize = 0;
const MSG_TYPE: usize = 3;
const MSG_FLAGS: usize = 8;
const IBYTES: usize = 32 + 64;
const OBYTES: usize = 32 + 72;

fn read_totals() -> Option<(u64, u64)> {
    let mut mib = [sys::CTL_NET, sys::PF_ROUTE, 0, 0, sys::NET_RT_IFLIST2, 0];
    let table = sys::sysctl_table(&mut mib)?;
    let (mut received, mut sent) = (0u64, 0u64);
    let mut at = 0;
    while at + OBYTES + 8 <= table.len() {
        let msg = &table[at..];
        let len = u16::from_ne_bytes([msg[MSG_LEN], msg[MSG_LEN + 1]]) as usize;
        if len == 0 {
            break;
        }
        let flags = i32::from_ne_bytes(msg[MSG_FLAGS..MSG_FLAGS + 4].try_into().ok()?);
        if msg[MSG_TYPE] == sys::RTM_IFINFO2 && flags & sys::IFF_LOOPBACK == 0 {
            received += u64::from_ne_bytes(msg[IBYTES..IBYTES + 8].try_into().ok()?);
            sent += u64::from_ne_bytes(msg[OBYTES..OBYTES + 8].try_into().ok()?);
        }
        at += len;
    }
    Some((received, sent))
}
