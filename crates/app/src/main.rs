mod dashboard;
mod format;
mod layout;
mod menu_bar;
mod monitor;
#[cfg(feature = "perf")]
mod perf;
mod popover;
mod processes;
#[cfg(feature = "screenshot")]
mod screenshot;
mod settings;
mod status_item;
mod theme;
mod tiles;
mod updater;
mod vitals;
mod widget;
mod widgets;
mod windows;

use gpui_kit::{App, AppContext as _, KeyBinding, Menu, MenuItem, actions};

use crate::monitor::Monitor;

actions!(
    top,
    [
        ToggleFreeze,
        FocusSearch,
        FocusProcesses,
        QuitProcess,
        ShowDashboard,
        ShowMenuBar,
        ToggleWidget,
        ClosePopover,
        CheckForUpdates,
        StableUpdates,
        BetaUpdates,
        Quit
    ]
);

/// Single-key shortcuts must not fire while typing in the search field.
const DASHBOARD_KEYS: &str = "Dashboard && !Input";

/// The app's keymap. Bound before the menus, which read shortcuts from it.
fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("f", ToggleFreeze, Some(DASHBOARD_KEYS)),
        KeyBinding::new("/", FocusSearch, Some(DASHBOARD_KEYS)),
        KeyBinding::new("backspace", QuitProcess, Some(DASHBOARD_KEYS)),
        KeyBinding::new("delete", QuitProcess, Some(DASHBOARD_KEYS)),
        KeyBinding::new("escape", FocusProcesses, Some("Dashboard")),
        KeyBinding::new("escape", ClosePopover, Some("MenuBarPopover")),
        KeyBinding::new("cmd-0", ShowDashboard, None),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            theme::init(cx);
            status_item::set_application_icon();
            let updates_available = updater::init();

            bind_keys(cx);
            cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
            cx.on_action(|_: &ShowDashboard, cx: &mut App| {
                windows::show_dashboard(cx);
            });
            cx.on_action(|_: &ShowMenuBar, cx: &mut App| {
                cx.defer(popover::show);
            });
            cx.on_action(|_: &ToggleWidget, cx: &mut App| {
                windows::toggle_widget(cx);
            });
            cx.on_action(|_: &CheckForUpdates, _| updater::check_for_updates());
            cx.on_action(move |_: &StableUpdates, cx| {
                updater::set_channel(updater::Channel::Stable);
                set_menus(updates_available, cx);
            });
            cx.on_action(move |_: &BetaUpdates, cx| {
                updater::set_channel(updater::Channel::Beta);
                set_menus(updates_available, cx);
            });
            set_menus(updates_available, cx);

            let monitor = cx.new(Monitor::new);
            windows::init(monitor.clone(), cx);
            let menu_bar = popover::init(monitor.clone(), cx);
            let dashboard = windows::show_dashboard(cx);
            #[cfg(feature = "perf")]
            perf::schedule(dashboard, monitor, cx);
            #[cfg(not(feature = "perf"))]
            let _ = monitor;
            let widget = windows::toggle_widget(cx);

            #[cfg(feature = "screenshot")]
            {
                let popover = menu_bar.update(cx, |menu_bar, cx| {
                    menu_bar.open_popover(menu_bar::MenuBarItem::Cpu, cx)
                });
                screenshot::schedule(
                    [Some(dashboard), widget, popover]
                        .into_iter()
                        .flatten()
                        .collect(),
                    menu_bar,
                    cx,
                );
            }
            #[cfg(not(feature = "screenshot"))]
            let _ = (dashboard, widget, menu_bar);
        });
}

fn set_menus(updates_available: bool, cx: &mut App) {
    let mut app_items = Vec::new();
    if updates_available {
        app_items.extend([
            MenuItem::action("Check for Updates…", CheckForUpdates),
            MenuItem::submenu(
                Menu::new("Update Channel").items([
                    MenuItem::action("Stable", StableUpdates)
                        .checked(updater::channel() == updater::Channel::Stable),
                    MenuItem::action("Beta", BetaUpdates)
                        .checked(updater::channel() == updater::Channel::Beta),
                ]),
            ),
            MenuItem::separator(),
        ]);
    }
    app_items.push(MenuItem::action("Quit Top", Quit));
    cx.set_menus([
        Menu::new("Top").items(app_items),
        Menu::new("Window").items([
            MenuItem::action("Dashboard", ShowDashboard),
            MenuItem::action("Menu Bar…", ShowMenuBar),
            MenuItem::action("Floating Widget", ToggleWidget),
        ]),
    ]);
}
