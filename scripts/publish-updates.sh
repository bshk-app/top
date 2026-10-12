#!/usr/bin/env bash
# Publish a signed channel-aware feed and the stable Homebrew cask from shipped assets.
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
dist=${1:?Usage: bash scripts/publish-updates.sh ARTIFACT_DIRECTORY}
dist=$(cd "$dist" && pwd)
repository=$(jq -er '.repository' "$root/Distribution/release.json")
tap=$(jq -er '.tap' "$root/Distribution/release.json")
branch=$(jq -er '.appcastBranch' "$root/Distribution/release.json")
export GH_REPO="$repository"
: "${RELEASE_TAG:?Set RELEASE_TAG}"
version=$(jq -er '.releaseVersion' "$dist/manifest.json")
if [[ "${version%%+*}" != *-* ]]; then
    : "${TAP_GITHUB_TOKEN:?Set TAP_GITHUB_TOKEN}"
fi
commit=$(git -C "$root" rev-parse HEAD)
[[ "$RELEASE_TAG" == "v$version" ]] || { echo 'Release tag and manifest differ.' >&2; exit 1; }
jq -e --arg commit "$commit" '.commit == $commit and .notarized == true and .stapled == true' \
    "$dist/manifest.json" >/dev/null || { echo 'Expected notarized assets from this commit.' >&2; exit 1; }
(cd "$dist" && shasum -a 256 --check SHA256SUMS)
zip="$dist/Top-$version-macos-arm64.zip"
dmg="$dist/$(jq -er '.artifact' "$dist/manifest.json")"
[[ "$(shasum -a 256 "$dmg" | awk '{ print $1 }')" == "$(jq -er '.sha256' "$dist/manifest.json")" ]] || exit 1
release=$(gh release view "$RELEASE_TAG" --json isDraft,assets)
[[ "$(jq -r '.isDraft' <<< "$release")" == false ]] || { echo 'Publish the GitHub release first.' >&2; exit 1; }
for asset in "$zip" "$dmg"; do
    digest=$(gh api "repos/$repository/releases/tags/$RELEASE_TAG" \
        --jq ".assets[] | select(.name == \"$(basename "$asset")\") | .digest")
    [[ "$digest" == "sha256:$(shasum -a 256 "$asset" | awk '{ print $1 }')" ]] \
        || { echo 'Local archive differs from the published GitHub asset.' >&2; exit 1; }
done

work=$(mktemp -d)
trap 'rm -f "$work/ed-key"' EXIT
sparkle=$(bash "$root/scripts/setup-sparkle.sh")
bash "$root/scripts/export-sparkle-key.sh" "$work/ed-key"
notes="$work/notes.md"
awk -v version="$version" '
    index($0, "## [" version "]") == 1 { found = 1; next }
    found && /^## / { exit }
    found { print }
' "$root/CHANGELOG.md" > "$notes"
[[ -n "$(tr -d '[:space:]' < "$notes")" ]] || { echo 'Release notes are missing.' >&2; exit 1; }

# Never treat an authentication or server failure as an empty update history.
if ! gh api "repos/$repository/contents/appcast.xml?ref=$branch" \
    -H 'Accept: application/vnd.github.raw+json' > "$work/existing.xml" 2> "$work/error"; then
    grep -q 'HTTP 404' "$work/error" || { cat "$work/error" >&2; exit 1; }
    rm -f "$work/existing.xml"
fi
if [[ -f "$work/existing.xml" ]]; then
    "$sparkle/bin/sign_update" --ed-key-file "$work/ed-key" --verify "$work/existing.xml"
fi
signature=$("$sparkle/bin/sign_update" --ed-key-file "$work/ed-key" "$zip")
url="https://github.com/$repository/releases/download/$RELEASE_TAG/$(basename "$zip")"
swift -module-cache-path "$root/target/swift-module-cache" "$root/scripts/build-appcast.swift" \
    "$dist/manifest.json" "$notes" "$work/existing.xml" "$work/appcast.xml" "$signature" "$url"
