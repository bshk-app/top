//! The middle row: per-core load, power and system facts.

use gpui_kit::component::progress::Progress;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    App, Context, Div, Entity, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, Window, div, prelude::FluentBuilder as _, relative, rems,
};
use top_metrics::CoreKind;

use crate::format;
use crate::monitor::Monitor;
use crate::tiles::value_with_unit;
use crate::widgets::{bar_strip, card, card_header};

/// The row's height in rem; the dashboard lays the cached view out with it.
pub const HEIGHT: f32 = 19.;
/// Power bars span this many watts until a higher peak is seen.
const POWER_SCALE_FLOOR: f32 = 80.0;
const POWER_BARS: usize = 40;
const VOLUMES_SHOWN: usize = 3;

/// Rendered as a cached view: it repaints only when a sample arrives.
pub struct Vitals {
    monitor: Entity<Monitor>,
    _observe: Subscription,
}

impl Vitals {
    pub fn new(monitor: Entity<Monitor>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&monitor, |_, _, cx| cx.notify());
        Self {
            monitor,
            _observe: observe,
        }
    }

    fn render_cores(&self, cx: &App) -> Div {
        let snapshot = self.monitor.read(cx).snapshot();
        let theme = cx.theme();
        let mut efficiency = 0;
        let mut performance = 0;
        let cores = snapshot.cpu.cores.iter().map(|core| {
            let name = match core.kind {
                CoreKind::Efficiency => {
                    efficiency += 1;
                    format!("E{efficiency}")
                }
                CoreKind::Performance => {
                    performance += 1;
                    format!("P{performance}")
                }
            };
            v_flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap_1p5()
                .child(div().text_xs().child(format!("{:.0}", core.usage)))
                .child(
                    div()
                        .flex_1()
                        .w_full()
                        .flex()
                        .items_end()
                        .overflow_hidden()
                        .rounded(theme.radius / 2.)
                        .bg(theme.accent)
                        .border_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .w_full()
                                .h(relative(core.usage.clamp(0.0, 100.0) / 100.0))
                                .bg(theme.primary)
                                .when(core.kind == CoreKind::Efficiency, |this| this.opacity(0.6)),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(name),
                )
        });

        card(cx)
            .p_3p5()
            .gap_3()
            .child(card_header(
                "Cores",
                format!(
                    "E {} · P {}",
                    format::gigahertz(snapshot.cpu.efficiency_mhz),
                    format::gigahertz(snapshot.cpu.performance_mhz)
                ),
                cx,
            ))
            .child(h_flex().items_stretch().gap_2().flex_1().children(cores))
    }

    fn render_power(&self, cx: &App) -> Div {
        let monitor = self.monitor.read(cx);
        let power = monitor.snapshot().power;
        let theme = cx.theme();
        let scale = monitor.power_peak().max(POWER_SCALE_FLOOR);
        let bars: Vec<f32> = monitor
            .history()
            .power
            .iter()
            .rev()
            .take(POWER_BARS)
            .rev()
            .map(|w| w / scale)
            .collect();
        // The rest of the system is known only when every block is.
        let rest = [power.cpu, power.gpu, power.ane, power.dram]
            .into_iter()
            .sum::<Option<f32>>()
            .map(|blocks| power.system - blocks);
        let parts = [
            ("CPU", power.cpu),
            ("GPU", power.gpu),
            ("ANE", power.ane),
            ("DRAM", power.dram),
            ("System", rest),
        ];

        card(cx)
            .p_3p5()
            .gap_2p5()
            .child(card_header(
                "Power",
                format!(
                    "avg {:.1} · max {:.1} W",
                    monitor.power_average(),
                    monitor.power_peak()
                ),
                cx,
            ))
            .child(value_with_unit(
                format!("{:.1}", power.system),
                "W total",
                cx,
            ))
            .child(bar_strip(&bars, POWER_BARS, cx).h_11())
            .child(
                div()
                    .grid()
                    .grid_cols(5)
                    .gap_1()
                    .pt_2p5()
                    .border_t_1()
                    .border_color(theme.border)
                    .text_xs()
                    .children(parts.map(|(label, watts)| {
                        v_flex()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().text_color(theme.muted_foreground).child(label))
                            .child(div().truncate().child(format::watts(watts)))
                    })),
            )
    }

    fn render_system(&self, cx: &App) -> Div {
        let snapshot = self.monitor.read(cx).snapshot();
        let theme = cx.theme();
        let battery = snapshot.battery.map_or("No battery".to_owned(), |b| {
            format!(
                "{}% · {}",
                b.percent,
                if b.on_ac { "AC" } else { "Battery" }
            )
        });
        let rows = [
            ("Thermals", snapshot.thermal.label().to_owned()),
            ("Battery", battery),
            (
                "Uptime",
                match snapshot.uptime_secs {
                    0 => "–".to_owned(),
                    secs => format::uptime(secs),
                },
            ),
            (
                "Network",
                format!(
                    "↑ {}  ↓ {}",
                    format::rate(snapshot.network.write),
                    format::rate(snapshot.network.read)
                ),
            ),
            (
                "Disk I/O",
                format!(
                    "R {}  W {}",
                    format::rate(snapshot.disk.read),
                    format::rate(snapshot.disk.write)
                ),
            ),
        ];
        let volumes = snapshot.volumes.iter().take(VOLUMES_SHOWN).map(|volume| {
            let used = volume.total.saturating_sub(volume.available);
            let pct = used as f32 / volume.total.max(1) as f32 * 100.0;
            v_flex()
                .gap_1()
                .child(
                    h_flex()
                        .justify_between()
                        .gap_3()
                        .child(div().truncate().child(volume.name.clone()))
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_color(theme.muted_foreground)
                                .child(format!(
                                    "{} / {}",
                                    format::capacity(used),
                                    format::capacity(volume.total)
                                )),
                        ),
                )
                .child(
                    Progress::new(SharedString::from(format!("volume-{}", volume.mount_point)))
                        .xsmall()
                        .value(pct),
                )
        });

        card(cx)
            .p_3p5()
            .gap_2()
            .text_xs()
            .child(card_header(
                "System",
                concat!("v", env!("CARGO_PKG_VERSION")),
                cx,
            ))
            .children(rows.map(|(label, value)| {
                h_flex()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(theme.muted_foreground)
                            .child(label),
                    )
                    .child(div().min_w_0().truncate().child(value))
            }))
            .child(
                v_flex()
                    .gap_2()
                    .mt_0p5()
                    .pt_2p5()
                    .border_t_1()
                    .border_color(theme.border)
                    .children(volumes),
            )
    }
}

impl Render for Vitals {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A zero basis shares the row by the grow factors alone, so changing
        // text never changes a card's width.
        let share = |card: Div, grow: f32| card.flex_grow(grow).flex_basis(relative(0.)).min_w_0();
        h_flex()
            .size_full()
            .items_stretch()
            .gap(rems(0.75))
            .child(share(self.render_cores(cx), 1.25))
            .child(share(self.render_power(cx), 1.0))
            .child(share(self.render_system(cx), 1.1))
    }
}
