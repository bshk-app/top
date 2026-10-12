//! The metrics Top can show in the menu bar, each as its own status item,
//! and which of them the user keeps there.

use std::path::PathBuf;

use top_metrics::Snapshot;

use crate::format;
use crate::settings;
use crate::status_item::{Align, Column, Font, Line, Tint};

/// Traffic above this lights the network item's dot.
const ACTIVE_RATE: f64 = 1024.;
const NO_VALUE: &str = "–";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuBarItem {
    Sensors,
    Cpu,
    Gpu,
    Memory,
    Disk,
    Network,
}

impl MenuBarItem {
    /// Left to right, as they appear in the menu bar.
    pub const ALL: [Self; 6] = [
        Self::Sensors,
        Self::Cpu,
        Self::Gpu,
        Self::Memory,
        Self::Disk,
        Self::Network,
    ];
    const DEFAULT: [Self; 5] = [
        Self::Sensors,
        Self::Cpu,
        Self::Memory,
        Self::Disk,
        Self::Network,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Sensors => "sensors",
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Memory => "memory",
            Self::Disk => "disk",
            Self::Network => "network",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sensors => "Sensors",
            Self::Cpu => "CPU",
            Self::Gpu => "GPU",
            Self::Memory => "RAM",
            Self::Disk => "SSD",
            Self::Network => "Network",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|item| item.key() == key)
    }

    /// What the item shows; placeholders until the first sample.
    pub fn columns(self, snapshot: Option<&Snapshot>) -> Vec<Column> {
        let percent = |value: fn(&Snapshot) -> f32| {
            snapshot.map_or(format!("{NO_VALUE}%"), |s| format!("{:.0}%", value(s)))
        };
        match self {
            Self::Sensors => {
                let watts = |value: fn(&Snapshot) -> Option<f32>| {
                    snapshot
                        .and_then(value)
                        .map_or(format!("{NO_VALUE}W"), |w| format!("{w:.1}W"))
                };
                let degrees = |value: fn(&Snapshot) -> Option<f32>| {
                    snapshot
                        .and_then(value)
                        .map_or(format!("{NO_VALUE}°"), |t| format!("{t:.0}°"))
                };
                vec![
                    stacked(
                        [watts(|s| s.power.cpu), degrees(|s| s.temperatures.cpu)],
                        "99.9W",
                    ),
                    stacked(
                        [watts(|s| s.power.gpu), degrees(|s| s.temperatures.gpu)],
                        "99.9W",
                    ),
                ]
            }
            Self::Cpu => captioned("CPU", percent(|s| s.cpu.usage)),
            Self::Gpu => captioned("GPU", percent(|s| s.gpu.usage)),
            Self::Memory => captioned("RAM", percent(memory_used)),
            Self::Disk => captioned("SSD", percent(startup_disk_used)),
            Self::Network => {
                let (sent, received) =
                    snapshot.map_or((0., 0.), |s| (s.network.write, s.network.read));
                let dot = |rate: f64, active: Tint| Line {
                    text: "●".to_owned(),
                    font: Font::Stacked,
                    tint: if rate > ACTIVE_RATE {
                        active
                    } else {
                        Tint::Muted
                    },
                };
                let rate = |rate: f64| snapshot.map_or(NO_VALUE.to_owned(), |_| format::rate(rate));
                let mut rates = stacked([rate(sent), rate(received)], "999.9 MB/s");
                rates.align = Align::Trailing;
                vec![
                    Column {
                        lines: vec![dot(sent, Tint::Upload), dot(received, Tint::Download)],
                        widest: "●",
                        align: Align::Center,
                    },
                    rates,
                ]
            }
        }
    }

    /// What VoiceOver reads: the item's name and values.
    pub fn accessibility_label(self, columns: &[Column]) -> String {
        let values: Vec<&str> = columns
            .iter()
            .flat_map(|column| &column.lines)
            .filter(|line| line.font != Font::Caption && line.text != "●")
            .map(|line| line.text.as_str())
            .collect();
        format!("{}: {}", self.label(), values.join(" "))
    }
}

fn captioned(caption: &str, value: String) -> Vec<Column> {
    vec![Column {
        lines: vec![
            Line {
                text: caption.to_owned(),
                font: Font::Caption,
                tint: Tint::Primary,
            },
            Line {
                text: value,
                font: Font::Value,
                tint: Tint::Primary,
            },
        ],
        widest: "100%",
        align: Align::Center,
    }]
}

fn stacked(lines: [String; 2], widest: &'static str) -> Column {
    Column {
        lines: lines
            .map(|text| Line {
                text,
                font: Font::Stacked,
                tint: Tint::Primary,
            })
            .into(),
        widest,
        align: Align::Center,
    }
}

fn memory_used(snapshot: &Snapshot) -> f32 {
    let memory = &snapshot.memory;
    memory.used as f32 / memory.total.max(1) as f32 * 100.
}

/// Used share of the startup volume's APFS container, which every volume on it shares.
fn startup_disk_used(snapshot: &Snapshot) -> f32 {
    snapshot
        .volumes
        .iter()
        .find(|volume| volume.mount_point == "/")
        .map_or(0., |volume| {
            (volume.total - volume.available.min(volume.total)) as f32 / volume.total.max(1) as f32
                * 100.
        })
}

