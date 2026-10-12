# Top

A native Apple Silicon system monitor for macOS 14 or later, built with GPUI Kit.
Top provides a dashboard, menu bar metrics, a floating widget, and a process list.

All menu bar indicators, including Network, can be disabled in the popover.
The selection is saved between launches. To restore hidden indicators,
activate Top from the Dock and choose **Window → Menu Bar…**.

## Build from source

Install Xcode with its Metal compiler and Rust, then run:

```sh
cargo run --package top --locked
```

The Rust toolchain is pinned in `rust-toolchain.toml`. To build a local application
bundle and preview ZIP/DMG, install `jq` and run:

```sh
bash scripts/package-macos.sh --preview
```

## Install a release

Stable releases are available from [GitHub Releases](https://github.com/bshk-app/top/releases)
and the Homebrew tap:

```sh
brew install --cask bshk-app/tap/top
```

Packaged builds use Sparkle for updates. Choose **Top → Check for Updates…** for
a manual check, or **Top → Update Channel → Stable/Beta** to select the channel.
Stable is the default. Beta subscribers also receive newer stable releases.

See the [release guide](docs/releases.md) for signing, notarization, workflows,
update feeds, and cask recovery.
