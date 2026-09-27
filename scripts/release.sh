#!/usr/bin/env bash
# Builds a signed Kuberniva release for Apple Silicon and, with --publish, uploads it
# to GitHub so installed copies find it through "Check for updates".
#
#   scripts/release.sh                     # build into release/v<version>/
#   scripts/release.sh --publish notes.md  # build, then publish a GitHub release
#
# The version comes from src-tauri/tauri.conf.json; bump it there (and in
# package.json and src-tauri/Cargo.toml) before releasing.
#
# Apple signing: when ~/.config/kuberniva/release.env defines APPLE_SIGNING_IDENTITY and
# the App Store Connect API key (APPLE_API_KEY, APPLE_API_ISSUER, APPLE_API_KEY_PATH,
# NOTARY_PROFILE), the app and DMG are signed with that Developer ID, notarized by Apple,
# and stapled. Without it, builds are ad-hoc signed and need a first-launch override.
set -euo pipefail

REPO="velqa/Kuberniva"
KEY_PATH="${KUBERNIVA_UPDATER_KEY:-$HOME/.tauri/kuberniva-updater.key}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

publish=false
notes_file=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --publish) publish=true ;;
    *) notes_file="$1" ;;
  esac
  shift
done

APPLE_ENV="${KUBERNIVA_RELEASE_ENV:-$HOME/.config/kuberniva/release.env}"
if [[ -f "$APPLE_ENV" ]]; then
  # shellcheck source=/dev/null
  source "$APPLE_ENV"
fi
apple_signed=false
if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  for var in APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH NOTARY_PROFILE; do
    if [[ -z "${!var:-}" ]]; then
      echo "$APPLE_ENV sets APPLE_SIGNING_IDENTITY but not $var" >&2
      exit 1
    fi
  done
  if [[ ! -f "$APPLE_API_KEY_PATH" ]]; then
    echo "App Store Connect API key not found at $APPLE_API_KEY_PATH" >&2
    exit 1
  fi
  # Tauri reads these to sign, notarize, and staple the app before packaging it.
  export APPLE_SIGNING_IDENTITY APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH
  apple_signed=true
fi

if [[ ! -f "$KEY_PATH" ]]; then
  echo "Updater signing key not found at $KEY_PATH" >&2
  exit 1
fi

version="$(node -p "require('./src-tauri/tauri.conf.json').version")"
tag="v$version"
out="release/$tag"
bundle="src-tauri/target/release/bundle"

if $apple_signed; then
  echo "Building Kuberniva $version, signed and notarized as: $APPLE_SIGNING_IDENTITY"
else
  echo "Building Kuberniva $version with ad-hoc signing (no Apple Developer ID settings found)"
fi
# Pass the key's path, not its contents, so the key never appears in the process list.
TAURI_SIGNING_PRIVATE_KEY_PATH="$KEY_PATH" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" \
  npx tauri build --bundles app,dmg

rm -rf "$out"
mkdir -p "$out"
dmg="Kuberniva_${version}_aarch64.dmg"
archive="Kuberniva_${version}_aarch64.app.tar.gz"
cp "$bundle/dmg/Kuberniva_${version}_aarch64.dmg" "$out/$dmg"
cp "$bundle/macos/Kuberniva.app.tar.gz" "$out/$archive"
cp "$bundle/macos/Kuberniva.app.tar.gz.sig" "$out/$archive.sig"

if $apple_signed; then
  # The app inside is already notarized and stapled; the DMG gets the same treatment so
  # a fresh download opens without any Gatekeeper warning, even offline.
  codesign --force --timestamp --sign "$APPLE_SIGNING_IDENTITY" "$out/$dmg"
  echo "Notarizing $dmg (this usually takes a few minutes)…"
  xcrun notarytool submit "$out/$dmg" --keychain-profile "$NOTARY_PROFILE" --wait
  xcrun stapler staple "$out/$dmg"
  spctl --assess --type execute --verbose=2 "$bundle/macos/Kuberniva.app"
  spctl --assess --type open --context context:primary-signature --verbose=2 "$out/$dmg"
fi

notes="Kuberniva $version"
if [[ -n "$notes_file" ]]; then notes="$(cat "$notes_file")"; fi

# The update manifest installed apps read from releases/latest/download/latest.json.
NOTES="$notes" VERSION="$version" TAG="$tag" ARCHIVE="$archive" REPO="$REPO" \
SIGNATURE="$(cat "$out/$archive.sig")" node -e '
  const { NOTES, VERSION, TAG, ARCHIVE, REPO, SIGNATURE } = process.env;
  const manifest = {
    version: VERSION,
    notes: NOTES,
    pub_date: new Date().toISOString(),
    platforms: {
      "darwin-aarch64": {
        signature: SIGNATURE,
        url: `https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE}`,
      },
    },
  };
  require("fs").writeFileSync(process.argv[1], JSON.stringify(manifest, null, 2) + "\n");
' "$out/latest.json"

echo "Built $out:"
ls -lh "$out"

if $publish; then
  notes_args=(--notes "Kuberniva $version")
  if [[ -n "$notes_file" ]]; then notes_args=(--notes-file "$notes_file"); fi
  gh release create "$tag" "$out/$dmg" "$out/$archive" "$out/latest.json" \
    --repo "$REPO" \
    --target "$(git rev-parse HEAD)" \
    --title "Kuberniva $version" \
    "${notes_args[@]}" \
    --latest
fi
