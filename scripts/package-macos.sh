#!/usr/bin/env bash
# Local previews and CI releases share the same unsigned build and packaging.
set -euo pipefail

mode=${1:---preview}
case "$mode" in
    --preview|--release) mode=${mode#--} ;;
    *) printf 'Usage: bash scripts/package-macos.sh [--preview|--release]\n' >&2; exit 1 ;;
esac

if [ "$(uname -s)" != "Darwin" ] || [ "$(uname -m)" != "arm64" ]; then
    printf '%s\n' 'Packaging requires an Apple Silicon Mac.' >&2
    exit 1
fi

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

export MACOSX_DEPLOYMENT_TARGET=14.0
metadata=$(cargo metadata --no-deps --locked --format-version 1)
version=$(printf '%s\n' "$metadata" | jq -er '.packages[] | select(.name == "top") | .version')
[[ "$(cat VERSION)" == "$version" ]] || { echo 'VERSION and Cargo.toml differ.' >&2; exit 1; }
target_directory=$(printf '%s\n' "$metadata" | jq -er '.target_directory')
bundle_version=${version%%[-+]*}
commit=$(git rev-parse --verify HEAD 2>/dev/null || true)
build_number=${BUILD_NUMBER:-$(git rev-list --count HEAD 2>/dev/null || printf '1')}
[[ "$build_number" =~ ^[1-9][0-9]*$ ]] || { echo 'BUILD_NUMBER must be a positive integer.' >&2; exit 1; }
app_bundle="$target_directory/bundle/$version/$mode/Top.app"
dmg_directory="$target_directory/dmg/$version/$mode"
dist_directory="$target_directory/dist/$version/$mode"
package_directory="$target_directory/package/$version"
archive_name="Top-$version-macos-arm64"
sparkle_directory=$(bash scripts/setup-sparkle.sh)
sparkle_public_key=$(jq -er '.sparklePublicKey' Distribution/release.json)
repository=$(jq -er '.repository' Distribution/release.json)
feed_branch=$(jq -er '.appcastBranch' Distribution/release.json)

if [[ "$mode" == release ]]; then
    command -v zamokctl >/dev/null || { echo 'zamokctl is required for signed releases.' >&2; exit 1; }
    signing_identity=${RELEASE_SIGNING_IDENTITY_SHA1:-}
    if [[ -z "$signing_identity" ]]; then
        identity_name=${RELEASE_DEVELOPER_ID_APPLICATION:-Developer ID Application: Aleksandr Beshkenadze (Q8H6GWJ658)}
        signing_identity=$(security find-identity -v -p codesigning | awk -v name="$identity_name" 'index($0, name) { print $2; exit }')
    fi
    [[ "$signing_identity" =~ ^[[:xdigit:]]{40}$ ]] || { echo 'No matching Developer ID signing identity.' >&2; exit 1; }
    notary_args=()
    if [[ -n "${RELEASE_NOTARY_PROFILE:-}" ]]; then
        notary_args=(--notary-profile "$RELEASE_NOTARY_PROFILE")
    else
        : "${NOTARY_KEY_PATH:?Set RELEASE_NOTARY_PROFILE or NOTARY_KEY_PATH}"
        : "${NOTARY_KEY_ID:?Set NOTARY_KEY_ID}"
        : "${NOTARY_ISSUER:?Set NOTARY_ISSUER}"
        [[ -f "$NOTARY_KEY_PATH" ]] || { echo 'Notary key file is missing.' >&2; exit 1; }
        notary_args=(--notary-api-key "$NOTARY_KEY_PATH" --notary-key-id "$NOTARY_KEY_ID" --notary-issuer "$NOTARY_ISSUER")
    fi
fi

cargo build --package top --release --locked

mkdir -p "$app_bundle/Contents/MacOS" "$app_bundle/Contents/Resources" "$app_bundle/Contents/Frameworks" \
    "$dmg_directory" "$dist_directory"
