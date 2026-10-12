use gpui_kit::component::{Theme, ThemeMode, ThemeRegistry};
use gpui_kit::{App, SharedString};

const THEMES: &str = include_str!("../themes/top.json");

/// The design's numeric face, in order of preference; the theme's platform
/// default stays when neither is installed, because GPUI panics on a family
/// it cannot load.
const MONO_FAMILIES: [&str; 2] = ["JetBrains Mono", "JetBrains Mono NL"];

/// Registers the Top light and dark themes and starts in dark appearance.
pub fn init(cx: &mut App) {
    let registry = ThemeRegistry::global_mut(cx);
    registry
        .load_themes_from_str(THEMES)
        .expect("bundled Top theme is valid JSON");
    let theme = |name: &str| {
        registry
            .themes()
            .get(&SharedString::from(name))
            .cloned()
            .unwrap_or_else(|| panic!("bundled theme {name:?} is registered"))
    };
    let (light, dark) = (theme("Top Light"), theme("Top Dark"));

    let installed = cx.text_system().all_font_names();
    let mono = MONO_FAMILIES
        .into_iter()
        .find(|family| installed.iter().any(|name| name == family));

    Theme::update(cx, |theme| {
        theme.light_theme = light;
        theme.dark_theme = dark;
        if let Some(mono) = mono {
            theme.mono_font_family = mono.into();
        }
    });
    Theme::change(ThemeMode::Dark, None, cx);
}

pub fn set_mode(mode: ThemeMode, cx: &mut App) {
    Theme::change(mode, None, cx);
}
