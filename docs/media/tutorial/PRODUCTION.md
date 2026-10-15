# Completed native media production

Status: **complete**. Capture and rendering were local on alex-mac. The source
contains only validated Top crops, local libraries, licenses and relative paths.
Raw Cap projects, full-screen originals, control logs and caches are excluded.

## Deliverables

| File | Format | Timing |
| --- | --- | --- |
| `../overview.png` | 1600×960 PNG | Native dashboard metrics cover |
| `../menu-widget.png` | 1600×960 PNG | Native indicators and widget cover |
| `../tutorial.mp4` | 1920×1080 H.264/yuv420p | 30 fps, 1080 frames, 36 seconds, no audio |
| `../tutorial-preview.gif` | Opaque 960×540 GIF | Nominal 8 fps, 72 frames, 9.01 seconds |

The GIF's centisecond timing rounds the authored 9 seconds to 9.01 seconds.
Exact sizes, hashes and codec metadata are in `../deliverables-manifest.json`.

## Final source schedule

| Output time | Chapter | Input |
| --- | --- | --- |
| 0–7 s | Read the menu bar | `assets/menu-bar.png`, labelled PNG hold |
| 7–15 s | Open the dashboard | `assets/dashboard-metrics.png`, labelled PNG hold |
| 15–20 s | Search a real process | `assets/process-search.png`, labelled PNG hold |
| 20–28 s | Inspect the result | `assets/process-detail.mp4`, source 0–8 s, 1× |
| 28–36 s | Keep the widget nearby | `assets/widget.png`, labelled PNG hold |

The teaser uses dashboard PNG 0–3 s, live detail 3–6 s (source 0–3 s, 1×),
and widget PNG 6–9 s. Its live table viewport omits blank right-hand space.

The real-time asset comes from a Cap native-window recording of Top:

- Original display segment: **2482×1800**, duration **100.810756 s**.
- Selected original source range: **49–57 s**; filtered WindowServer results
  are already present. Typing and full-window actions are not shown in the film.
- Physical-pixel crop: **x=32, y=1204, width=2418, height=210**.
- Cap aligns the encoded width to **2420×210**, 30 fps, 8 seconds, silent.
- The unrelated updater and real pointer are outside this crop. `cursor.hide`
  is true; camera/audio are hidden or muted; background shadow, rounding,
  padding, frame, blur and grading are disabled.
- The raw project is preserved privately. Export used Cap's FFmpeg decoder.
  First and later source/export frames were compared, and changing GPU values
  survived export and the final encode. No CFR intermediate was needed for
  this accepted crop; no source range extends past EOF.

Screenshot inputs use these exact physical-pixel crops:

| Input | Original source | x, y, width, height |
| --- | --- | --- |
| `dashboard-metrics.png` | 2482×1800 native Top window PNG | 0, 0, 2482, 1090 |
| `process-search.png` | 2482×1800 native Top window PNG | 32, 1112, 2418, 300 |
| `menu-bar.png` | 3840×2160 Cap screen PNG | 1570, 0, 750, 58 |
| `widget.png` | Same Cap screen PNG | 3192, 120, 576, 444 |

The widget crop stays inside the panel's outer margins and preserves every
visible label and close control. The indicator crop contains only Top's status
items. Each PNG input was compared pixel-for-pixel with its original source crop.
No source UI pixels were painted over, graded or recreated.

## Actual verification

- Original and cropped working Cap projects passed validation; stop reported
  `recordingMetaExists: true`.
- Main composition, teaser and both covers passed lint, runtime, layout and
  contrast checks with zero errors and zero warnings.
- Main contrast: 34/34; teaser: 18/18; each cover: 20/20.
- Animation map reviewed all 10 main tweens: five heading entrances and five
  chapter progress fills. No animation is applied to native app pixels.
- Both delivered covers were inspected at full resolution. Scene snapshots,
  the encoded contact sheet, live frames and all four cut boundaries were reviewed.
- Encoded MP4 frames 0, 90, 209, 210, 449, 450, 599, 600, 660, 720, 839, 840 and
  1079 were inspected. The final frame is the native widget, with no black tail.
- GIF first, both cut boundaries, live midpoint and last frames were reviewed;
  every pixel of all 72 decoded GIF frames has alpha 255.
- FFprobe confirmed MP4 codec, dimensions, frame rate, duration, frame count and
  absence of audio. A full MP4 blackdetect scan found no black interval.
- Relative asset paths and internal asset symlinks were audited. No private
  source, log, runtime cache, personal personal account label or absolute machine path is packaged.

Main render: screenshot capture, hardware Apple M1 GPU, two workers;
66.7 seconds wall time, about 60.6 seconds capture and 4.9 seconds encode.
The teaser rendered locally in about 12.6 seconds with one worker.

## Reproduce

The HTML already contains the final timing. No private assembler or control script
is needed. Preserve relative internal asset links when unpacking or copying the
project; the archive preserves them. Licenses are in `ASSET-LICENSES.md`.

From this project directory, with Node.js and FFmpeg installed:

```sh
HYPERFRAMES_SKIP_SKILLS=1 HYPERFRAMES_NO_TELEMETRY=1 npx --yes hyperframes@0.8.149 check
HYPERFRAMES_SKIP_SKILLS=1 HYPERFRAMES_NO_TELEMETRY=1 npx --yes hyperframes@0.8.149 render --fps 30 --quality delivery --video-frame-format png --strict-all --workers 2 --frames-cache-dir off --output renders/tutorial.mp4
```

From `teaser/`:

```sh
HYPERFRAMES_SKIP_SKILLS=1 HYPERFRAMES_NO_TELEMETRY=1 npx --yes hyperframes@0.8.149 check
HYPERFRAMES_SKIP_SKILLS=1 HYPERFRAMES_NO_TELEMETRY=1 npx --yes hyperframes@0.8.149 render --fps 8 --quality delivery --video-frame-format png --strict-all --workers 1 --frames-cache-dir off --format gif --gif-loop 0 --output ../renders/tutorial-preview.gif
```

For either `covers/<name>/`, run its `check`, then
`snapshot --at 0.5 --no-end --describe false`; the midpoint PNG is the cover.
No CDN, cloud renderer, Cap upload, public hosting, paid processing or audio
is required. The project can also be edited locally in HyperFrames Studio.
