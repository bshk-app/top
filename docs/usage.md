# Using Top

[README](../README.md) · [Development](development.md) · [Releases](releases.md)

This guide describes the shipped **0.1.0** application. The development checkout
may contain newer interface changes. Top requires an Apple Silicon Mac running
macOS 14 or later.

## Install and launch

Install through Homebrew:

```sh
brew install --cask bshk-app/tap/top
```

Alternatively, download the Apple Silicon DMG from
[GitHub Releases](https://github.com/bshk-app/top/releases), open it, and move
Top.app to Applications. Launch Top from Applications. The initial readings
need a sampling interval before rate counters become useful.

The release opens the dashboard and floating widget and creates menu bar
indicators. Closing a view leaves the app running. Choose **Top → Quit Top** or
press **⌘Q** to stop it.

## Menu bar and popover

Click a Top indicator to open the compact popover. It shows headline metrics,
recent power readings, thermal/battery facts, and the busiest processes.

Under **Menu bar**, choose which indicators remain visible:

| Indicator | Reading |
| --- | --- |
| Sensors | Available power and temperature readings |
| CPU | Overall CPU usage |
| GPU | Available GPU usage |
| RAM | Memory usage |
| SSD | Startup disk space used |
| Network | Received and sent data rates |

You can disable every indicator, including Network. Disabled indicators are
removed from the menu bar, and the selection is saved between launches. To
restore them, activate Top from the Dock and open **Top → Settings** (**⌘,**),
then select **Menu Bar**. Hiding indicators does not stop the shared sampler
used by the dashboard and floating widget. The **SSD** indicator describes
disk space, while **Disk I/O** in the dashboard describes current read/write
activity.

Select **Open Top** to show the dashboard. **Freeze/Resume** controls live
updates across the views. **Escape** closes the popover; **Quit** stops Top.

## Dashboard

The dashboard combines headline tiles, system facts, and a process table.
Choose **0.5s**, **1s**, or **2s** in the title bar to change the update interval.
The default is one second. Recent charts keep a short in-memory history.

Use **Freeze** to hold readings while inspecting them, then **Resume** to
continue. This holds the display; it does not suspend the processes being
measured. The live indicator and status bar explain the current state.

Use **Light/Dark** to change the current appearance. The tile layout menu offers
automatic sizing or one to four columns. Drag a tile onto another to rearrange
the order, or use its context menu. **Reset layout** restores the default.
Tile arrangement and column choices persist between launches.

## Process list

Search by command name, user, or PID. The search is case-insensitive. Click
column headings to change sorting and use the arrow keys to select a row.
Selection follows the PID when live sorting moves the row.

| Field | Meaning |
| --- | --- |
| CPU | Share of one CPU core; a multithreaded process can exceed 100% |
| MEM / RES | Memory percentage and resident memory |
| Time | Accumulated CPU time |
| PID / User / Command | Process identity and command |

Some per-process counters depend on macOS access and hardware availability.
Use the displayed readings as observations over the sampling interval.

### Ending a process

The selected row's quit control, **Backspace**, or **Delete** opens confirmation.
Top sends **SIGTERM**, then **SIGKILL after three seconds** if necessary.
Unsaved work in that process can be lost. Verify the displayed process before
confirming. A process that exits or changes identity is handled separately.

## Floating widget

Choose **Window → Floating Widget** to show or hide the compact, always-on-top
view. It uses the same readings and tile order as the dashboard. Drag it to a
useful position; its close button hides it. Choose **Window → Dashboard** or
press **⌘0** to return to the main view.

## Shortcuts

| Shortcut | Action |
| --- | --- |
| ⌘0 | Show the dashboard |
| ⌘Q | Quit Top |
| F | Freeze/resume, while the dashboard is focused |
| / | Focus process search |
| ↑ / ↓ | Move process selection |
| Backspace / Delete | Request confirmation to end the selected process |
| Escape | Return focus from dashboard search, or close the popover |

Single-key dashboard actions do not fire while typing in the search field.

## Understanding the readings

CPU and GPU readings describe sampled activity. Memory, disk space, disk I/O,
and network traffic measure different things; their percentages and rates
should not be compared as if they shared one scale.

ANE load is estimated from the Neural Engine's power draw using a fixed
reference. It is not an occupancy measurement or a chip-calibrated benchmark.
ANE memory traffic is a separate counter and may be unavailable. Power,
frequency, and temperature coverage depends on the chip and macOS interfaces.
Unavailable sensor values appear as dashes rather than fabricated readings.

Charts are recent observations, not a durable monitoring log. The first rate
reading needs a baseline, and live process rows can disappear when processes
exit. Top is not a cross-platform monitor or a benchmark suite.

## Storage and updates

Top reads local macOS counters for its metrics. The app-owned preferences live
in `~/Library/Application Support/Top`:

| File | Preference |
| --- | --- |
| `layout` | Tile order and columns |
| `menu-bar` | Visible indicators |
| `update-channel` | Stable or Beta |

Recent metric history stays in memory. A preference that cannot be written can
still apply for the current session. The app does not require an account.

Packaged releases include Sparkle. Choose **Top → Check for Updates…** for a
manual check and **Top → Update Channel** to choose **Stable** or **Beta**.
Stable is the default. Beta subscribers also accept newer stable items;
switching to Stable does not downgrade an installed beta. Automatic checks
follow Sparkle's permission prompt and saved preference.

Update feeds and downloads are hosted on GitHub, which receives ordinary
connection metadata. Sparkle profile reporting is disabled in the release
configuration. Local metrics collection is separate from this update traffic;
do not interpret it as a promise that the application makes no network requests.

## Troubleshooting

- **A sensor shows a dash:** its underlying reading may be unavailable on this
  chip or macOS version. Check other metrics independently.
- **Readings do not move:** check whether the interface says **Frozen**, then
  select **Resume**. Allow an interval for the next sample.
- **A process is missing:** clear the search, check sorting, or confirm that
  the process is still running. macOS can restrict individual process readings.
- **Update menus are absent in a source run:** use the packaged application
  containing Sparkle to check managed updates.
- **An update check fails:** retry when GitHub is reachable, or install the
  latest verified release directly. [Release recovery](releases.md#manual-releases-and-recovery)
  describes maintainer operations.