/// The items in the menu bar, kept between launches. At least one stays,
/// so the popover can always be opened.
pub struct ShownItems {
    items: Vec<MenuBarItem>,
    path: Option<PathBuf>,
}

impl ShownItems {
    pub fn load() -> Self {
        let path = settings::path("menu-bar");
        Self {
            items: parse(&settings::read(path.as_deref())),
            path,
        }
    }

    /// The default items, never saved; for tests.
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self {
            items: MenuBarItem::DEFAULT.to_vec(),
            path: None,
        }
    }

    pub fn contains(&self, item: MenuBarItem) -> bool {
        self.items.contains(&item)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Shows or hides `item`; returns whether anything changed. Hiding the
    /// last item is refused.
    pub fn set(&mut self, item: MenuBarItem, shown: bool) -> bool {
        if shown == self.contains(item) || (!shown && self.items.len() == 1) {
            return false;
        }
        self.items = MenuBarItem::ALL
            .into_iter()
            .filter(|&other| {
                if other == item {
                    shown
                } else {
                    self.contains(other)
                }
            })
            .collect();
        if let Some(path) = &self.path {
            settings::write(path, &serialize(&self.items));
        }
        true
    }
}

fn serialize(items: &[MenuBarItem]) -> String {
    let keys: Vec<&str> = items.iter().map(|item| item.key()).collect();
    format!("items={}\n", keys.join(","))
}

/// Parses `items=cpu,memory,…`; unknown keys are dropped, and nothing valid
/// means the defaults.
fn parse(text: &str) -> Vec<MenuBarItem> {
    let keys: Vec<MenuBarItem> = text
        .lines()
        .filter_map(|line| line.strip_prefix("items="))
        .flat_map(|value| value.split(','))
        .filter_map(|key| MenuBarItem::from_key(key.trim()))
        .collect();
    let items: Vec<MenuBarItem> = MenuBarItem::ALL
        .into_iter()
        .filter(|item| keys.contains(item))
        .collect();
    if items.is_empty() {
        MenuBarItem::DEFAULT.to_vec()
    } else {
        items
    }
}

#[cfg(test)]
mod tests {
    use top_metrics::{Snapshot, Volume};

    use super::*;

    #[test]
    fn round_trips_the_shown_items() {
        let items = vec![MenuBarItem::Gpu, MenuBarItem::Network];
        assert_eq!(parse(&serialize(&items)), items);
        assert_eq!(parse(""), MenuBarItem::DEFAULT);
        assert_eq!(parse("items=bogus,cpu,cpu\n"), vec![MenuBarItem::Cpu]);
    }

    #[test]
    fn never_hides_every_item() {
        let mut shown = ShownItems::in_memory();
        assert!(shown.set(MenuBarItem::Gpu, true));
        assert!(!shown.set(MenuBarItem::Gpu, true), "already shown");
        for item in MenuBarItem::ALL {
            shown.set(item, false);
        }
        assert_eq!(shown.len(), 1, "the last item stays");
        assert!(shown.contains(MenuBarItem::Network));
    }

    fn texts(columns: &[Column]) -> Vec<Vec<&str>> {
        columns
            .iter()
            .map(|column| column.lines.iter().map(|line| line.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn shows_values_like_the_menu_bar_design() {
        let mut snapshot = Snapshot::default();
        snapshot.cpu.usage = 39.4;
        snapshot.power.cpu = Some(0.71);
        snapshot.temperatures.cpu = Some(74.2);
        snapshot.memory.used = 86;
        snapshot.memory.total = 100;
        let mut startup = Volume::default();
        startup.mount_point = "/".to_owned();
        startup.total = 100;
        startup.available = 19;
        snapshot.volumes = vec![startup];
        snapshot.network.write = 114. * 1024.;
        snapshot.network.read = 100.;

        let snapshot = Some(&snapshot);
        assert_eq!(texts(&MenuBarItem::Cpu.columns(snapshot)), [["CPU", "39%"]]);
        assert_eq!(
            texts(&MenuBarItem::Memory.columns(snapshot)),
            [["RAM", "86%"]]
        );
        assert_eq!(
            texts(&MenuBarItem::Disk.columns(snapshot)),
            [["SSD", "81%"]]
        );
        assert_eq!(
            texts(&MenuBarItem::Sensors.columns(snapshot)),
            [["0.7W", "74°"], ["–W", "–°"]]
        );

        let network = MenuBarItem::Network.columns(snapshot);
        assert_eq!(texts(&network)[1], ["114.0 KB/s", "0.1 KB/s"]);
        let dots: Vec<Tint> = network[0].lines.iter().map(|line| line.tint).collect();
        assert_eq!(
            dots,
            [Tint::Upload, Tint::Muted],
            "only active traffic lights up"
        );
        assert_eq!(
            MenuBarItem::Network.accessibility_label(&network),
            "Network: 114.0 KB/s 0.1 KB/s"
        );
    }

    #[test]
    fn shows_placeholders_before_the_first_sample() {
        assert_eq!(texts(&MenuBarItem::Cpu.columns(None)), [["CPU", "–%"]]);
    }
}
