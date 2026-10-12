use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, FontWeight, Global,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, Window, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, div,
    point, prelude::FluentBuilder as _, px, size,
};

use crate::menu_bar::{MenuBarItem, ShownItems};
use crate::monitor::{Monitor, ProcessDemand};
use crate::status_item::{self, StatusItem};
use crate::widgets::{bar_strip, live_dot, mono};
use crate::{ClosePopover, windows};

const POPOVER_SIZE: (f32, f32) = (320., 624.);
/// Space between the menu bar and the popover, and the popover and a screen edge.
const POPOVER_GAP: f32 = 6.;
const POWER_BARS: usize = 24;

/// The status items in the menu bar and the popover they toggle.
pub struct MenuBar {
    /// In [`MenuBarItem::ALL`] order.
    items: Vec<(MenuBarItem, StatusItem)>,
    shown: Entity<ShownItems>,
    monitor: Entity<Monitor>,
    popover: Option<AnyWindowHandle>,
    _subscriptions: Vec<Subscription>,
}

/// Keeps the status items alive for the life of the app.
struct GlobalMenuBar {
    _menu_bar: Entity<MenuBar>,
}

impl Global for GlobalMenuBar {}

pub fn init(monitor: Entity<Monitor>, cx: &mut App) -> Entity<MenuBar> {
    let menu_bar = cx.new(|cx| {
        let weak = cx.weak_entity();
        let async_cx = cx.to_async();
        status_item::on_click(move |tag| {
            let Some(&item) = usize::try_from(tag)
                .ok()
                .and_then(|ix| MenuBarItem::ALL.get(ix))
            else {
                return;
            };
            // AppKit calls this from its event loop; run the update on GPUI's
            // own turn so it can never re-enter an update in progress.
            let weak = weak.clone();
            async_cx
                .spawn(async move |cx| {
                    weak.update(cx, |menu_bar: &mut MenuBar, cx| {
                        menu_bar.toggle_popover(item, cx)
                    })
                    .ok();
                })
                .detach();
        });
        // The menu bar adds each item left of the existing ones.
        let mut items: Vec<_> = MenuBarItem::ALL
            .into_iter()
            .enumerate()
            .rev()
            .filter_map(|(ix, item)| Some((item, StatusItem::new(ix as isize)?)))
            .collect();
        items.reverse();
        let shown = cx.new(|_| ShownItems::load());
        let menu_bar = MenuBar {
            items,
            _subscriptions: vec![
                cx.observe(&monitor, |menu_bar, _, cx| menu_bar.refresh(cx)),
                cx.observe(&shown, |menu_bar, _, cx| menu_bar.show_items(cx)),
            ],
            shown,
            monitor,
            popover: None,
        };
        menu_bar.show_items(cx);
        menu_bar
    });
    cx.set_global(GlobalMenuBar {
        _menu_bar: menu_bar.clone(),
    });
    menu_bar
}

impl MenuBar {
    fn show_items(&self, cx: &App) {
        let shown = self.shown.read(cx);
        for (item, status_item) in &self.items {
            status_item.set_visible(shown.contains(*item));
        }
        self.refresh(cx);
    }

    /// Redraws the shown items with the latest sample.
    fn refresh(&self, cx: &App) {
        let monitor = self.monitor.read(cx);
        let snapshot = monitor.is_live().then(|| monitor.snapshot().as_ref());
        let shown = self.shown.read(cx);
        for (item, status_item) in &self.items {
            if shown.contains(*item) {
                let columns = item.columns(snapshot);
                let label = item.accessibility_label(&columns);
                status_item.set_content(columns, &label);
            }
        }
    }

    pub fn toggle_popover(&mut self, anchor: MenuBarItem, cx: &mut Context<Self>) {
        if let Some(handle) = self.popover.take()
            && handle
                .update(cx, |_, window, _| window.remove_window())
                .is_ok()
        {
            return;
        }
        self.popover = self.open_popover(anchor, cx);
    }

    /// Opens the popover under `anchor`'s item, kept inside the primary display.
    pub fn open_popover(
        &mut self,
        anchor: MenuBarItem,
        cx: &mut Context<Self>,
    ) -> Option<AnyWindowHandle> {
        let display = cx.primary_display()?.bounds();
        let (width, height) = POPOVER_SIZE;
        let frame = self
            .items
            .iter()
            .find(|(item, _)| *item == anchor)
            .map(|(_, status_item)| status_item.frame())
            .unwrap_or_default();
        // Cocoa measures y up from the primary display's bottom; GPUI measures down from its top.
        let menu_bar_bottom = display.size.height - px(frame.origin.y as f32);
        let anchor_x = px((frame.origin.x + frame.size.width / 2.) as f32) - px(width / 2.);
        let x = anchor_x.clamp(
            display.left() + px(POPOVER_GAP),
            display.right() - px(width + POPOVER_GAP),
        );
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(x, menu_bar_bottom.max(px(24.)) + px(POPOVER_GAP)),
                size(px(width), px(height)),
            ))),
            titlebar: None,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            ..WindowOptions::default()
        };
        let (monitor, shown) = (self.monitor.clone(), self.shown.clone());
        let handle = gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| MenuBarPopover::new(monitor, shown, window, cx))
        })
        .ok()?
        .0;
        cx.activate(true);
        self.popover = Some(handle);
        Some(handle)
    }

    /// Writes each shown item to `<stem>-bar-<item>.png` next to `path`.
    #[cfg(feature = "screenshot")]
    pub fn save_items(&self, path: &std::path::Path, cx: &App) {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("shot");
        for (item, status_item) in &self.items {
            if self.shown.read(cx).contains(*item) {
                let target = path.with_file_name(format!("{stem}-bar-{}.png", item.key()));
                let saved = status_item.save_png(&target);
                eprintln!("screenshot {}: {saved}", target.display());
            }
        }
    }
}

