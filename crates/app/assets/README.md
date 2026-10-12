# Top application icon

- `app-icon.png`: 1024 × 1024 RGBA master, with transparency outside the tile.
- `AppIcon.icns`: standard macOS icon sizes from 16 to 1024 pixels, including Retina variants.

The application embeds `AppIcon.icns` and sets it through AppKit at launch, so the icon also appears when running through Cargo. The [release packaging script](../../../scripts/package-macos.sh) also includes this file as the app bundle's `CFBundleIconFile` resource.

To rebuild the ICNS file after replacing the PNG, run from the repository root:

```sh
sh scripts/build-icon.sh
```

The master was generated with the built-in imagegen tool. Generation prompt:

```text
Use case: stylized-concept
Asset type: production macOS application icon for Top, an Apple Silicon system monitor.
Primary request: a beautifully crafted, highly legible native Mac app icon that matches Top's dark charcoal interface and lime-green CPU graphs.
Subject: a single bold lime-green performance waveform / mountain-like load trace, with one dominant tall sharp peak slightly right of center and a shorter peak to its left, horizontal short endpoints. The trace is thick and confidently shaped, with gently rounded joins. It should read instantly as a live system performance monitor at 32px.
Style/medium: premium restrained macOS 3D icon, front view, soft dimensional material, clean geometry. A dark graphite rounded-square tile with continuous squircle corners and subtle beveled edges. The performance trace is a raised satin green inlay, not a harsh neon light. Very subtle green ambient reflection directly under the trace, soft upper-left light. Extremely understated barely-visible chart grid carved into the tile, without numbers.
Color palette: tile #1a1a1e to #0e0e10; green #a3d468 with slight light variation; tiny subdued edge highlights. No other colors.
Composition/framing: one icon centered on a square 1024x1024 transparent canvas, icon tile occupying about 86% of the width and height, generous safe padding around the green trace, trace spanning about 58% of canvas width. Front-facing, symmetric tile, no perspective tilt. The full rounded-square tile and its short soft shadow are visible, nothing cropped.
Constraints: actual transparent alpha outside the tile. The tile itself is opaque. Exactly one final icon, no presentation board, no multiple variants, no text, no letters, no wordmark, no watermark, no extra badges. Avoid tiny details, overloaded circuitry, stock-chart arrows, medical heart motifs, outer white backgrounds, checkerboard rendered as pixels, dramatic bloom or shiny toy-like plastic.
```
