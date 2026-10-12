//! Times `Sampler::sample`: `cargo run --release -p top-metrics --example bench`.
//! Run it under `sample <pid>` to see where the time goes. Set `NO_PROCESSES`
//! to time a sample without the process table, as when no window shows it.

use std::time::{Duration, Instant};

fn main() {
    let mut sampler = top_metrics::Sampler::new();
    if std::env::var_os("NO_PROCESSES").is_some() {
        sampler.set_lists_processes(false);
    }
    let rounds: usize = std::env::var("ROUNDS")
        .ok()
        .and_then(|r| r.parse().ok())
        .unwrap_or(40);
    let mut times: Vec<Duration> = (0..rounds)
        .map(|_| {
            std::thread::sleep(Duration::from_millis(100));
            let started = Instant::now();
            std::hint::black_box(sampler.sample());
            started.elapsed()
        })
        .collect();
    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    println!(
        "sample: p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms over {rounds} rounds",
        ms(times[times.len() / 2]),
        ms(times[times.len() * 95 / 100]),
        ms(times[times.len() - 1]),
    );
}
