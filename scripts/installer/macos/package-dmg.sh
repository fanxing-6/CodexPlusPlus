#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-0.0.0}"
ARCH="${2:-$(uname -m)}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
DIST="$ROOT/dist/macos"
STAGE="$DIST/stage"
BINARY_DIR="${BINARY_DIR:-$ROOT/target/release}"
DMG="$DIST/CodexPlusPlus-${VERSION}-macos-${ARCH}.dmg"
ICON_SOURCE="$ROOT/apps/codex-plus-manager/src-tauri/icons/icon.png"
ICON_NAME="codex-plus-plus.icns"
ICON_ICNS="$DIST/$ICON_NAME"

rm -rf "$DIST"
mkdir -p "$STAGE"

prepare_icon() {
  local iconset="$DIST/codex-plus-plus.iconset"
  rm -rf "$iconset"
  mkdir -p "$iconset"

  sips -z 16 16 "$ICON_SOURCE" --out "$iconset/icon_16x16.png" >/dev/null
  sips -z 32 32 "$ICON_SOURCE" --out "$iconset/icon_16x16@2x.png" >/dev/null
  sips -z 32 32 "$ICON_SOURCE" --out "$iconset/icon_32x32.png" >/dev/null
  sips -z 64 64 "$ICON_SOURCE" --out "$iconset/icon_32x32@2x.png" >/dev/null
  sips -z 128 128 "$ICON_SOURCE" --out "$iconset/icon_128x128.png" >/dev/null
  sips -z 256 256 "$ICON_SOURCE" --out "$iconset/icon_128x128@2x.png" >/dev/null
  sips -z 256 256 "$ICON_SOURCE" --out "$iconset/icon_256x256.png" >/dev/null
  sips -z 512 512 "$ICON_SOURCE" --out "$iconset/icon_256x256@2x.png" >/dev/null
  sips -z 512 512 "$ICON_SOURCE" --out "$iconset/icon_512x512.png" >/dev/null
  sips -z 1024 1024 "$ICON_SOURCE" --out "$iconset/icon_512x512@2x.png" >/dev/null

  iconutil -c icns "$iconset" -o "$ICON_ICNS"
}

copy_bundled_flameshot() {
  local app_dir="$1"
  local source="${FLAMESHOT_BUNDLE_DIR:-$ROOT/dist/vendor/flameshot/$ARCH}"
  local destination="$app_dir/Contents/Helpers"

  if [ ! -d "$source/Flameshot.app" ]; then
    echo "error: bundled Flameshot app not found: $source/Flameshot.app" >&2
    return 1
  fi
  rm -rf "$destination/Flameshot.app"
  mkdir -p "$destination"
  ditto "$source/Flameshot.app" "$destination/Flameshot.app"
  xattr -cr "$destination/Flameshot.app" >/dev/null 2>&1 || true
  if [ ! -x "$destination/Flameshot.app/Contents/MacOS/flameshot" ]; then
    echo "error: bundled Flameshot executable missing in $destination" >&2
    return 1
  fi
}

find_embedded_flameshot_lib() {
  if [ -n "${EMBEDDED_FLAMESHOT_LIB:-}" ]; then
    printf '%s\n' "$EMBEDDED_FLAMESHOT_LIB"
    return 0
  fi

  find "$ROOT/target" \
    -path "*/out/flameshot-embedded-lib/libcodex_flameshot_embedded.dylib" \
    -type f \
    -print \
    -quit 2>/dev/null || true
}

copy_embedded_flameshot_runtime() {
  local app_dir="$1"
  local lib_source
  lib_source="$(find_embedded_flameshot_lib)"
  if [ -z "$lib_source" ] || [ ! -f "$lib_source" ]; then
    echo "error: embedded Flameshot dylib not found; set EMBEDDED_FLAMESHOT_LIB" >&2
    return 1
  fi

  local frameworks_dir="$app_dir/Contents/Frameworks"
  local lib_name="libcodex_flameshot_embedded.dylib"
  mkdir -p "$frameworks_dir"
  cp "$lib_source" "$frameworks_dir/$lib_name"
  chmod +x "$frameworks_dir/$lib_name"
  install_name_tool -id "@rpath/$lib_name" "$frameworks_dir/$lib_name"
}

