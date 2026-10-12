#!/usr/bin/env bash
# Upload a complete notarized build to a draft, then publish it once.
set -euo pipefail

: "${GH_REPO:?Set GH_REPO to the GitHub repository}"
: "${RELEASE_TAG:?Set RELEASE_TAG to an existing version tag}"
: "${RELEASE_COMMIT:?Set RELEASE_COMMIT to the built commit}"
dist_directory=${1:?Usage: bash scripts/publish-release.sh ARTIFACT_DIRECTORY}
repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
version=${RELEASE_TAG#v}
archive_name="Top-$version-macos-arm64"

die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
[[ "$RELEASE_TAG" == "v$(cat "$repo_root/VERSION")" ]] || die 'Tag and VERSION differ.'
[[ "$RELEASE_COMMIT" =~ ^[[:xdigit:]]{40}$ ]] || die 'Release commit must be a full SHA.'
(
    cd "$dist_directory"
    shasum -a 256 --check SHA256SUMS
)
for asset in "$archive_name.zip" "$archive_name.dmg" manifest.json SHA256SUMS; do
    [[ -s "$dist_directory/$asset" ]] || die "Missing asset: $asset"
done
jq -e --arg version "$version" --arg commit "$RELEASE_COMMIT" --arg artifact "$archive_name.dmg" \
    '.releaseVersion == $version and .commit == $commit and .artifact == $artifact and .notarized == true and .stapled == true' \
    "$dist_directory/manifest.json" >/dev/null || die 'Notarization or build provenance is missing.'
[[ "$(jq -r '.sha256' "$dist_directory/manifest.json")" == "$(shasum -a 256 "$dist_directory/$archive_name.dmg" | awk '{ print $1 }')" ]] \
    || die 'DMG does not match its packaging manifest.'

# Peel annotated tags and verify the remote ref has not moved since the build.
object=$(gh api "repos/$GH_REPO/git/ref/tags/$RELEASE_TAG")
while [[ "$(jq -r '.object.type' <<< "$object")" == tag ]]; do
    object=$(gh api "repos/$GH_REPO/git/tags/$(jq -r '.object.sha' <<< "$object")")
done
[[ "$(jq -r '.object.sha' <<< "$object")" == "$RELEASE_COMMIT" ]] || die 'Remote tag does not match the built commit.'

notes_file=$(mktemp)
errors_file=$(mktemp)
trap 'rm -f "$notes_file" "$errors_file"' EXIT
awk -v version="$version" '
    index($0, "## [" version "]") == 1 { found = 1; next }
    found && /^## / { exit }
    found { print }
' "$repo_root/CHANGELOG.md" > "$notes_file"
[[ -n "$(tr -d '[:space:]' < "$notes_file")" ]] || die "CHANGELOG.md has no notes for $version."

prerelease=false
latest=true
[[ "${version%%+*}" != *-* ]] || { prerelease=true; latest=false; }
assets=("$dist_directory/$archive_name.zip" "$dist_directory/$archive_name.dmg" "$dist_directory/manifest.json" "$dist_directory/SHA256SUMS")

if release=$(gh release view "$RELEASE_TAG" --json isDraft,isImmutable 2> "$errors_file"); then
    [[ "$(jq -r '.isDraft' <<< "$release")" == true ]] || die 'Release is already published; create a new version.'
    [[ "$(jq -r '.isImmutable' <<< "$release")" != true ]] || die 'Release is immutable.'
    gh release upload "$RELEASE_TAG" "${assets[@]}" --clobber
else
    # An authentication or server error is not an absent release.
    if ! grep -Eqi 'HTTP 404|release not found|Not Found' "$errors_file"; then
        cat "$errors_file" >&2
        die 'Could not inspect the release.'
    fi
    gh release create "$RELEASE_TAG" "${assets[@]}" --verify-tag \
        --title "Top $version" --notes-file "$notes_file" --draft \
        --prerelease="$prerelease" --latest=false
fi
gh release edit "$RELEASE_TAG" --title "Top $version" --notes-file "$notes_file" \
    --draft=false --prerelease="$prerelease" --latest="$latest"
