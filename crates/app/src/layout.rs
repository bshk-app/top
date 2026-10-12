//! The user's tile arrangement: order and column count, kept between launches.

use std::path::PathBuf;

use gpui_kit::{Context, Pixels};

use crate::settings;

/// One headline metric tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileKind {
    Cpu,
    Gpu,
    Ane,
    Memory,
}

impl TileKind {
    pub const ALL: [Self; 4] = [Self::Cpu, Self::Gpu, Self::Ane, Self::Memory];

    /// Stable identifier used for element ids and the saved layout.
    pub fn key(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Ane => "ane",
            Self::Memory => "memory",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key)
    }
}

/// How many columns the tile grid uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridColumns {
    /// As many as fit at the minimum tile width.
    Auto,
    Fixed(u8),
}

impl GridColumns {
    pub const CHOICES: [Self; 5] = [
        Self::Auto,
        Self::Fixed(1),
        Self::Fixed(2),
        Self::Fixed(3),
        Self::Fixed(4),
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Fixed(1) => "1 column",
            Self::Fixed(2) => "2 columns",
            Self::Fixed(3) => "3 columns",
            Self::Fixed(_) => "4 columns",
        }
    }

    fn key(self) -> String {
        match self {
            Self::Auto => "auto".to_owned(),
            Self::Fixed(count) => count.to_string(),
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "auto" => Some(Self::Auto),
            count => count
                .parse()
                .ok()
                .filter(|count| (1..=TileKind::ALL.len() as u8).contains(count))
                .map(Self::Fixed),
        }
    }
}

/// The tile order and grid columns. Changes are saved immediately.
pub struct TileLayout {
    order: Vec<TileKind>,
    columns: GridColumns,
    path: Option<PathBuf>,
}

impl TileLayout {
    /// Reads the saved layout, falling back to the default for anything missing or invalid.
    pub fn load() -> Self {
        let path = settings::path("layout");
        let (order, columns) = parse(&settings::read(path.as_deref()));
        Self {
            order,
            columns,
            path,
        }
    }

    /// The default layout, never saved; for tests.
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self {
            order: TileKind::ALL.to_vec(),
            columns: GridColumns::Auto,
            path: None,
        }
    }

    pub fn order(&self) -> &[TileKind] {
        &self.order
    }

    pub fn columns(&self) -> GridColumns {
        self.columns
    }

    pub fn is_default(&self) -> bool {
        self.order == TileKind::ALL && self.columns == GridColumns::Auto
    }

    pub fn set_columns(&mut self, columns: GridColumns, cx: &mut Context<Self>) {
        if self.columns != columns {
            self.columns = columns;
            self.changed(cx);
        }
    }

    /// Puts `tile` where `target` is; the tiles between them shift toward `tile`'s old place.
    pub fn move_tile(&mut self, tile: TileKind, target: TileKind, cx: &mut Context<Self>) {
        if reorder(&mut self.order, tile, target) {
            self.changed(cx);
        }
    }

    /// Moves `tile` one place toward the start (`-1`) or the end (`1`).
    pub fn shift_tile(&mut self, tile: TileKind, by: isize, cx: &mut Context<Self>) {
        let Some(ix) = self.order.iter().position(|t| *t == tile) else {
            return;
        };
        let target = ix
            .checked_add_signed(by)
            .and_then(|ix| self.order.get(ix))
            .copied();
        if let Some(target) = target {
            self.move_tile(tile, target, cx);
        }
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        if !self.is_default() {
            self.order = TileKind::ALL.to_vec();
            self.columns = GridColumns::Auto;
            self.changed(cx);
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = &self.path {
            settings::write(path, &serialize(&self.order, self.columns));
        }
        cx.notify();
    }
}

/// Columns actually used: the fixed count, or as many as fit `width` for Auto.
pub fn resolve_columns(
    columns: GridColumns,
    width: Pixels,
    min_tile: Pixels,
    gap: Pixels,
) -> usize {
    let max = TileKind::ALL.len();
    match columns {
        GridColumns::Fixed(count) => (count as usize).clamp(1, max),
        GridColumns::Auto => {
            let fit = ((width + gap) / (min_tile + gap)).floor();
            (fit.max(1.0) as usize).min(max)
        }
    }
}

