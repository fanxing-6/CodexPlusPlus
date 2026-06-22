#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
vendor_root="${CODEX_VENDOR_DIR:-"$repo_root/dist/vendor"}"
vendor_dir="${CODEX_FLAMESHOT_VENDOR_DIR:-"$vendor_root/flameshot-src"}"
tag="${CODEX_FLAMESHOT_TAG:-v14.0.0}"
expected_commit="${CODEX_FLAMESHOT_COMMIT:-e408812d77ff1835957f85796c4cf737466bd69d}"
qt_color_widgets_dir="${CODEX_QT_COLOR_WIDGETS_SOURCE_DIR:-"$vendor_root/qt-color-widgets-src"}"
qt_color_widgets_commit="${CODEX_QT_COLOR_WIDGETS_COMMIT:-5d52e907e50dc88cf969b41cea44665ff6c475b1}"
qhotkey_dir="${CODEX_QHOTKEY_SOURCE_DIR:-"$vendor_root/qhotkey-src"}"
qhotkey_commit="${CODEX_QHOTKEY_COMMIT:-d7063877c14d5ae2b489dc70bbe02e76a43bf38b}"
kdsingleapplication_dir="${CODEX_KDSINGLEAPPLICATION_SOURCE_DIR:-"$vendor_root/kdsingleapplication-src"}"
kdsingleapplication_commit="${CODEX_KDSINGLEAPPLICATION_COMMIT:-3186a158f8e6565e89f5983b4028c892737844ff}"

mkdir -p "$(dirname "$vendor_dir")"
if [[ ! -d "$vendor_dir/.git" ]]; then
  rm -rf "$vendor_dir"
  git clone --depth 1 --branch "$tag" https://github.com/flameshot-org/flameshot.git "$vendor_dir"
else
  git -C "$vendor_dir" fetch --depth 1 origin "refs/tags/$tag:refs/tags/$tag"
  git -C "$vendor_dir" checkout --detach "$tag"
fi

actual_commit="$(git -C "$vendor_dir" rev-parse HEAD)"
if [[ "$actual_commit" != "$expected_commit" ]]; then
  echo "Unexpected Flameshot commit for $tag: $actual_commit (expected $expected_commit)" >&2
  exit 1
fi

echo "$actual_commit" > "$vendor_dir/.codex-flameshot-commit"

prepare_pinned_checkout() {
  local name="$1"
  local url="$2"
  local dir="$3"
  local expected="$4"

  mkdir -p "$(dirname "$dir")"
  if [[ ! -d "$dir/.git" ]]; then
    rm -rf "$dir"
    git clone "$url" "$dir"
  else
    git -C "$dir" fetch origin
  fi
  git -C "$dir" checkout --detach "$expected"

  local actual
  actual="$(git -C "$dir" rev-parse HEAD)"
  if [[ "$actual" != "$expected" ]]; then
    echo "Unexpected $name commit: $actual (expected $expected)" >&2
    exit 1
  fi
  echo "$actual" > "$dir/.codex-$name-commit"
  echo "Prepared $name at $dir ($actual)"
}

prepare_pinned_checkout \
  "qt-color-widgets" \
  "https://gitlab.com/mattbas/Qt-Color-Widgets.git" \
  "$qt_color_widgets_dir" \
  "$qt_color_widgets_commit"

prepare_pinned_checkout \
  "qhotkey" \
  "https://github.com/flameshot-org/QHotkey" \
  "$qhotkey_dir" \
  "$qhotkey_commit"

prepare_pinned_checkout \
  "kdsingleapplication" \
  "https://github.com/KDAB/KDSingleApplication.git" \
  "$kdsingleapplication_dir" \
  "$kdsingleapplication_commit"

echo "Prepared Flameshot $tag at $vendor_dir ($actual_commit)"
