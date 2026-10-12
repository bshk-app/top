//! Prints two samples one second apart: `cargo run -p top-metrics --example dump`.

fn main() {
    let mut sampler = top_metrics::Sampler::new();
    println!("{:?}", sampler.chip());
    println!("ioreport: {}", sampler.has_ioreport());
    std::thread::sleep(std::time::Duration::from_secs(1));
    let mut snapshot = sampler.sample();
    snapshot.processes.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
    let processes = std::mem::take(&mut snapshot.processes);
    println!("{snapshot:#?}");
    println!("{} processes; top by CPU:", processes.len());
    for p in processes.iter().take(8) {
        println!(
            "{:>6} {:<14} cpu {:>5.1}% gpu {:>5.1}% mem {:>4.1}% res {:>6} MB time {:>9.1}s {}",
            p.pid,
            p.user,
            p.cpu,
            p.gpu,
            p.memory,
            p.resident / 1_048_576,
            p.cpu_time,
            p.command
        );
    }
    let mut by_gpu: Vec<_> = processes.iter().filter(|p| p.gpu > 0.0).collect();
    by_gpu.sort_by(|a, b| b.gpu.total_cmp(&a.gpu));
    for p in by_gpu.iter().take(3) {
        println!("gpu {:>5.1}% {}", p.gpu, p.command);
    }
}