copy_screenshot_runtime() {
  local app_dir="$1"
  if [ -n "${EMBEDDED_FLAMESHOT_LIB:-}" ] || [ "${CODEX_PLUS_EMBEDDED_FLAMESHOT:-}" = "1" ]; then
    copy_embedded_flameshot_runtime "$app_dir"
  else
    copy_bundled_flameshot "$app_dir"
  fi
}

patch_embedded_flameshot_linkage() {
  local app_dir="$1"
  local executable="$2"
  local executable_path="$app_dir/Contents/MacOS/$executable"
  local lib_name="libcodex_flameshot_embedded.dylib"
  if [ ! -f "$app_dir/Contents/Frameworks/$lib_name" ]; then
    return 0
  fi

  local linked_name
  linked_name="$(otool -L "$executable_path" | awk '/libcodex_flameshot_embedded\.dylib/ { print $1; exit }')"
  if [ -n "$linked_name" ] && [ "$linked_name" != "@rpath/$lib_name" ]; then
    install_name_tool -change "$linked_name" "@rpath/$lib_name" "$executable_path"
  fi
  if ! otool -l "$executable_path" | grep -F "@executable_path/../Frameworks" >/dev/null; then
    install_name_tool -add_rpath "@executable_path/../Frameworks" "$executable_path"
  fi
}

deploy_qt_runtime_if_needed() {
  local app_dir="$1"
  if [ ! -f "$app_dir/Contents/Frameworks/libcodex_flameshot_embedded.dylib" ]; then
    return 0
  fi

  local macdeployqt="${MACDEPLOYQT:-}"
  if [ -z "$macdeployqt" ]; then
    macdeployqt="$(command -v macdeployqt || true)"
  fi
  if [ -z "$macdeployqt" ] && command -v brew >/dev/null 2>&1; then
    local qt_prefix
    qt_prefix="$(brew --prefix qt 2>/dev/null || true)"
    if [ -x "$qt_prefix/bin/macdeployqt" ]; then
      macdeployqt="$qt_prefix/bin/macdeployqt"
    fi
  fi
  if [ -z "$macdeployqt" ]; then
    echo "error: macdeployqt not found; install Qt or set MACDEPLOYQT" >&2
    return 1
  fi

  "$macdeployqt" "$app_dir" -always-overwrite
}

sign_embedded_runtime() {
  local app_dir="$1"
  if [ ! -d "$app_dir/Contents/Frameworks" ]; then
    return 0
  fi

  while IFS= read -r -d '' framework; do
    codesign --force --sign - "$framework"
  done < <(find "$app_dir/Contents/Frameworks" -maxdepth 2 -type d -name "*.framework" -print0)

  while IFS= read -r -d '' dylib; do
    codesign --force --sign - "$dylib"
  done < <(find "$app_dir/Contents/Frameworks" -type f -name "*.dylib" -print0)

  if [ -d "$app_dir/Contents/PlugIns" ]; then
    while IFS= read -r -d '' plugin; do
      codesign --force --sign - "$plugin"
    done < <(find "$app_dir/Contents/PlugIns" -type f \( -name "*.dylib" -o -perm -111 \) -print0)
  fi
}

