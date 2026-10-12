//! Which of Top's windows are open, so menus, the menu bar popover and
//! shortcuts can bring one forward instead of opening duplicates.

use gpui_kit::component::TitleBar;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Entity, Global, TitlebarOptions,
    WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, point, px, size,
};

use crate::dashboard::Dashboard;
use crate::layout::TileLayout;
use crate::monitor::Monitor;
use crate::widget::FloatingWidget;

/// The floating widget's size; its card fills the window.
const WIDGET_SIZE: (f32, f32) = (312., 246.);
/// Gap between the widget and the screen's top-right corner, below the menu bar.
const WIDGET_INSET: f32 = 24.;

pub struct Windows {
    monitor: Entity<Monitor>,
    /// The tile arrangement, shared by the dashboard and the widget.
    layout: Entity<TileLayout>,
    dashboard: Option<AnyWindowHandle>,
    widget: Option<AnyWindowHandle>,
}

impl Global for Windows {}

pub fn init(monitor: Entity<Monitor>, cx: &mut App) {
    let layout = cx.new(|_| TileLayout::load());
    cx.set_global(Windows {
        monitor,
        layout,
        dashboard: None,
        widget: None,
    });
}

pub fn monitor(cx: &App) -> Entity<Monitor> {
    cx.global::<Windows>().monitor.clone()
}

pub fn layout(cx: &App) -> Entity<TileLayout> {
    cx.global::<Windows>().layout.clone()
}

/// Brings the dashboard forward, reopening it if it was closed.
pub fn show_dashboard(cx: &mut App) -> AnyWindowHandle {
    if let Some(handle) = cx.global::<Windows>().dashboard
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        cx.activate(true);
        return handle;
    }
    let (monitor, layout) = (monitor(cx), layout(cx));
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(1240.), px(900.)), cx)),
        window_min_size: Some(size(px(960.), px(640.))),
        // Traffic lights centred in the 52 px header, inset like the design.
        titlebar: Some(TitlebarOptions {
            traffic_light_position: Some(point(px(18.), px(20.))),
            ..TitleBar::title_bar_options()
        }),
        ..TitleBar::window_options()
    };
    let handle = gpui_kit::open_window(options, cx, |window, cx| {
        window.set_window_title("Top");
        cx.new(|cx| Dashboard::new(monitor, layout, window, cx))
    })
    .expect("failed to open the Top window")
    .0;
    cx.global_mut::<Windows>().dashboard = Some(handle);
    cx.activate(true);
    handle
}

/// Shows the floating widget in the top-right corner, or closes it.
pub fn toggle_widget(cx: &mut App) -> Option<AnyWindowHandle> {
    if let Some(handle) = cx.global_mut::<Windows>().widget.take()
        && handle
            .update(cx, |_, window, _| window.remove_window())
            .is_ok()
    {
        return None;
    }
    let (monitor, layout) = (monitor(cx), layout(cx));
    let display = cx.primary_display()?.bounds();
    let (width, height) = WIDGET_SIZE;
    let origin = point(
        display.right() - px(width + WIDGET_INSET),
        display.top() + px(WIDGET_INSET * 2.),
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            origin,
            size(px(width), px(height)),
        ))),
        titlebar: None,
        kind: WindowKind::PopUp,
        focus: false,
        is_movable: true,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        ..WindowOptions::default()
    };
    let handle = gpui_kit::open_window(options, cx, |_, cx| {
        cx.new(|cx| FloatingWidget::new(monitor, layout, cx))
    })
    .ok()?
    .0;
    cx.global_mut::<Windows>().widget = Some(handle);
    Some(handle)
}

/// Closes the widget from inside its own window, where updating it through
/// its handle would re-enter the update in progress.
pub fn close_widget(window: &mut gpui_kit::Window, cx: &mut App) {
    window.remove_window();
    cx.global_mut::<Windows>().widget = None;
}
