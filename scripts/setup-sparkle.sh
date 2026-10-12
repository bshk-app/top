#!/usr/bin/env bash
# Use the same checksum-pinned Sparkle distribution for the framework and tools.
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
version=$(jq -er '.sparkleVersion' "$root/Distribution/release.json")
digest=$(jq -er '.sparkleSHA256' "$root/Distribution/release.json")
directory="$root/target/sparkle/$version"
mkdir -p "$directory"
if [[ ! -f "$directory/Sparkle.tar.xz" ]]; then
    curl --fail --location --silent --show-error --retry 3 \
        "https://github.com/sparkle-project/Sparkle/releases/download/$version/Sparkle-$version.tar.xz" \
        -o "$directory/Sparkle.tar.xz"
fi
printf '%s  %s\n' "$digest" "$directory/Sparkle.tar.xz" | shasum -a 256 --check >&2
if [[ ! -x "$directory/bin/sign_update" || ! -d "$directory/Sparkle.framework" ]]; then
    tar -xJf "$directory/Sparkle.tar.xz" -C "$directory"
fi
printf '%s\n' "$directory"