create_app() {
  local app_name="$1"
  local executable_name="$2"
  local binary_path="$3"
  local bundle_id="$4"
  local lsui_element="${5:-false}"
  local include_flameshot="${6:-false}"
  local app_dir="$STAGE/$app_name.app"

  if [ ! -x "$binary_path" ]; then
    echo "error: binary not found or not executable: $binary_path" >&2
    return 1
  fi

  rm -rf "$app_dir"
  mkdir -p "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources"
  cp "$binary_path" "$app_dir/Contents/MacOS/$executable_name"
  cp "$ICON_ICNS" "$app_dir/Contents/Resources/$ICON_NAME"
  if [ "$include_flameshot" = "true" ]; then
    copy_screenshot_runtime "$app_dir"
  fi
  chmod +x "$app_dir/Contents/MacOS/$executable_name"
  printf 'APPL????' > "$app_dir/Contents/PkgInfo"
  cat > "$app_dir/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>$app_name</string>
  <key>CFBundleDisplayName</key>
  <string>$app_name</string>
  <key>CFBundleIdentifier</key>
  <string>$bundle_id</string>
  <key>CFBundleVersion</key>
  <string>$VERSION</string>
  <key>CFBundleShortVersionString</key>
  <string>$VERSION</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleSignature</key>
  <string>????</string>
  <key>CFBundleExecutable</key>
  <string>$executable_name</string>
  <key>CFBundleIconFile</key>
  <string>$ICON_NAME</string>
  <key>LSMinimumSystemVersion</key>
  <string>12.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>LSUIElement</key>
  <$lsui_element/>
</dict>
</plist>
PLIST
}

sign_app() {
  local app_dir="$1"
  local executable
  executable="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$app_dir/Contents/Info.plist")"
  if [ -d "$app_dir/Contents/Helpers/Flameshot.app" ]; then
    codesign --verify --deep --strict "$app_dir/Contents/Helpers/Flameshot.app"
  fi
  patch_embedded_flameshot_linkage "$app_dir" "$executable"
  deploy_qt_runtime_if_needed "$app_dir"
  sign_embedded_runtime "$app_dir"
  codesign --force --sign - "$app_dir/Contents/MacOS/$executable"
  codesign --force --deep --sign - "$app_dir"
}

verify_app() {
  local app_dir="$1"
  local plist="$app_dir/Contents/Info.plist"
  local plutil_bin
  plutil_bin="$(command -v plutil || true)"
  if [ -n "$plutil_bin" ]; then
    "$plutil_bin" -lint "$plist" >/dev/null
  else
    /usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$plist" >/dev/null
  fi
  if [ ! -f "$app_dir/Contents/PkgInfo" ]; then
    echo "error: missing PkgInfo in $app_dir" >&2
    return 1
  fi
  codesign --verify --deep --strict "$app_dir" >/dev/null 2>&1 || {
    echo "error: codesign verification failed for $app_dir" >&2
    return 1
  }
  if [ -d "$app_dir/Contents/Helpers/Flameshot.app" ]; then
    codesign --verify --deep --strict "$app_dir/Contents/Helpers/Flameshot.app" >/dev/null
  fi
  if [ -f "$app_dir/Contents/Frameworks/libcodex_flameshot_embedded.dylib" ]; then
    codesign --verify --strict "$app_dir/Contents/Frameworks/libcodex_flameshot_embedded.dylib" >/dev/null
    otool -L "$app_dir/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$plist")" |
      grep -F "@rpath/libcodex_flameshot_embedded.dylib" >/dev/null
  fi
}

prepare_icon
create_app "Codex++" "CodexPlusPlus" "$BINARY_DIR/codex-plus-plus" "com.bigpizzav3.codexplusplus" "true" "true"
create_app "Codex++ 管理工具" "CodexPlusPlusManager" "$BINARY_DIR/codex-plus-plus-manager" "com.bigpizzav3.codexplusplus.manager" "false"

sign_app "$STAGE/Codex++.app"
sign_app "$STAGE/Codex++ 管理工具.app"

verify_app "$STAGE/Codex++.app"
verify_app "$STAGE/Codex++ 管理工具.app"

ln -s /Applications "$STAGE/Applications"

hdiutil create -volname "Codex++" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
echo "$DMG"