fn reorder(order: &mut Vec<TileKind>, tile: TileKind, target: TileKind) -> bool {
    let (Some(from), Some(to)) = (
        order.iter().position(|t| *t == tile),
        order.iter().position(|t| *t == target),
    ) else {
        return false;
    };
    if from == to {
        return false;
    }
    let tile = order.remove(from);
    order.insert(to, tile);
    true
}

fn serialize(order: &[TileKind], columns: GridColumns) -> String {
    let order: Vec<&str> = order.iter().map(|tile| tile.key()).collect();
    format!("order={}\ncolumns={}\n", order.join(","), columns.key())
}

/// Parses `order=cpu,gpu,…` and `columns=auto|1…4`. Unknown or repeated
/// tiles are dropped and missing ones appended, so every tile shows exactly once.
fn parse(text: &str) -> (Vec<TileKind>, GridColumns) {
    let mut order = Vec::new();
    let mut columns = GridColumns::Auto;
    for line in text.lines() {
        match line.split_once('=') {
            Some(("order", value)) => {
                for tile in value
                    .split(',')
                    .filter_map(|key| TileKind::from_key(key.trim()))
                {
                    if !order.contains(&tile) {
                        order.push(tile);
                    }
                }
            }
            Some(("columns", value)) => {
                columns = GridColumns::from_key(value.trim()).unwrap_or(GridColumns::Auto)
            }
            _ => {}
        }
    }
    for tile in TileKind::ALL {
        if !order.contains(&tile) {
            order.push(tile);
        }
    }
    (order, columns)
}

#[cfg(test)]
mod tests {
    use gpui_kit::px;

    use super::*;

    #[test]
    fn round_trips_a_saved_layout() {
        let order = vec![
            TileKind::Memory,
            TileKind::Cpu,
            TileKind::Ane,
            TileKind::Gpu,
        ];
        let text = serialize(&order, GridColumns::Fixed(2));
        assert_eq!(parse(&text), (order, GridColumns::Fixed(2)));
    }

    #[test]
    fn repairs_a_damaged_layout() {
        let (order, columns) = parse("order=gpu,gpu,bogus\ncolumns=9\n");
        assert_eq!(
            order,
            vec![
                TileKind::Gpu,
                TileKind::Cpu,
                TileKind::Ane,
                TileKind::Memory
            ]
        );
        assert_eq!(columns, GridColumns::Auto);
    }

    #[test]
    fn moving_a_tile_takes_the_target_place() {
        let mut order = TileKind::ALL.to_vec();
        assert!(reorder(&mut order, TileKind::Cpu, TileKind::Ane));
        assert_eq!(
            order,
            vec![
                TileKind::Gpu,
                TileKind::Ane,
                TileKind::Cpu,
                TileKind::Memory
            ]
        );
        assert!(reorder(&mut order, TileKind::Memory, TileKind::Gpu));
        assert_eq!(
            order,
            vec![
                TileKind::Memory,
                TileKind::Gpu,
                TileKind::Ane,
                TileKind::Cpu
            ]
        );
        assert!(!reorder(&mut order, TileKind::Gpu, TileKind::Gpu));
    }

    #[test]
    fn auto_columns_follow_the_width() {
        let (min, gap) = (px(240.), px(12.));
        assert_eq!(resolve_columns(GridColumns::Auto, px(1208.), min, gap), 4);
        assert_eq!(resolve_columns(GridColumns::Auto, px(900.), min, gap), 3);
        assert_eq!(resolve_columns(GridColumns::Auto, px(500.), min, gap), 2);
        assert_eq!(resolve_columns(GridColumns::Auto, px(100.), min, gap), 1);
        assert_eq!(
            resolve_columns(GridColumns::Fixed(2), px(1208.), min, gap),
            2
        );
    }
}
