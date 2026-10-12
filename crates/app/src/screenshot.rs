//! Development aid behind the `screenshot` feature: renders windows to PNG
//! through GPUI's offscreen renderer, which needs no screen-recording access.
//!
//! `TOP_SCREENSHOT=shot.png` writes `shot.png` (and `shot-<n>.png` for later
//! windows) once a few samples have arrived, then quits. `TOP_SCREENSHOT_THEME`
//! set to `light` captures the light appearance.

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::component::ThemeMode;
use gpui_kit::{AnyWindowHandle, App, Entity};

use crate::popover::MenuBar;

const SETTLE: Duration = Duration::from_secs(6);

pub fn schedule(windows: Vec<AnyWindowHandle>, menu_bar: Entity<MenuBar>, cx: &mut App) {
    let Some(path) = std::env::var_os("TOP_SCREENSHOT").map(PathBuf::from) else {
        return;
    };
    if std::env::var("TOP_SCREENSHOT_THEME").as_deref() == Ok("light") {
        crate::theme::set_mode(ThemeMode::Light, cx);
    }
    cx.spawn(async move |cx| {
        cx.background_executor().timer(SETTLE).await;
        for (ix, window) in windows.into_iter().enumerate() {
            let target = if ix == 0 {
                path.clone()
            } else {
                numbered(&path, ix)
            };
            let saved = window.update(cx, |_, window, _| {
                window.refresh();
                window.render_to_image().map(|image| image.save(&target))
            });
            eprintln!("screenshot {}: {saved:?}", target.display());
        }
        cx.update(|cx| {
            menu_bar.read(cx).save_items(&path, cx);
            cx.quit()
        });
    })
    .detach();
}

fn numbered(path: &std::path::Path, ix: usize) -> PathBuf {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("shot");
    path.with_file_name(format!("{stem}-{ix}.png"))
}