/// The menu bar popover: headline meters, power, thermals and the busiest processes.
pub struct MenuBarPopover {
    monitor: Entity<Monitor>,
    shown: Entity<ShownItems>,
    _processes: ProcessDemand,
    _subscriptions: Vec<Subscription>,
}

impl MenuBarPopover {
    fn new(
        monitor: Entity<Monitor>,
        shown: Entity<ShownItems>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.observe(&monitor, |_, _, cx| cx.notify()),
            cx.observe(&shown, |_, _, cx| cx.notify()),
            // Like a menu, the popover closes as soon as another window takes focus.
            cx.observe_window_activation(window, |_, window, _| {
                if !window.is_window_active() {
                    window.remove_window();
                }
            }),
        ];
        Self {
            _processes: monitor.read(cx).demand_processes(),
            monitor,
            shown,
            _subscriptions: subscriptions,
        }
    }

    fn close(&mut self, _: &ClosePopover, window: &mut Window, _: &mut Context<Self>) {
        window.remove_window();
    }

    fn render_meters(&self, cx: &App) -> impl IntoElement {
        let monitor = self.monitor.read(cx);
        let snapshot = monitor.snapshot();
        let memory = &snapshot.memory;
        let memory_pct = memory.used as f32 / memory.total.max(1) as f32 * 100.0;
        let meters: [(&str, f32, String); 4] = [
            (
                "CPU",
                snapshot.cpu.usage,
                format!("{:.1}%", snapshot.cpu.usage),
            ),
            (
                "GPU",
                snapshot.gpu.usage,
                format!("{:.0}%", snapshot.gpu.usage),
            ),
            (
                "ANE",
                snapshot.ane.unwrap_or(0.0),
                snapshot
                    .ane
                    .map_or("–".to_owned(), |load| format!("≈{load:.0}%")),
            ),
            (
                "MEM",
                memory_pct,
                format!("{:.2} GB", memory.used as f64 / (1u64 << 30) as f64),
            ),
        ];
        let muted = cx.theme().muted_foreground;
        v_flex()
            .gap_2p5()
            .children(meters.map(|(label, pct, value)| {
                h_flex()
                    .gap_2p5()
                    .child(div().w_10().text_color(muted).child(label))
                    .child(
                        Progress::new(SharedString::from(format!("meter-{label}")))
                            .small()
                            .flex_1()
                            .value(pct.clamp(0.0, 100.0)),
                    )
                    .child(div().w_20().text_right().child(value))
            }))
    }

    fn render_power(&self, cx: &App) -> impl IntoElement {
        let monitor = self.monitor.read(cx);
        let theme = cx.theme();
        let scale = monitor.power_peak().max(80.0);
        let bars: Vec<f32> = monitor
            .history()
            .power
            .iter()
            .rev()
            .take(POWER_BARS)
            .rev()
            .map(|w| w / scale)
            .collect();
        h_flex()
            .items_end()
            .gap_3()
            .pt_3()
            .border_t_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .gap_0p5()
                    .child(div().text_color(theme.muted_foreground).child("Power"))
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{:.1} W", monitor.snapshot().power.system)),
                    ),
            )
            .child(bar_strip(&bars, POWER_BARS, cx).flex_1().h_9())
    }

    fn render_facts(&self, cx: &App) -> impl IntoElement {
        let snapshot = self.monitor.read(cx).snapshot();
        let muted = cx.theme().muted_foreground;
        let facts = [
            ("Thermals", snapshot.thermal.label().to_owned()),
            (
                "CPU temp",
                snapshot
                    .temperatures
                    .cpu
                    .map_or("–".to_owned(), |t| format!("{t:.0}°C")),
            ),
            (
                "Battery",
                snapshot.battery.map_or("None".to_owned(), |b| {
                    format!(
                        "{}% · {}",
                        b.percent,
                        if b.on_ac { "AC" } else { "Battery" }
                    )
                }),
            ),
        ];
        div()
            .grid()
            .grid_cols(3)
            .gap_2()
            .children(facts.map(|(label, value)| {
                v_flex()
                    .gap_0p5()
                    .child(div().text_color(muted).child(label))
                    .child(value)
            }))
    }

    fn render_processes(&self, cx: &App) -> impl IntoElement {
        let monitor = self.monitor.read(cx);
        let theme = cx.theme();
        v_flex()
            .gap_1p5()
            .pt_3()
            .border_t_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .justify_between()
                    .text_color(theme.muted_foreground)
                    .child("Top processes")
                    .child("CPU"),
            )
            .children(monitor.busiest().map(|process| {
                h_flex()
                    .justify_between()
                    .gap_3()
                    .child(div().truncate().child(process.command.clone()))
                    .child(div().flex_shrink_0().child(format!("{:.1}%", process.cpu)))
            }))
    }

    fn render_menu_bar_items(&self, cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let shown_items = self.shown.read(cx);
        let last = shown_items.len() == 1;
        let checkboxes = MenuBarItem::ALL.map(|item| {
            let checked = shown_items.contains(item);
            let shown = self.shown.clone();
            Checkbox::new(item.key())
                .label(item.label())
                .checked(checked)
                .disabled(checked && last)
                .on_click(move |&checked, _, cx| {
                    shown.update(cx, |shown, cx| {
                        if shown.set(item, checked) {
                            cx.notify();
                        }
                    })
                })
        });
        v_flex()
            .gap_1p5()
            .pt_3()
            .border_t_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Menu bar"),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(3)
                    .gap_x_3()
                    .gap_y_1p5()
                    .text_xs()
                    .children(checkboxes),
            )
    }

    fn render_actions(&self, cx: &App) -> impl IntoElement {
        let frozen = self.monitor.read(cx).is_frozen();
        let monitor = self.monitor.clone();
        h_flex()
            .gap_2()
            .pt_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("open-top")
                    .small()
                    .primary()
                    .label("Open Top")
                    .on_click(|_, window, cx| {
                        window.remove_window();
                        windows::show_dashboard(cx);
                    }),
            )
            .child(
                Button::new("freeze")
                    .small()
                    .ghost()
                    .label(if frozen { "Resume" } else { "Freeze" })
                    .on_click(move |_, _, cx| {
                        monitor.update(cx, |monitor, cx| monitor.toggle_freeze(cx))
                    }),
            )
            .child(div().flex_1())
            .child(
                Button::new("quit")
                    .small()
                    .ghost()
                    .label("Quit")
                    .on_click(|_, _, cx| cx.quit()),
            )
    }
}

