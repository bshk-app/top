#!/bin/sh
# Build all standard macOS icon representations from the PNG master.
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
source_icon="$repo_root/crates/app/assets/app-icon.png"
iconset="$repo_root/target/icon-build/AppIcon.iconset"
output_icon="$repo_root/crates/app/assets/AppIcon.icns"

mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$source_icon" \
        --out "$iconset/icon_${size}x${size}.png" >/dev/null
    retina_size=$((size * 2))
    sips -z "$retina_size" "$retina_size" "$source_icon" \
        --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done

iconutil --convert icns "$iconset" --output "$output_icon"
printf 'Built %s\n' "$output_icon"