for feed in "$work"/appcast*.xml; do
    "$sparkle/bin/sign_update" --ed-key-file "$work/ed-key" --disable-signing-warning "$feed"
    "$sparkle/bin/sign_update" --ed-key-file "$work/ed-key" --verify "$feed"
    xmllint --noout "$feed"
done

if ! gh api "repos/$repository/git/ref/heads/$branch" > "$work/ref.json" 2> "$work/error"; then
    grep -q 'HTTP 404' "$work/error" || { cat "$work/error" >&2; exit 1; }
    jq -n --arg ref "refs/heads/$branch" --arg sha "$commit" '{ref:$ref,sha:$sha}' > "$work/ref-input.json"
    gh api -X POST "repos/$repository/git/refs" --input "$work/ref-input.json" > "$work/ref.json"
fi
base=$(jq -er '.object.sha' "$work/ref.json")
tree=$(gh api "repos/$repository/git/commits/$base" --jq '.tree.sha')
nodes='[]'
for feed in "$work"/appcast*.xml; do
    jq -n --rawfile content "$feed" '{content:$content,encoding:"utf-8"}' > "$work/blob.json"
    blob=$(gh api -X POST "repos/$repository/git/blobs" --input "$work/blob.json" --jq '.sha')
    nodes=$(jq --arg path "$(basename "$feed")" --arg sha "$blob" \
        '. + [{path:$path,mode:"100644",type:"blob",sha:$sha}]' <<< "$nodes")
done
jq -n --arg base "$tree" --argjson nodes "$nodes" '{base_tree:$base,tree:$nodes}' > "$work/tree.json"
new_tree=$(gh api -X POST "repos/$repository/git/trees" --input "$work/tree.json" --jq '.sha')
jq -n --arg message "release: update Top feed for $RELEASE_TAG" --arg tree "$new_tree" --arg parent "$base" \
    '{message:$message,tree:$tree,parents:[$parent]}' > "$work/commit.json"
new_commit=$(gh api -X POST "repos/$repository/git/commits" --input "$work/commit.json" --jq '.sha')
jq -n --arg sha "$new_commit" '{sha:$sha,force:false}' > "$work/ref-update.json"
gh api -X PATCH "repos/$repository/git/refs/heads/$branch" --input "$work/ref-update.json" --silent

if [[ "${version%%+*}" != *-* ]]; then
    newest_stable=$(xmllint --xpath 'string(/rss/channel/item[1]/*[local-name()="shortVersionString"])' "$work/appcast-stable.xml")
fi
if [[ "${version%%+*}" != *-* && "$newest_stable" == "$version" ]]; then
    cask="$work/top.rb"
    ruby "$root/scripts/render-cask.rb" "$dist/manifest.json" > "$cask"
    ruby -c "$cask"
    path="repos/$tap/contents/Casks/top.rb"
    old_sha=''
    if previous=$(GH_TOKEN="$TAP_GITHUB_TOKEN" gh api "$path" 2> "$work/error"); then
        old_sha=$(jq -er '.sha' <<< "$previous")
    else
        grep -q 'HTTP 404' "$work/error" || { cat "$work/error" >&2; exit 1; }
    fi
    encoded=$(base64 < "$cask" | tr -d '\n')
    jq -n --arg content "$encoded" --arg sha "$old_sha" --arg message "release: update Top cask to $version" \
        '{message:$message,content:$content} + (if $sha == "" then {} else {sha:$sha} end)' > "$work/cask.json"
    GH_TOKEN="$TAP_GITHUB_TOKEN" gh api -X PUT "$path" --input "$work/cask.json" --silent
fi

# Check the public consumer path, including the release file and signed feed.
curl --fail --location --silent --show-error --retry 5 --retry-delay 3 \
    "https://raw.githubusercontent.com/$repository/$new_commit/appcast.xml" > "$work/published.xml"
cmp "$work/appcast.xml" "$work/published.xml"
curl --fail --location --silent --show-error --head "$url" >/dev/null
printf 'Published %s to the Top update feed%s.\n' "$RELEASE_TAG" "$( [[ "${version%%+*}" == *-* ]] || printf ' and Homebrew cask' )"
