# macOS releases

Top uses the release approach from Murmator and Tishina: build an unsigned app,
sign with Developer ID and hardened runtime, notarize and staple the app and DMG,
verify the artifacts, upload them to a draft, then publish the completed release.
The local and CI entry point is [scripts/package-macos.sh](../scripts/package-macos.sh).
Top targets Apple Silicon and macOS 14 or later; its bundle identifier is
`app.bshk.top`. The repository is `bshk-app/top`, and the Homebrew tap is
`bshk-app/homebrew-tap`.

## Repository setup

Push the project to a GitHub repository with `main` as its default branch. Enable
Actions and **Settings → Actions → General → Allow GitHub Actions to create and
approve pull requests**, which release-please needs to maintain its release PR.

Configure these repository or organization secrets, using the same names as
Murmator. Organization secrets must grant this repository access.

| Secret | Value |
| --- | --- |
| `DEVELOPER_ID_P12` | Base64-encoded Developer ID Application certificate and private key export |
| `DEVELOPER_ID_P12_PASSWORD` | Password protecting that export |
| `NOTARY_KEY_P8` | Base64-encoded App Store Connect API private key |
| `NOTARY_KEY_ID` | API key ID |
| `NOTARY_ISSUER` | API issuer UUID |
| `RELEASE_SIGNING_IDENTITY_SHA1` | Optional: exact certificate SHA1; otherwise resolve the existing Developer ID by name |
| `SPARKLE_PRIVATE_ED_KEY` | Base64-encoded 32-byte Ed25519 seed matching `Distribution/release.json`; falls back to the shared organization secret `SPARKLE_ED_PRIVATE_KEY` |
| `TAP_GITHUB_TOKEN` | Token with write access to `bshk-app/homebrew-tap` |

The default identity is
`Developer ID Application: Aleksandr Beshkenadze (Q8H6GWJ658)`.
The runner imports the certificate and Apple intermediates into an ephemeral
keychain and removes the keychain and notary key after the job. Missing credentials
stop a publishing run; it never falls back to ad-hoc signing.
The public Sparkle key is the same as Murmator's current key. It is committed in
`Distribution/release.json` and embedded in `SUPublicEDKey`. CI verifies the
private/public key pair and stable tap access before notarization.

GitHub cannot export encrypted secrets from another repository. Use the original
credential store or grant access to shared organization secrets.
The workflow uses the built-in `GITHUB_TOKEN`; no personal access token is needed.

## Automatic releases

Use English Conventional Commit subjects, such as `feat: add network history`
or `fix: preserve the selected process`. The
[Release Please workflow](../.github/workflows/release-please.yml) maintains a PR
with the next version and changelog. Merging it creates `vX.Y.Z` and a draft
GitHub Release, then directly invokes the
[Release workflow](../.github/workflows/release.yml) with the tag and inherited
secrets. This explicit call is necessary because tags created with `GITHUB_TOKEN`
do not start another tag-push workflow.

Release-please updates `VERSION`, `Cargo.toml`, both workspace package versions in
`Cargo.lock`, and `CHANGELOG.md` together. The two crates retain
`version.workspace = true`. The targeted TOML updater selects lockfile package
names through its tagged `name.value` fields; this configuration was checked with
release-please 17.6.0, used by the pinned action.

`VERSION` and the Cargo package version must agree. The workflow checks that the
selected tag resolves to the built commit. `CFBundleShortVersionString` contains
the numeric version without a prerelease suffix; `CFBundleVersion` counts commits.
CI fetches full history so the build number is consistent.

## Manual releases and recovery

For the first release or a beta, update `VERSION` and the root Cargo workspace
version together, run `cargo check --workspace` to update `Cargo.lock`, and add
notes under `## [<version>]` in `CHANGELOG.md`. Commit and push the source, then
push the matching tag:

