# Native Top media

These assets show the shipped **Top 0.1.0** with real readings on an Apple M1
Mac running macOS 27.0. Capture took place on **alex-mac** on October 12, 2026,
at 23:29–23:31 **Europe/Helsinki (UTC+03:00)**. Readings are observations at
capture time and will differ later.

- [Dashboard overview](overview.png) — native CPU, GPU, ANE, memory and power tiles.
- [Menu bar and widget](menu-widget.png) — live indicators and the floating panel.
- [Native tutorial](tutorial.mp4) — 36 seconds, 1920×1080, H.264/yuv420p, 30 fps, silent.
- [Animated preview](tutorial-preview.gif) — 960×540, nominal 8 fps, about 9 seconds.

## Capture and editing

Cap Desktop **0.6.0** / CLI **0.1.0** captured the installed, signed and notarized
GitHub release. Its release hashes, installed version, Developer ID signature,
Gatekeeper assessment and stapled ticket were verified. All capture and rendering
ran on alex-mac; no parent-laptop captures were used.

The tutorial contains **8 seconds of real native footage at 1×** and **28 seconds
of PNG reading holds**. Every hold is labelled in the film. The live detail shows
actual changing WindowServer GPU readings after a real process-name search.
Menu bar, dashboard, search-field and widget views are captured stills; the film
does not claim to show their opening or an uninterrupted action sequence.

An unrelated updater overlapped the full-window recording. Its pixels are excluded:
only the verified clear process-table crop from source **49–57 seconds** is used.
The original capture stays private. Cap's export cursor visibility was disabled,
and all delivered crops were checked for baked cursors and occlusion. No cursor,
synthetic UI, fabricated reading, camera, microphone or system audio appears.
Temporary light appearance and search state were restored to dark and empty;
the 1-second interval, live state and widget visibility were unchanged.

ANE load is a power-based estimate, not occupancy. Unavailable counters remain
as shown by the app. No standalone Settings window, process termination or update
installation is demonstrated.

## Portable source

The [HyperFrames project](tutorial/PRODUCTION.md) uses local Inter WOFF2 with its
OFL license, local GSAP with its original license header, Top's actual icon and
relative application-only assets. Framing and captions sit outside native pixels.
No grading or interface reconstruction was applied. HyperFrames is pinned to
**0.8.149**; telemetry is disabled in the documented commands.

[Production notes](tutorial/PRODUCTION.md) record exact source ranges and crops.
[QA results](qa-results.json), [encoded-frame contact sheet](preview-contact-sheet.png)
and [file manifest](deliverables-manifest.json) record the completed checks and hashes.
