//! The headline metric tiles: a grid the user arranges by dragging, with a
//! column count that is fixed or follows the window width.

use std::collections::VecDeque;

use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, TestSupportExt as _, Window, div, relative, rems,
};

use crate::format;
use crate::layout::{TileKind, TileLayout, resolve_columns};
use crate::monitor::{HISTORY_LEN, History, Monitor};
use crate::widgets::{Sparkline, card, card_header, mono};

/// Tile geometry in rem, so the grid zooms with the base font.
const TILE_HEIGHT: f32 = 10.;
const MIN_TILE_WIDTH: f32 = 15.;
const GAP: f32 = 0.75;
/// The dashboard's content inset on each side (`p_4`).
const CONTENT_INSET: f32 = 1.;

/// What a tile shows for its metric.
pub struct TileReading {
    pub label: &'static str,
    pub side: String,
    pub value: String,
    pub unit: &'static str,
    pub sub: String,
}

impl TileReading {
    pub fn read(kind: TileKind, monitor: &Monitor) -> Self {
        let snapshot = monitor.snapshot();
        let temperature = |t: Option<f32>| t.map_or("–".to_owned(), |t| format!("{t:.0}°C"));
        let gib = |bytes: u64| bytes as f64 / (1u64 << 30) as f64;
        match kind {
            TileKind::Cpu => Self {
                label: "CPU",
                side: temperature(snapshot.temperatures.cpu),
                value: format!("{:.1}", snapshot.cpu.usage),
                unit: "%",
                sub: format!(
                    "{} cores · E {:.1} / P {:.1} GHz",
                    snapshot.cpu.cores.len(),
                    snapshot.cpu.efficiency_mhz as f64 / 1000.0,
                    snapshot.cpu.performance_mhz as f64 / 1000.0
                ),
            },
            TileKind::Gpu => Self {
                label: "GPU",
                side: temperature(snapshot.temperatures.gpu),
                value: format!("{:.0}", snapshot.gpu.usage),
                unit: "%",
                sub: format!(
                    "{} cores · {} MHz",
                    monitor.chip().map_or(0, |chip| chip.gpu_cores),
                    snapshot.gpu.mhz
                ),
            },
            TileKind::Ane => {
                let scale = if snapshot.ane.is_some() {
                    format!("share of {:.0} W", top_metrics::ANE_FULL_LOAD_WATTS)
                } else {
                    "power not reported".to_owned()
                };
                Self {
                    label: "ANE",
                    side: format::watts(snapshot.power.ane),
                    // Apple exposes no occupancy counter, so the load is an estimate.
                    value: snapshot
                        .ane
                        .map_or("–".to_owned(), |load| format!("≈{load:.0}")),
                    unit: "%",
                    sub: match snapshot.ane_traffic {
                        Some(traffic) => format!("{scale} · {} traffic", format::rate(traffic)),
                        None => scale,
                    },
                }
            }
            TileKind::Memory => {
                let memory = &snapshot.memory;
                Self {
                    label: "Memory",
                    side: format!(
                        "{:.0}%",
                        memory.used as f64 / memory.total.max(1) as f64 * 100.0
                    ),
                    value: format!("{:.2}", gib(memory.used)),
                    unit: "GB",
                    sub: format!(
                        "of {:.0} GB · swap {:.2} / {:.2} GB",
                        gib(memory.total),
                        gib(memory.swap_used),
                        gib(memory.swap_total)
                    ),
                }
            }
        }
    }
}

/// The recent values a tile's sparkline draws, 0–100.
pub fn series(kind: TileKind, history: &History) -> &VecDeque<f32> {
    match kind {
        TileKind::Cpu => &history.cpu,
        TileKind::Gpu => &history.gpu,
        TileKind::Ane => &history.ane,
        TileKind::Memory => &history.memory,
    }
}

/// Columns the grid uses in `window` for `layout`. The dashboard sizes the
/// cached grid from the same answer, so the two cannot disagree.
pub fn columns(layout: &TileLayout, window: &Window) -> usize {
    let rem = window.rem_size();
    let width = window.viewport_size().width - rem * (CONTENT_INSET * 2.);
    resolve_columns(layout.columns(), width, rem * MIN_TILE_WIDTH, rem * GAP)
}

/// Height of the grid for `columns`, in rem-derived pixels.
pub fn grid_height(columns: usize, window: &Window) -> Pixels {
    let rows = TileKind::ALL.len().div_ceil(columns.max(1)) as f32;
    window.rem_size() * (rows * TILE_HEIGHT + (rows - 1.) * GAP)
}

/// The draggable tile grid. Rendered as a cached view: it repaints only for a
/// new sample, a layout change or its own interaction.
pub struct TileGrid {
    monitor: Entity<Monitor>,
    layout: Entity<TileLayout>,
    _subscriptions: Vec<Subscription>,
}