```sh
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

A tag push signs, notarizes, and publishes the release. A suffix such as
`v0.2.0-beta.1` selects a prerelease and does not replace the latest stable release.

To recover a failed build, run **Actions → Release → Run workflow**, enter the
existing tag, and enable **publish**. The workflow checks out that exact tag.
It can replace assets on an unfinished draft. It refuses to modify an already
published release, including immutable releases; changes need a new version.

If a release was published but distribution failed afterward, use
**Actions → Repair Distribution → Run workflow** with that tag. It downloads the
existing release assets and repairs feeds and the stable cask without rebuilding,
re-signing, re-notarizing, or modifying the published release. Repairs for an
older stable version do not downgrade a newer Homebrew cask.

The release assets are:

- `Top-<version>-macos-arm64.dmg`, signed, notarized, and stapled.
- `Top-<version>-macos-arm64.zip`, containing the signed and stapled app.
- `manifest.json`, recording notarization, bundle metadata, the DMG digest, release version, and source commit.
- `SHA256SUMS`, covering both archives and the manifest.

The publisher verifies checksums, notarization flags, the packaged DMG digest,
and the remote tag's commit before uploading. All assets reach a draft before it
becomes public. Release notes come from the matching changelog section.

## Local builds and previews

Install the pinned Rust toolchain, Xcode with the Metal compiler, and `jq`.
Run on an Apple Silicon Mac:

```sh
bash scripts/package-macos.sh --preview
```

A manual workflow run with **publish** disabled produces the same preview.
Previews have an ad-hoc signature and do not contact Apple or publish a release.
Their manifest records `notarized: false`, and the publisher refuses them.

For a signed build, also install `zamokctl` and use a configured notary profile:

```sh
brew install bshk-app/tap/zamokctl
RELEASE_NOTARY_PROFILE=Murmur bash scripts/package-macos.sh --release
```

This command submits the app and DMG to Apple's notary service and verifies their
tickets and Gatekeeper acceptance. It produces local files; GitHub publication is
a separate script. API-key authentication is also supported through
`NOTARY_KEY_PATH`, `NOTARY_KEY_ID`, and `NOTARY_ISSUER`.

Applications live at `target/bundle/<version>/<preview|release>/Top.app`;
artifacts live at `target/dist/<version>/<preview|release>/`. Workflow artifacts
are retained for 14 days. These directories are ignored by Git.

```sh
shasum -a 256 --check SHA256SUMS
```

## Reference projects

- [Murmator workflow](https://github.com/bshk-app/murmur/blob/main/.github/workflows/release.yml):
  shared `zamokctl package` pipeline, ephemeral signing credentials, draft-first publication, and release-please.
- Tishina's `apps/tish-mac/Taskfile.yml` and `.gitea/workflows/release-tish-mac.yml`:
  Developer ID signing, notarization, stapling, and distribution checks.

## Sparkle channels and Homebrew

Packaged apps include the checksum-pinned Sparkle 2.9.6 framework and its license.
The Top menu provides **Check for Updates…** and **Update Channel → Stable/Beta**.
The channel is saved in `~/Library/Application Support/Top/update-channel`.
Stable is the default; Beta accepts stable and beta items. Switching back to
Stable selects future stable updates and does not downgrade an installed beta.
Automatic checks follow Sparkle's permission prompt and saved preference.

The canonical feed is
`https://raw.githubusercontent.com/bshk-app/top/updates/appcast.xml`.
Prereleases carry `<sparkle:channel>beta</sparkle:channel>`; stable entries use
the default channel. Both archives and the complete feed are EdDSA signed.
The app requires signed feeds and verifies archives before extraction.
History keeps ten recent items and always preserves the newest stable item.
Feed commits go to `updates`, which does not trigger release-please on `main`.

Stable releases also publish `appcast-stable.xml` and update
`bshk-app/homebrew-tap/Casks/top.rb`. The cask uses the notarized DMG, pins its
SHA-256, requires Apple Silicon and macOS 14, declares `auto_updates true`, and
uses the stable feed for livecheck. Beta releases leave the cask and stable feed
unchanged. After the first stable release is published:

```sh
brew install --cask bshk-app/tap/top
```

The distribution job checks published GitHub asset digests before generating
the feed. It verifies existing and new feed signatures, publishes both feed
files atomically, and checks the public feed and enclosure URL. A failure makes
the workflow fail; the Repair Distribution workflow retries from shipped files.
Distribution jobs are serialized so two tag releases cannot overwrite history.
