#!/usr/bin/env bash
# Builds a signed Kuberniva release for Apple Silicon and, with --publish, uploads it
# to GitHub so installed copies find it through "Check for updates".
#
#   scripts/release.sh                     # build into release/v<version>/
#   scripts/release.sh --publish notes.md  # build, then publish a GitHub release
#
# The version comes from src-tauri/tauri.conf.json; bump it there (and in
# package.json and src-tauri/Cargo.toml) before releasing.
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

if [[ ! -f "$KEY_PATH" ]]; then
  echo "Updater signing key not found at $KEY_PATH" >&2
  exit 1
fi

version="$(node -p "require('./src-tauri/tauri.conf.json').version")"
tag="v$version"
out="release/$tag"
bundle="src-tauri/target/release/bundle"

echo "Building Kuberniva $version…"
TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_PATH")" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" \
  npx tauri build --bundles app,dmg

rm -rf "$out"
mkdir -p "$out"
dmg="Kuberniva_${version}_aarch64.dmg"
archive="Kuberniva_${version}_aarch64.app.tar.gz"
cp "$bundle/dmg/Kuberniva_${version}_aarch64.dmg" "$out/$dmg"
cp "$bundle/macos/Kuberniva.app.tar.gz" "$out/$archive"
cp "$bundle/macos/Kuberniva.app.tar.gz.sig" "$out/$archive.sig"

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
