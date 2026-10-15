# README and native media plan

This document retains the planning guidance handed off from KnowYourLimits
and adapts it to Top's actual source and distribution. Production is complete
on alex-mac; the [media provenance](media/README.md) records the delivered
36-second tour, native sources, and verification. The scenes below are the
original proposed plan, rather than a claim about every delivered scene.

Reference: [KnowYourLimits at e2e9527](https://github.com/bshk-app/KnowYourLimits/tree/e2e9527),
including its README, user/developer guides, media provenance, and portable
HyperFrames tutorial. Reuse the editorial and verification approach; capture
Top itself and write Top-specific claims, captions, and instructions.

## Product facts to preserve

- Top is a native Apple Silicon system monitor for macOS 14 or later, built with
  Rust and GPUI Kit. Intel Macs, Windows, and Linux are not release targets.
- The shipped surfaces are a dashboard, configurable menu bar indicators and
  popover, a floating widget, and a searchable/sortable process list. There is
  no separate Settings window to feature in a screenshot.
- Metrics include CPU, GPU, memory, disk/network activity, and available
  power/temperature readings. ANE load is estimated from power draw, not measured
  occupancy. Hardware counters can be unavailable; describe the real display.
- The dashboard supports tile arrangements, Freeze/Resume, and process search.
  Process termination has a confirmation flow; it is unnecessary for this tour.
- Signed, notarized release `0.1.0` and the Homebrew cask were verified on
  October 12, 2026. Recheck the current release before editing installation text.
  The install command is `brew install --cask bshk-app/tap/top`.
- Packaged builds use Sparkle with Stable/Beta channels. Stable is the default.
  Source runs without the packaged framework do not demonstrate updater menus.
- Settings for tile layout, menu bar items, and update channel are stored under
  `~/Library/Application Support/Top`. Metrics are read locally. Update checks
  and downloads contact GitHub; avoid an absolute claim of no network activity.

Sources: [application entry point](../crates/app/src/main.rs),
[menu bar/popover](../crates/app/src/popover.rs),
[metric definitions](../crates/metrics/src/snapshot.rs),
[settings](../crates/app/src/settings.rs), and [release guide](releases.md).

## README structure

Keep the README short and oriented toward someone installing Top:

1. Existing Top icon and one useful sentence, for example: “See your Mac's
   activity in the menu bar, dashboard, and process list.”
2. A restrained navigation row and the Apple Silicon/macOS 14+ requirement.
3. One sharp overview image showing the actual dashboard.
4. Four brief features: live metrics, configurable menu bar readings, process
   search/sorting, and the floating widget. Keep ANE's estimate caveat explicit.
5. A short tutorial preview linked to a real MP4, once both files exist.
6. Homebrew installation and the verified GitHub Releases link.
7. Three first-use steps: launch Top; click a menu bar indicator and choose
   visible metrics; open the dashboard to search processes or show the widget.
8. Brief privacy/measurement limits and guide links.

Create `docs/usage.md` for controls, shortcuts, indicator meanings, unavailable
readings, storage, and updates. Create `docs/development.md` for Rust/Xcode/Metal
prerequisites, source builds, tests, and project structure. Keep operational
release details in the existing release guide. Add links only after the target
files exist. Use English throughout code, documentation, comments, and captions.

Do not add speculative badges, prices, benchmarks, platform support, or account
features. The reference project's quota, provider, and Windows claims do not
describe Top.

## Native captures and visual direction

Capture the actual signed Top app on alex-mac. Read the installed Cap and
cap-demo skills and inspect Cap's CLI before recording. Use Cap's native window
capture first. If a screen recording is needed for the menu bar/popover, keep
the raw desktop capture private and export only validated application crops.
Check the execution context before requesting duplicate capture permissions;
respect explicit Computer Use denials.

Produce two or three screenshots:

| Proposed file | Real state to capture |
| --- | --- |
| `docs/media/overview.png` | Dashboard with legible metric tiles and process list |
| `docs/media/menu-bar.png` | Top indicators and the open popover with its metric choices |
| `docs/media/widget.png` | Floating widget, if it adds useful information to the guide |

Use Top's own icon and palette from its theme file: a paper-like `#f8f8fa`
outer canvas, `#1f2432` caption text, and restrained `#4d7c0f` accent. Keep the
captured light/dark interface unchanged. Compose 1600×960 covers in HyperFrames
with generous margins, local Inter, and sharp UI at a legible scale. Decoration
and captions belong outside the application crop. No regenerated UI, pixel
grading, synthetic cursor, fabricated readings, or reference-app screenshots.
Final screenshots and video must contain no cursor. Configure capture/export
cursor visibility or keep the real pointer outside the app crop; inspect and
recapture any contaminated frame rather than painting over native UI pixels.

Process names and user labels can be sensitive even inside an app crop. Inspect
them before publication and recapture a naturally nonsensitive view when needed.

## Tutorial narrative

Target a silent 45–55 second tutorial at 1920×1080, 30 fps, H.264/yuv420p. Use
real footage at 1× speed, clear English captions, small chapter progress, and
honest PNG holds for reading. These are proposed scenes, not captured timestamps:

| Scene | Action and purpose |
| --- | --- |
| Introduction | Hold the actual dashboard and introduce the three surfaces |
| Read at a glance | Click a menu bar indicator and inspect the popover |
| Choose your indicators | Change one visible metric choice and restore it |
| Find a process | Open the dashboard, sort by CPU, and search a real process |
| Keep metrics nearby | Show the floating widget and finish on a readable native view |

Record exact actions, source in/out times, app version, and observed states.
Adjust scene lengths to the real footage. Avoid demonstrating process
termination, installing an update, or manufacturing a workload for the tour.
Restore any temporary display preferences after capture.

Produce a short, opaque GIF teaser linked to `tutorial.mp4`; the reference's
960×540 at 8 fps is a useful starting point. Choose Top's final length and size
after checking readability and GitHub playback.

## Verification and deliverables

- Preserve original Cap sources privately. If sparse variable-frame-rate
  footage freezes during decoding, make isolated lossless CFR derivatives and
  verify their actual states and hold times before export.
- Run HyperFrames lint/runtime/layout/contrast checks, then inspect a contact
  sheet and the encoded video's first, action, transition, and final frames.
  Check source ranges against EOF, readable controls, no black tail, and an
  opaque GIF background. Duration/codec checks alone are insufficient.
- Add media provenance in `docs/media/README.md`: captured Top version, date,
  capture/render tools, real interactions, source ranges, hashes, and final
  encoding metadata. Do not claim checks that have not been performed.
- If included, keep `docs/media/tutorial/` portable: local GSAP with its license
  header, local Inter WOFF2 and OFL, relative asset paths, and app-only exports.
  Exclude raw desktop `.cap` projects, accounts, control logs, caches, snapshots,
  runtime state, credentials, and absolute machine paths.
- Verify every README and local HTML asset link before committing. Keep media
  and documentation changes separate from product behavior changes.
- For a remotely viewed local preview, expose its port with `bb connect expose`
  and share the returned public link. No cloud upload, paid processing, or email
  is needed for this workflow.

Optional writing references from the handoff are awesome-copilot's
`skills/create-readme/SKILL.md` and agent-toolkit's
`skills/crafting-effective-readmes/README.md`. Read them if used. The mentioned
`study8677/Readme.skill` is a personal profile skill, not a project README tool;
personal conversation history is outside this task.