install -m 755 "$target_directory/release/top" "$app_bundle/Contents/MacOS/top"
install -m 644 crates/app/assets/AppIcon.icns "$app_bundle/Contents/Resources/AppIcon.icns"
install -m 644 "$sparkle_directory/LICENSE" "$app_bundle/Contents/Resources/Sparkle-LICENSE"
ditto "$sparkle_directory/Sparkle.framework" "$app_bundle/Contents/Frameworks/Sparkle.framework"

cat > "$app_bundle/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>Top</string>
    <key>CFBundleExecutable</key>
    <string>top</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon.icns</string>
    <key>CFBundleIdentifier</key>
    <string>app.bshk.top</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Top</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$bundle_version</string>
    <key>CFBundleVersion</key>
    <string>$build_number</string>
    <key>LSMinimumSystemVersion</key>
    <string>$MACOSX_DEPLOYMENT_TARGET</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>SUFeedURL</key>
    <string>https://raw.githubusercontent.com/$repository/$feed_branch/appcast.xml</string>
    <key>SUPublicEDKey</key>
    <string>$sparkle_public_key</string>
    <key>SURequireSignedFeed</key>
    <true/>
    <key>SUVerifyUpdateBeforeExtraction</key>
    <true/>
    <key>SUSendProfileInfo</key>
    <false/>
</dict>
</plist>
EOF
printf 'APPL????' > "$app_bundle/Contents/PkgInfo"
plutil -lint "$app_bundle/Contents/Info.plist"

if [[ "$mode" == release ]]; then
    # The same tool used by Murmator and Tishina signs, notarizes and staples
    # both the staged app and the DMG, and records its digest in manifest.json.
    zamokctl package --input "$app_bundle" --output-dir "$package_directory" \
        --signing-identity-sha1 "$signing_identity" "${notary_args[@]}" --format dmg
    ditto "$package_directory/staging/Top.app" "$app_bundle"
    cp "$package_directory/Top-$bundle_version.dmg" "$dist_directory/$archive_name.dmg"
    jq --arg artifact "$archive_name.dmg" --arg version "$version" --arg commit "$commit" \
        '.artifact = $artifact | .releaseVersion = $version | .commit = $commit' \
        "$package_directory/manifest.json" > "$dist_directory/manifest.json"
    xcrun stapler validate "$app_bundle"
    xcrun stapler validate "$dist_directory/$archive_name.dmg"
    spctl --assess --type execute --verbose=2 "$app_bundle"
else
    codesign --force --deep --sign - "$app_bundle"
    ditto "$app_bundle" "$dmg_directory/Top.app"
    ln -sfn /Applications "$dmg_directory/Applications"
    hdiutil create -volname Top -srcfolder "$dmg_directory" -format UDZO \
        -ov "$dist_directory/$archive_name.dmg"
    dmg_sha=$(shasum -a 256 "$dist_directory/$archive_name.dmg" | awk '{ print $1 }')
    jq -n --arg artifact "$archive_name.dmg" --arg sha256 "$dmg_sha" \
        --arg version "$version" --arg short "$bundle_version" --arg build "$build_number" --arg commit "$commit" \
        '{artifact: $artifact, sha256: $sha256, releaseVersion: $version, shortVersion: $short,
          bundleVersion: $build, bundleId: "app.bshk.top", appBundleName: "Top.app", minMacOS: "14.0",
          commit: $commit, notarized: false, stapled: false}' \
        > "$dist_directory/manifest.json"
fi
codesign --verify --deep --strict --verbose=2 "$app_bundle"
ditto -c -k --sequesterRsrc --keepParent "$app_bundle" "$dist_directory/$archive_name.zip"

(
    cd "$dist_directory"
    shasum -a 256 "$archive_name.zip" "$archive_name.dmg" manifest.json > SHA256SUMS
    shasum -a 256 --check SHA256SUMS
)
printf 'Release artifacts: %s\n' "$dist_directory"
