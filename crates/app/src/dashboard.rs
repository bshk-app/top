use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _, ThemeMode, TitleBar, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, rems,
};

use crate::layout::{GridColumns, TileLayout};
use crate::monitor::{Interval, Monitor};
use crate::processes::ProcessPanel;
use crate::tiles::{self, TileGrid};
use crate::vitals::{self, Vitals};
use crate::widgets::{hidden, live_dot, mono};
use crate::{FocusProcesses, FocusSearch, QuitProcess, ToggleFreeze, theme};

/// The title bar's height in rem: 52 px at the default size, centring the
/// traffic lights placed by the window options.
const HEADER_HEIGHT: f32 = 3.25;
/// The process table keeps at least this much height; below it the content scrolls.
const MIN_TABLE_HEIGHT: f32 = 16.;

/// The main window. Its own render is the cheap chrome — title bar, status
/// bar, layout — while the tiles, the vitals row and the process card are
/// cached views that repaint only when their own data changes.
pub struct Dashboard {
    monitor: Entity<Monitor>,
    layout: Entity<TileLayout>,
    header: Entity<Header>,
    tiles: Entity<TileGrid>,
    vitals: Entity<Vitals>,
    processes: Entity<ProcessPanel>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Dashboard {
    pub fn new(
        monitor: Entity<Monitor>,
        layout: Entity<TileLayout>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let header = cx.new(|cx| Header::new(monitor.clone(), layout.clone(), cx));
        let tiles = cx.new(|cx| TileGrid::new(monitor.clone(), layout.clone(), cx));
        let vitals = cx.new(|cx| Vitals::new(monitor.clone(), cx));
        let processes = cx.new(|cx| ProcessPanel::new(monitor.clone(), window, cx));
        let subscriptions = vec![
            cx.observe(&monitor, |_, _, cx| cx.notify()),
            cx.observe(&layout, |_, _, cx| cx.notify()),
        ];
        Self {
            monitor,
            layout,
            header,
            tiles,
            vitals,
            processes,
            focus_handle: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    fn toggle_freeze(&mut self, _: &ToggleFreeze, _: &mut Window, cx: &mut Context<Self>) {
        self.monitor
            .update(cx, |monitor, cx| monitor.toggle_freeze(cx));
    }

    fn focus_search(&mut self, _: &FocusSearch, window: &mut Window, cx: &mut Context<Self>) {
        self.processes
            .update(cx, |panel, cx| panel.focus_search(window, cx));
    }

    fn focus_processes(&mut self, _: &FocusProcesses, window: &mut Window, cx: &mut Context<Self>) {
        self.processes
            .update(cx, |panel, cx| panel.focus_table(window, cx));
    }

    fn quit_process(&mut self, _: &QuitProcess, window: &mut Window, cx: &mut Context<Self>) {
        let Some(process) = self.processes.read(cx).selected(cx) else {
            return;
        };
        let monitor = self.monitor.clone();
        let command: SharedString = process.command.into();
        let (pid, started_at, user) = (process.pid, process.started_at, process.user);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let monitor = monitor.clone();
            let command = command.clone();
            alert
                .title(format!("Quit “{command}”?"))
                .description(format!(
                    "PID {pid} · {user}. Top sends SIGTERM first, then SIGKILL after 3 s. \
                     Unsaved work in this process is lost."
                ))
                .confirm()
                .ok_text("Quit process")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, _, cx| {
                    monitor.update(cx, |monitor, cx| {
                        monitor.quit_process(pid, started_at, command.clone(), cx)
                    });
                    true
                })
        });
    }

    fn render_status_bar(&self, cx: &App) -> impl IntoElement {
        let monitor = self.monitor.read(cx);
        let status: SharedString = match monitor.notice() {
            Some(notice) => notice.clone(),
            None if monitor.is_frozen() => "Frozen · press F to resume".into(),
            None if !monitor.is_live() => "Reading sensors…".into(),
            None => format!(
                "Updating every {} ms · thermals {}",
                monitor.interval().millis(),
                monitor.snapshot().thermal.label().to_lowercase()
            )
            .into(),
        };
        StatusBar::new()
            .h_8()
            .px_4()
            .font_family(cx.theme().mono_font_family.clone())
            .left(status)
            .right("F freeze · / search · ↑↓ select · ⌫ quit · drag tiles to arrange")
    }
}