impl TileGrid {
    pub fn new(
        monitor: Entity<Monitor>,
        layout: Entity<TileLayout>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.observe(&monitor, |_, _, cx| cx.notify()),
            cx.observe(&layout, |_, _, cx| cx.notify()),
        ];
        Self {
            monitor,
            layout,
            _subscriptions: subscriptions,
        }
    }

    fn render_tile(
        &self,
        kind: TileKind,
        ix: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let monitor = self.monitor.read(cx);
        let reading = TileReading::read(kind, monitor);
        let series = series(kind, monitor.history()).iter().copied().collect();
        let theme = cx.theme();
        let (is_first, is_last) = (ix == 0, ix + 1 == TileKind::ALL.len());
        let layout = self.layout.clone();
        let dragged = DraggedTile {
            label: reading.label,
            value: format!("{}{}", reading.value, reading.unit).into(),
            kind,
        };

        card(cx)
            .id(("tile", kind as usize))
            .test_support()
            .h(rems(TILE_HEIGHT))
            .min_w_0()
            .overflow_hidden()
            .on_drag(dragged, |dragged, _, _, cx| cx.new(|_| dragged.clone()))
            .drag_over::<DraggedTile>(move |style, dragged, _, cx| {
                if dragged.kind == kind {
                    style
                } else {
                    style.border_color(cx.theme().primary).bg(cx.theme().accent)
                }
            })
            .on_drop(cx.listener(move |this, dragged: &DraggedTile, _, cx| {
                this.layout
                    .update(cx, |layout, cx| layout.move_tile(dragged.kind, kind, cx));
            }))
            .child(
                v_flex()
                    .gap_1()
                    .pt_3p5()
                    .px_3p5()
                    .child(card_header(reading.label, reading.side, cx))
                    .child(value_with_unit(reading.value, reading.unit, cx))
                    .child(
                        mono(cx)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .truncate()
                            .child(reading.sub),
                    ),
            )
            .child(
                Sparkline::new(kind.key(), series, HISTORY_LEN)
                    .render(cx)
                    .mt_2()
                    .flex_1()
                    .min_h_0(),
            )
            .context_menu(move |menu, _, cx| {
                let is_default = layout.read(cx).is_default();
                let (back, forward, reset) = (layout.clone(), layout.clone(), layout.clone());
                menu.item(PopupMenuItem::new("Move left").disabled(is_first).on_click(
                    move |_, _, cx| back.update(cx, |layout, cx| layout.shift_tile(kind, -1, cx)),
                ))
                .item(PopupMenuItem::new("Move right").disabled(is_last).on_click(
                    move |_, _, cx| forward.update(cx, |layout, cx| layout.shift_tile(kind, 1, cx)),
                ))
                .separator()
                .item(
                    PopupMenuItem::new("Reset layout")
                        .disabled(is_default)
                        .on_click(move |_, _, cx| reset.update(cx, |layout, cx| layout.reset(cx))),
                )
            })
    }
}

impl Render for TileGrid {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = self.layout.read(cx);
        let columns = columns(layout, window);
        let order = layout.order().to_vec();
        div()
            .id("tiles")
            .size_full()
            .grid()
            .grid_cols(columns as u16)
            .gap(rems(GAP))
            .children(
                order
                    .into_iter()
                    .enumerate()
                    .map(|(ix, kind)| self.render_tile(kind, ix, cx)),
            )
    }
}

/// A large mono reading with a muted unit, aligned on the baseline.
pub fn value_with_unit(
    value: impl Into<SharedString>,
    unit: impl Into<SharedString>,
    cx: &App,
) -> impl IntoElement {
    mono(cx)
        .flex()
        .items_baseline()
        .gap_1()
        .child(
            div()
                .text_3xl()
                .font_weight(FontWeight::SEMIBOLD)
                .line_height(relative(1.1))
                .child(value.into()),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(unit.into()),
        )
}

/// What follows the pointer while a tile is dragged.
#[derive(Clone)]
struct DraggedTile {
    kind: TileKind,
    label: &'static str,
    value: SharedString,
}

impl Render for DraggedTile {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .gap_2()
            .px_3()
            .py_2()
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .border_1()
            .border_color(theme.primary)
            .rounded(theme.radius)
            .shadow_lg()
            .font_family(theme.mono_font_family.clone())
            .text_xs()
            .child(div().text_color(theme.muted_foreground).child(self.label))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.value.clone()),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{AppContext as _, TestAppContext};
    use top_metrics::{ChipInfo, Snapshot};

    use super::TileReading;
    use crate::layout::TileKind;
    use crate::monitor::Monitor;

    fn ane_tile(snapshot: Snapshot, cx: &mut TestAppContext) -> [String; 3] {
        let monitor = cx.new(|cx| Monitor::fixture(ChipInfo::default(), snapshot, cx));
        cx.read(|cx| {
            let tile = TileReading::read(TileKind::Ane, monitor.read(cx));
            [tile.side, tile.value, tile.sub]
        })
    }

    #[gpui_kit::test]
    fn the_ane_tile_shows_an_estimate_or_nothing(cx: &mut TestAppContext) {
        const GIB: f64 = (1u64 << 30) as f64;
        let mut measured = Snapshot::default();
        measured.power.ane = Some(0.19);
        measured.ane = Some(2.4);
        measured.ane_traffic = Some(2.2 * GIB);
        assert_eq!(
            ane_tile(measured, cx),
            ["0.19 W", "≈2", "share of 8 W · 2.2 GB/s traffic"]
        );

        // Dead energy counters: no power and no load, though traffic still counts.
        let mut dead = Snapshot::default();
        dead.ane_traffic = Some(GIB / 1024.);
        assert_eq!(
            ane_tile(dead, cx),
            ["– W", "–", "power not reported · 1.0 MB/s traffic"]
        );
    }
}
