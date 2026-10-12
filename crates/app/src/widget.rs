use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    Context, Entity, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Render, Styled as _, Subscription, Window, div,
};

use crate::layout::TileLayout;
use crate::monitor::{HISTORY_LEN, Monitor};
use crate::tiles::{TileReading, series};
use crate::widgets::{Sparkline, live_dot, mono, short_chip_name};
use crate::windows;

/// A small always-on-top card with the four headline series in the user's
/// tile order. Drag it anywhere; the close button or Window ▸ Floating Widget hides it.
pub struct FloatingWidget {
    monitor: Entity<Monitor>,
    layout: Entity<TileLayout>,
    _subscriptions: Vec<Subscription>,
}

impl FloatingWidget {
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
}

impl Render for FloatingWidget {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let monitor = self.monitor.read(cx);
        let snapshot = monitor.snapshot();
        let theme = cx.theme();
        let battery = snapshot.battery.map_or("No battery".to_owned(), |b| {
            format!("{}% {}", b.percent, if b.on_ac { "AC" } else { "Batt" })
        });
        let temperature = snapshot
            .temperatures
            .cpu
            .map_or("–".to_owned(), |t| format!("{t:.0}°C"));
        let tiles = self.layout.read(cx).order().iter().map(|&kind| {
            let reading = TileReading::read(kind, monitor);
            let series = series(kind, monitor.history()).iter().copied().collect();
            v_flex()
                .min_w_0()
                .overflow_hidden()
                .rounded(theme.radius)
                .bg(theme.accent)
                .border_1()
                .border_color(theme.border)
                .child(
                    h_flex()
                        .items_baseline()
                        .justify_between()
                        .pt_2()
                        .px_2()
                        .child(
                            div()
                                .text_color(theme.muted_foreground)
                                .child(reading.label),
                        )
                        .child(
                            h_flex()
                                .items_baseline()
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(reading.value),
                                )
                                .child(
                                    div().text_color(theme.muted_foreground).child(reading.unit),
                                ),
                        ),
                )
                .child(
                    Sparkline::new(kind.key(), series, HISTORY_LEN)
                        .render(cx)
                        .mt_1()
                        .h_9(),
                )
        });

        mono(cx)
            .id("floating-widget")
            .size_full()
            .flex()
            .flex_col()
            .gap_2p5()
            .p_3()
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .border_1()
            .border_color(theme.border)
            .rounded(theme.radius_lg)
            .text_xs()
            .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
            .child(
                h_flex()
                    .gap_2()
                    .text_color(theme.muted_foreground)
                    .child(live_dot(monitor.is_live() && !monitor.is_frozen(), cx))
                    .child(
                        monitor
                            .chip()
                            .map(|c| short_chip_name(&c.name))
                            .unwrap_or_default(),
                    )
                    .child(div().flex_1())
                    .child(format!("{} ms", monitor.interval().millis()))
                    .child(
                        // Pressing the button must not start a window drag.
                        div()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(
                                Button::new("close-widget")
                                    .xsmall()
                                    .ghost()
                                    .icon(IconName::Close)
                                    .tooltip("Close widget")
                                    .on_click(|_, window, cx| windows::close_widget(window, cx)),
                            ),
                    ),
            )
            .child(div().grid().grid_cols(2).gap_2().children(tiles))
            .child(
                h_flex()
                    .justify_between()
                    .text_color(theme.muted_foreground)
                    .child(format!("{:.1} W", snapshot.power.system))
                    .child(format!("{temperature} · {}", snapshot.thermal.label()))
                    .child(battery),
            )
    }
}