impl Focusable for Dashboard {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Dashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let columns = tiles::columns(self.layout.read(cx), window);
        let header_style = StyleRefinement::default()
            .w_full()
            .flex_shrink_0()
            .h(rems(HEADER_HEIGHT));
        let tiles_style = StyleRefinement::default()
            .w_full()
            .flex_shrink_0()
            .h(tiles::grid_height(columns, window));
        let vitals_style = StyleRefinement::default()
            .w_full()
            .flex_shrink_0()
            .h(rems(vitals::HEIGHT));
        let processes_style = StyleRefinement::default()
            .w_full()
            .flex_1()
            .min_h(rems(MIN_TABLE_HEIGHT));

        v_flex()
            .key_context("Dashboard")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::toggle_freeze))
            .on_action(cx.listener(Self::focus_search))
            .on_action(cx.listener(Self::focus_processes))
            .on_action(cx.listener(Self::quit_process))
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .when(!hidden("header"), |this| {
                this.child(self.header.clone().cached(header_style))
            })
            .child(
                v_flex()
                    .id("content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .gap_3()
                    .font_family(cx.theme().mono_font_family.clone())
                    .when(!hidden("tiles"), |this| {
                        this.child(self.tiles.clone().cached(tiles_style))
                    })
                    .when(!hidden("vitals"), |this| {
                        this.child(self.vitals.clone().cached(vitals_style))
                    })
                    .when(!hidden("processes"), |this| {
                        this.child(self.processes.clone().cached(processes_style))
                    }),
            )
            .child(self.render_status_bar(cx))
    }
}

/// The title bar. A cached view of its own, so moving through the process
/// table — which repaints every ancestor of the table — does not rebuild it.
struct Header {
    monitor: Entity<Monitor>,
    layout: Entity<TileLayout>,
    _subscriptions: Vec<Subscription>,
}

impl Header {
    fn new(monitor: Entity<Monitor>, layout: Entity<TileLayout>, cx: &mut Context<Self>) -> Self {
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

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let monitor = self.monitor.read(cx);
        let theme = cx.theme();
        let chip = monitor.chip().map(|chip| {
            format!(
                "{} · {}C ({}E+{}P) · {} GPU · {:.0} GB",
                chip.name,
                chip.efficiency_cores + chip.performance_cores,
                chip.efficiency_cores,
                chip.performance_cores,
                chip.gpu_cores,
                chip.memory_bytes as f64 / (1u64 << 30) as f64,
            )
        });
        let interval_ix = Interval::ALL
            .iter()
            .position(|i| *i == monitor.interval())
            .unwrap_or(1);
        let frozen = monitor.is_frozen();

        TitleBar::new()
            .h(rems(HEADER_HEIGHT))
            .pl_24()
            .pr_3p5()
            .child(
                h_flex()
                    .items_baseline()
                    .gap_2p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Top"),
                    )
                    .when_some(chip, |this, chip| {
                        this.child(
                            mono(cx)
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(chip),
                        )
                    }),
            )
            .child(
                h_flex()
                    .gap_4()
                    .child(live_indicator(monitor.is_live() && !frozen, frozen, cx))
                    .child(
                        TabBar::new("interval")
                            .segmented()
                            .xsmall()
                            .selected_index(interval_ix)
                            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                                let interval = Interval::ALL[*ix];
                                this.monitor
                                    .update(cx, |monitor, cx| monitor.set_interval(interval, cx));
                            }))
                            .children(
                                Interval::ALL.map(|interval| Tab::new().label(interval.label())),
                            ),
                    )
                    .child(
                        Button::new("freeze")
                            .small()
                            .outline()
                            .label(if frozen { "Resume" } else { "Freeze" })
                            .tooltip("Pause updates (F)")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.monitor
                                    .update(cx, |monitor, cx| monitor.toggle_freeze(cx));
                            })),
                    )
                    .child(self.render_layout_menu(cx))
                    .child(
                        TabBar::new("appearance")
                            .segmented()
                            .xsmall()
                            .selected_index(if cx.theme().is_dark() { 1 } else { 0 })
                            .on_click(cx.listener(|_, ix: &usize, _, cx| {
                                let mode = if *ix == 0 {
                                    ThemeMode::Light
                                } else {
                                    ThemeMode::Dark
                                };
                                theme::set_mode(mode, cx);
                            }))
                            .child(Tab::new().label("Light"))
                            .child(Tab::new().label("Dark")),
                    ),
            )
    }

    /// Tile columns and layout reset; tiles themselves are rearranged by dragging.
    fn render_layout_menu(&self, cx: &App) -> impl IntoElement {
        let layout = self.layout.clone();
        let current = self.layout.read(cx).columns();
        Button::new("layout")
            .small()
            .ghost()
            .icon(IconName::LayoutDashboard)
            .tooltip("Tile layout")
            .dropdown_menu(move |menu, _, cx| {
                let is_default = layout.read(cx).is_default();
                let menu = GridColumns::CHOICES.into_iter().fold(
                    menu.label("Columns"),
                    |menu, columns| {
                        let layout = layout.clone();
                        menu.item(
                            PopupMenuItem::new(columns.label())
                                .checked(columns == current)
                                .on_click(move |_, _, cx| {
                                    layout.update(cx, |layout, cx| layout.set_columns(columns, cx))
                                }),
                        )
                    },
                );
                let layout = layout.clone();
                menu.separator().item(
                    PopupMenuItem::new("Reset layout")
                        .disabled(is_default)
                        .on_click(move |_, _, cx| layout.update(cx, |layout, cx| layout.reset(cx))),
                )
            })
    }
}

