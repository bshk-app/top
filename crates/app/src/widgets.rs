//! Presentation pieces shared by the dashboard, the menu bar popover and the widget.

use gpui_kit::component::chart::AreaChart;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::{
    App, Div, ElementId, IntoElement, ParentElement as _, SharedString, Styled as _, div, relative,
};

/// Text in the theme's monospace face, for numbers and identifiers.
pub fn mono(cx: &App) -> Div {
    div().font_family(cx.theme().mono_font_family.clone())
}

/// A raised region: the group-box surface with a hairline boundary.
pub fn card(cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .bg(theme.group_box)
        .border_1()
        .border_color(theme.border)
        .rounded(theme.radius_lg)
}

/// The live/paused dot shared by every surface.
pub fn live_dot(live: bool, cx: &App) -> Div {
    let theme = cx.theme();
    div().flex_shrink_0().size_1p5().rounded_full().bg(if live {
        theme.success
    } else {
        theme.muted_foreground
    })
}

/// `Apple M1 Max` → `M1 Max`.
pub fn short_chip_name(name: &str) -> String {
    name.strip_prefix("Apple ").unwrap_or(name).to_owned()
}

/// A card's mono caption: what it shows on the leading edge, context on the trailing edge.
pub fn card_header(
    label: impl Into<SharedString>,
    side: impl Into<SharedString>,
    cx: &App,
) -> impl IntoElement {
    mono(cx)
        .flex()
        .justify_between()
        .gap_3()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(label.into())
        .child(div().truncate().child(side.into()))
}

/// A 0–100 series drawn edge to edge with no axes, filling from the left
/// until `capacity` samples have arrived.
pub struct Sparkline {
    id: ElementId,
    series: Vec<f32>,
    capacity: usize,
}

impl Sparkline {
    pub fn new(id: impl Into<ElementId>, series: Vec<f32>, capacity: usize) -> Self {
        Self {
            id: id.into(),
            series,
            capacity,
        }
    }

    pub fn render(self, cx: &App) -> Div {
        let color = cx.theme().primary;
        let points: Vec<(SharedString, f32)> = self
            .series
            .into_iter()
            .enumerate()
            .map(|(ix, value)| (ix.to_string().into(), value.clamp(0.0, 100.0)))
            .collect();
        div().w_full().child(
            AreaChart::new(points)
                .id(self.id)
                .x(|(ix, _)| ix.clone())
                .y(|(_, value)| *value)
                .stroke(color)
                .fill(color.opacity(0.16))
                .linear()
                .x_axis(false)
                .grid(false)
                .y_domain(0.0, 100.0)
                .y_padding(2.0, 0.0)
                .point_count(self.capacity)
                .interactive(false)
                .appear(false),
        )
    }
}

/// Vertical bars for fractions 0–1, newest on the trailing edge, with empty
/// slots reserved until `capacity` values exist.
///
/// Drawn from plain boxes rather than `BarChart` because the scale is fixed by
/// the caller (a watt ceiling), while `BarChart` fits its domain to the data.
pub fn bar_strip(fractions: &[f32], capacity: usize, cx: &App) -> Div {
    let color = cx.theme().primary;
    let empty = capacity.saturating_sub(fractions.len());
    let bar = |fraction: f32| {
        div()
            .flex_1()
            .min_h_px()
            .rounded_xs()
            .bg(color)
            .h(relative(fraction.clamp(0.02, 1.0)))
    };
    h_flex()
        .items_end()
        .gap_0p5()
        .children((0..empty).map(|_| div().flex_1()))
        .children(fractions.iter().map(|&f| bar(f)))
}

/// Perf builds can leave regions out (`TOP_PERF_HIDE=tiles,header`) to
/// attribute frame time; other builds always show everything.
pub fn hidden(region: &str) -> bool {
    cfg!(feature = "perf")
        && std::env::var("TOP_PERF_HIDE").is_ok_and(|hide| hide.split(',').any(|r| r == region))
}
