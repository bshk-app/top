#!/usr/bin/env bash
# Keep private values in a caller-owned file, never in command arguments or logs.
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
output=${1:?Usage: bash scripts/export-sparkle-key.sh OUTPUT}
sparkle=$(bash "$root/scripts/setup-sparkle.sh")
umask 077
if [[ -n "${SPARKLE_PRIVATE_ED_KEY:-}" ]]; then
    printf '%s' "$SPARKLE_PRIVATE_ED_KEY" > "$output"
else
    "$sparkle/bin/generate_keys" -x "$output" >/dev/null
fi
chmod 600 "$output"
public=$(swift -module-cache-path "$root/target/swift-module-cache" "$root/scripts/sparkle-public-key.swift" "$output")
expected=$(jq -er '.sparklePublicKey' "$root/Distribution/release.json")
[[ "$public" == "$expected" ]] || { echo 'Sparkle private key does not match the key embedded in Top.' >&2; exit 1; }
