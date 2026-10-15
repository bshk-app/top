# Developing Top

[README](../README.md) · [User guide](usage.md) · [Release guide](releases.md)

Top is a Rust workspace with a GPUI Kit application and a native macOS metrics
crate. Code, comments, documentation, and release notes are written in English.
The user guide describes release 0.1.0; uncommitted or newer source changes may
introduce controls that are not yet in that release.

## Prerequisites

- An Apple Silicon Mac running macOS 14 or later.
- Xcode with its Metal compiler and the selected developer tools available.
- Rust through rustup. `rust-toolchain.toml` pins Rust **1.95.0**, rustfmt,
  and Clippy.
- Git. Packaging additionally uses `jq` and the macOS signing/archive tools.

Signing credentials and `zamokctl` are needed for signed distribution, not for
ordinary source development. The supported release target is macOS arm64.

## Run from source

```sh
git clone https://github.com/bshk-app/top.git
cd top
cargo run --package top --locked
```

The metrics sampler reads actual OS counters. Sensor availability varies by
chip and OS; keep unavailable readings explicit. Do not replace them with demo
values in product documentation or media.

The numeric face uses JetBrains Mono when installed and otherwise keeps the
theme's platform font. The bundled themes live in `crates/app/themes/top.json`.
The existing icon is in `crates/app/assets/`.

## Project map

| Path | Responsibility |
| --- | --- |
| `crates/app/src/main.rs` | Application startup, actions, shortcuts, and menus |
| `crates/app/src/dashboard.rs` | Dashboard and its title-bar controls |
| `crates/app/src/processes.rs` | Process filtering, sorting, and selection |
| `crates/app/src/popover.rs`, `menu_bar.rs`, `status_item.rs` | Native menu bar indicators and compact view |
| `crates/app/src/widget.rs`, `windows.rs` | Floating widget and window lifecycle |
| `crates/app/src/monitor.rs` | Background sampling and shared recent history |
| `crates/app/src/layout.rs`, `settings.rs` | Saved layout and local preferences |
| `crates/app/src/updater.rs` | Packaged Sparkle integration and Stable/Beta selection |
| `crates/metrics/src/` | macOS CPU, process, memory, IOReport, SMC, disk, network, and system readers |
| `scripts/`, `.github/workflows/` | Packaging and release distribution |

Keep UI work on the main thread and sampling work off it. The surfaces share
one monitor. Process sampling is demanded by views that need a process list;
preserve that behavior when adding a view.

## Checks

Run from the repository root:

```sh
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked --release
```

Release tests exercise both application behavior and real local macOS access,
including the current process. A filesystem/process sandbox can restrict those
reads; distinguish a permission failure from a regression in the reader.
Unit and GPUI tests do not replace native QA of menu bar interactions, focus,
multiple windows, or the packaged updater.

For release tooling, run `actionlint` on the workflows, ShellCheck on the
release shell scripts, and Ruby syntax checks on the generated cask. Follow
the [release guide](releases.md) for artifact and distribution verification.

## Preview packaging

```sh
bash scripts/package-macos.sh --preview
```

This builds an app bundle and preview ZIP/DMG with an ad-hoc signature, without
submitting to Apple or publishing a release. The script downloads the pinned,
checksum-verified Sparkle framework and includes its license. Preview manifests
record that they are not notarized; the publisher refuses them.

Bundles are written under `target/bundle/<version>/preview/`; archives are
under `target/dist/<version>/preview/`. These generated directories are ignored.
The full signing/notarization setup is in the [release guide](releases.md).

## Development capture and profiling

The `screenshot` feature is a development aid that captures application views
after a few real samples, then exits:

```sh
TOP_SCREENSHOT=shot.png cargo run --package top --locked --features screenshot
```

The `perf` feature drives scripted scenarios and prints GPUI frame traces:

```sh
TOP_PERF=1 cargo run --package top --locked --release --features perf
```

Use actual signed release builds and native Cap recordings for the public
tutorial. Developer captures must be identified as such if they are ever used.
Keep raw desktop recordings, local settings, control logs, signing keys, and
render caches out of public source. [Media provenance](media/README.md)
records the delivered capture, source ranges, and visual verification.

## Version and release changes

The workspace version is inherited by both crates. Keep `VERSION`, the root
`Cargo.toml`, the two workspace package entries in `Cargo.lock`, and the changelog
consistent. Release-please updates them together through its release PR.

Tagged releases use Developer ID signing, Apple notarization, GitHub Releases,
signed Sparkle feeds, and the stable Homebrew cask. Repairs use shipped assets;
they do not rebuild published releases. See [macOS releases](releases.md) for
the exact workflows and recovery steps.
