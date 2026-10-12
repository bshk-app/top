//! Development aid behind the `perf` feature: drives the dashboard through
//! scripted scenarios in a real window and prints GPUI's own frame trace.
//!
//! `TOP_PERF=1 cargo run --release -p top --features perf`

use std::time::{Duration, Instant};

use gpui_kit::base::actions::SelectDown;
use gpui_kit::profiler::{FrameEvent, FrameTimingCollector, set_trace_enabled};
use gpui_kit::{AnyWindowHandle, App, Entity, WindowId};

use crate::monitor::Monitor;

/// Wait for real samples and a warm sampler before measuring.
const WARM_UP: Duration = Duration::from_secs(5);
const SCENARIO: Duration = Duration::from_secs(4);
/// Faster than the display, so every frame has work queued.
const TICK: Duration = Duration::from_millis(8);
const FRAME_BUDGET_60: Duration = Duration::from_micros(16_667);

#[derive(Debug, Clone, Copy)]
enum Scenario {
    /// A new sample on every frame: everything that shows data re-renders.
    Data,
    /// Arrow-key navigation through the process table.
    Navigate,
    /// `Window::refresh`, which bypasses every cache: the worst case.
    Refresh,
}

pub fn schedule(window: AnyWindowHandle, monitor: Entity<Monitor>, cx: &mut App) {
    if std::env::var_os("TOP_PERF").is_none() {
        return;
    }
    set_trace_enabled(true);
    cx.spawn(async move |cx| {
        let executor = cx.background_executor().clone();
        executor.timer(WARM_UP).await;
        for scenario in [Scenario::Data, Scenario::Navigate, Scenario::Refresh] {
            let mut collector = FrameTimingCollector::new();
            let mut handlers = Vec::new();
            let started = Instant::now();
            while started.elapsed() < SCENARIO {
                executor.timer(TICK).await;
                let handled = Instant::now();
                cx.update(|cx| match scenario {
                    Scenario::Data => monitor.update(cx, |_, cx| cx.notify()),
                    Scenario::Navigate => {
                        window
                            .update(cx, |_, window, cx| {
                                window.dispatch_action(Box::new(SelectDown), cx)
                            })
                            .ok();
                    }
                    Scenario::Refresh => {
                        window.update(cx, |_, window, _| window.refresh()).ok();
                    }
                });
                handlers.push(handled.elapsed());
            }
            report(
                scenario,
                window.window_id(),
                started.elapsed(),
                collector.collect_unseen(),
                handlers,
            );
        }
        cx.update(|cx| cx.quit());
    })
    .detach();
}

fn report(
    scenario: Scenario,
    window_id: WindowId,
    elapsed: Duration,
    events: Vec<FrameEvent>,
    mut handlers: Vec<Duration>,
) {
    let mut draws: Vec<Duration> = events
        .iter()
        .filter_map(|event| match event {
            FrameEvent::Draw(timing) if timing.window_id == window_id => {
                Some(timing.draw_duration())
            }
            _ => None,
        })
        .collect();
    let presents = events
        .iter()
        .filter(
            |event| matches!(event, FrameEvent::Present(timing) if timing.window_id == window_id),
        )
        .count();
    draws.sort();
    handlers.sort();
    let percentile = |values: &[Duration], p: f64| {
        values
            .get(((values.len() as f64 * p) as usize).min(values.len().saturating_sub(1)))
            .copied()
            .unwrap_or_default()
    };
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    let over_budget = draws.iter().filter(|d| **d > FRAME_BUDGET_60).count();
    eprintln!(
        "perf {scenario:?}: {:.1} fps, {} draws, draw p50 {:.2} ms p95 {:.2} ms max {:.2} ms, \
         {} over 16.7 ms; handler p50 {:.3} ms p95 {:.3} ms",
        presents as f64 / elapsed.as_secs_f64(),
        draws.len(),
        ms(percentile(&draws, 0.5)),
        ms(percentile(&draws, 0.95)),
        ms(draws.last().copied().unwrap_or_default()),
        over_budget,
        ms(percentile(&handlers, 0.5)),
        ms(percentile(&handlers, 0.95)),
    );
}