impl Render for Header {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_title_bar(cx)
    }
}

fn live_indicator(live: bool, frozen: bool, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    h_flex()
        .gap_1p5()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(live_dot(live, cx).size_2())
        .child(if frozen {
            "Frozen"
        } else if live {
            "Live"
        } else {
            "Starting"
        })
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::WindowExt as _;
    use gpui_kit::component::table::{ColumnSort, TableDelegate as _};
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{
        AnyWindowHandle, AppContext as _, Bounds, Entity, TestAppContext, WindowBounds,
        WindowOptions, px, size,
    };
    use top_metrics::{ChipInfo, Process, Snapshot};

    use super::Dashboard;
    use crate::layout::{TileKind, TileLayout};
    use crate::monitor::Monitor;

    /// Pids no real process uses, so a slip in a test cannot signal one.
    fn process(pid: i32, command: &str, cpu: f32) -> Process {
        let mut process = Process::default();
        process.pid = pid;
        process.command = command.into();
        process.user = "akira".into();
        process.cpu = cpu;
        process.resident = 1 << 20;
        process
    }

    struct Fixture {
        handle: AnyWindowHandle,
        monitor: Entity<Monitor>,
        layout: Entity<TileLayout>,
        dashboard: Entity<Dashboard>,
    }

    fn open_dashboard(cx: &mut TestAppContext) -> Fixture {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::theme::init(cx);
            crate::bind_keys(cx);
            let mut snapshot = Snapshot::default();
            snapshot.processes = vec![
                process(999_991, "WindowServer", 30.0),
                process(999_992, "ghostty", 12.0),
            ];
            let monitor = cx.new(|cx| Monitor::fixture(ChipInfo::default(), snapshot, cx));
            let layout = cx.new(|_| TileLayout::in_memory());
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(1240.), px(900.)),
                ))),
                ..Default::default()
            };
            let (monitor_for_view, layout_for_view) = (monitor.clone(), layout.clone());
            let (handle, dashboard) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| Dashboard::new(monitor_for_view, layout_for_view, window, cx))
            })
            .expect("open dashboard");
            Fixture {
                handle,
                monitor,
                layout,
                dashboard,
            }
        })
    }

    fn is_frozen(monitor: &Entity<Monitor>, cx: &mut TestAppContext) -> bool {
        cx.update(|cx| monitor.read(cx).is_frozen())
    }

    #[gpui_kit::test]
    fn lists_processes_only_while_open(cx: &mut TestAppContext) {
        let Fixture {
            handle, monitor, ..
        } = open_dashboard(cx);
        let views = |cx: &mut TestAppContext| cx.read(|cx| monitor.read(cx).process_views());
        assert_eq!(views(cx), 1);
        cx.update_window(handle, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        assert_eq!(
            views(cx),
            0,
            "closing the dashboard stops listing processes"
        );
    }

    #[gpui_kit::test]
    fn single_key_shortcuts_ignore_typing_in_search(cx: &mut TestAppContext) {
        let Fixture {
            handle, monitor, ..
        } = open_dashboard(cx);
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.press("f", cx);
        })
        .unwrap();
        assert!(is_frozen(&monitor, cx), "F freezes from the process table");

        cx.update_window(handle, |_, window, cx| {
            window.press("/", cx);
            assert_eq!(window.find("search").focused(), Some(true));
            window.input("f", cx);
            assert_eq!(window.find("search").value(), Some("f"));
        })
        .unwrap();
        assert!(
            is_frozen(&monitor, cx),
            "typing F in search leaves freeze alone"
        );

        cx.update_window(handle, |_, window, cx| {
            window.press("escape", cx);
            assert_eq!(window.find("search").focused(), Some(false));
            window.press("f", cx);
        })
        .unwrap();
        assert!(
            !is_frozen(&monitor, cx),
            "Escape returns the keys to the table"
        );
    }

    #[gpui_kit::test]
    fn backspace_confirms_before_quitting_the_selected_process(cx: &mut TestAppContext) {
        let Fixture {
            handle, dashboard, ..
        } = open_dashboard(cx);
        // Each key is its own event-loop turn, as when a person types them.
        let press = |key: &'static str, cx: &mut TestAppContext| {
            cx.update_window(handle, |_, window, cx| window.press(key, cx))
                .unwrap();
        };
        let has_dialog = |cx: &mut TestAppContext| {
            cx.update_window(handle, |_, window, cx| window.has_active_dialog(cx))
                .unwrap()
        };

        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        press("backspace", cx);
        assert!(!has_dialog(cx), "no process is selected yet");

        press("down", cx);
        let selected = cx.update(|cx| {
            let panel = dashboard.read(cx).processes.read(cx);
            panel.selected(cx).map(|p| p.pid)
        });
        assert_eq!(
            selected,
            Some(999_991),
            "the busiest process is the first row"
        );

        press("backspace", cx);
        assert!(has_dialog(cx));
    }

    #[gpui_kit::test]
    fn sorting_keeps_the_highlight_on_the_selected_process(cx: &mut TestAppContext) {
        let Fixture {
            handle, dashboard, ..
        } = open_dashboard(cx);
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.update_window(handle, |_, window, cx| window.press("down", cx))
            .unwrap();

        // Command A→Z puts ghostty first, moving WindowServer to row 1.
        let table = cx.update(|cx| dashboard.read(cx).processes.read(cx).table().clone());
        cx.update_window(handle, |_, window, cx| {
            table.update(cx, |table, cx| {
                let command = 7;
                table
                    .delegate_mut()
                    .perform_sort(command, ColumnSort::Ascending, window, cx);
            });
        })
        .unwrap();

        let (highlighted, selected) = cx.update(|cx| {
            let table = table.read(cx);
            (
                table.selected_row(),
                table.delegate().selected().map(|p| p.pid),
            )
        });
        assert_eq!(selected, Some(999_991));
        assert_eq!(highlighted, Some(1), "the highlight follows WindowServer");
    }

    #[gpui_kit::test]
    fn dropping_a_tile_on_another_takes_its_place(cx: &mut TestAppContext) {
        let Fixture { handle, layout, .. } = open_dashboard(cx);
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.drag_to(
                ("tile", TileKind::Cpu as usize),
                ("tile", TileKind::Ane as usize),
                cx,
            );
        })
        .unwrap();
        let order = cx.update(|cx| layout.read(cx).order().to_vec());
        assert_eq!(
            order,
            vec![
                TileKind::Gpu,
                TileKind::Ane,
                TileKind::Cpu,
                TileKind::Memory
            ]
        );
    }
}
