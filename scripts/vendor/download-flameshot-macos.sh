#!/usr/bin/env bash
set -euo pipefail

ARCH="${1:?usage: download-flameshot-macos.sh <arm64|x64> [destination]}"
DESTINATION="${2:-dist/vendor/flameshot/$ARCH}"
VERSION="${FLAMESHOT_VERSION:-14.0.0}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

case "$ARCH" in
  arm64)
    ASSET="Flameshot-14.0-macos-arm64.dmg"
    ;;
  x64)
    ASSET="Flameshot-14.0-macos-intel.dmg"
    ;;
  *)
    echo "unsupported macOS Flameshot arch: $ARCH" >&2
    exit 1
    ;;
esac

URL="https://github.com/flameshot-org/flameshot/releases/download/v$VERSION/$ASSET"
DESTINATION_ABS="$DESTINATION"
if [[ "$DESTINATION_ABS" != /* ]]; then
  DESTINATION_ABS="$ROOT/$DESTINATION_ABS"
fi

WORK_DIR="$(mktemp -d)"
MOUNT_DIR="$WORK_DIR/mount"
DMG="$WORK_DIR/$ASSET"
mkdir -p "$MOUNT_DIR"

cleanup() {
  hdiutil detach "$MOUNT_DIR" -quiet >/dev/null 2>&1 || true
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

curl --fail --location --retry 3 --output "$DMG" "$URL"
hdiutil attach "$DMG" -mountpoint "$MOUNT_DIR" -nobrowse -readonly >/dev/null

APP_SOURCE="$(find "$MOUNT_DIR" -maxdepth 2 -type d -name "*.app" | head -n 1)"
if [[ -z "$APP_SOURCE" ]]; then
  echo "Flameshot DMG did not contain a .app bundle" >&2
  exit 1
fi

rm -rf "$DESTINATION_ABS"
mkdir -p "$DESTINATION_ABS"
cp -R "$APP_SOURCE" "$DESTINATION_ABS/flameshot.app"
chmod -R u+w "$DESTINATION_ABS/flameshot.app"
xattr -cr "$DESTINATION_ABS/flameshot.app" >/dev/null 2>&1 || true

if [[ ! -x "$DESTINATION_ABS/flameshot.app/Contents/MacOS/flameshot" ]]; then
  echo "Failed to stage bundled Flameshot executable" >&2
  exit 1
fi

echo "Bundled Flameshot staged at $DESTINATION_ABS"