impl Render for MenuBarPopover {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let monitor = self.monitor.read(cx);
        let theme = cx.theme();
        v_flex()
            .key_context("MenuBarPopover")
            .on_action(cx.listener(Self::close))
            .size_full()
            .p_3p5()
            .gap_3()
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .border_1()
            .border_color(theme.border)
            .rounded(theme.radius_lg)
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Top"),
                    )
                    .when_some(monitor.chip(), |this, chip| {
                        this.child(
                            mono(cx)
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(chip.name.clone()),
                        )
                    })
                    .child(div().flex_1())
                    .child(live_dot(monitor.is_live() && !monitor.is_frozen(), cx).size_2()),
            )
            .child(
                mono(cx)
                    .flex()
                    .flex_col()
                    .gap_3()
                    .text_xs()
                    .child(self.render_meters(cx))
                    .child(self.render_power(cx))
                    .child(self.render_facts(cx))
                    .child(self.render_processes(cx)),
            )
            .child(self.render_menu_bar_items(cx))
            .child(self.render_actions(cx))
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{AppContext as _, Bounds, TestAppContext, WindowBounds, WindowOptions, size};
    use top_metrics::{ChipInfo, Snapshot};

    use super::{MenuBarPopover, POPOVER_SIZE};
    use crate::menu_bar::{MenuBarItem, ShownItems};
    use crate::monitor::Monitor;

    #[gpui_kit::test]
    fn checkboxes_choose_the_menu_bar_items(cx: &mut TestAppContext) {
        let (handle, monitor, shown) = cx.update(|cx| {
            gpui_kit::init(cx);
            crate::theme::init(cx);
            let monitor =
                cx.new(|cx| Monitor::fixture(ChipInfo::default(), Snapshot::default(), cx));
            let shown = cx.new(|_| ShownItems::in_memory());
            let (width, height) = POPOVER_SIZE;
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(width.into(), height.into()),
                ))),
                ..Default::default()
            };
            let (for_view, shown_for_view) = (monitor.clone(), shown.clone());
            let (handle, _) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| MenuBarPopover::new(for_view, shown_for_view, window, cx))
            })
            .expect("open popover");
            (handle, monitor, shown)
        });
        let is_shown = |item, cx: &mut TestAppContext| cx.read(|cx| shown.read(cx).contains(item));
        assert_eq!(
            cx.read(|cx| monitor.read(cx).process_views()),
            1,
            "the popover lists the busiest processes"
        );
        assert!(!is_shown(MenuBarItem::Gpu, cx));

        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click("gpu", cx);
            window.render_frame(cx);
            window.click("cpu", cx);
        })
        .unwrap();
        assert!(is_shown(MenuBarItem::Gpu, cx), "checking GPU shows it");
        assert!(!is_shown(MenuBarItem::Cpu, cx), "unchecking CPU hides it");
    }
}
