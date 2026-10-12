//! Small settings files in `~/Library/Application Support/Top`.

use std::path::{Path, PathBuf};

pub fn path(name: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join("Library/Application Support/Top")
            .join(name),
    )
}

/// The saved text, or an empty string when there is none.
pub fn read(path: Option<&Path>) -> String {
    path.and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default()
}

/// A setting that fails to save still applies for this session.
pub fn write(path: &Path, text: &str) {
    let _ = path
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| std::fs::write(path, text));
}
