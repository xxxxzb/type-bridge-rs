#!/usr/bin/env bash
# 一键构建 macOS .app bundle
#
# 用法：cd type-bridge-rs && ./scripts/build-mac.sh
#       INSTALL=0 ./scripts/build-mac.sh  # 只构建，不装到 /Applications

set -euo pipefail

cd "$(dirname "$0")/.."

APP_NAME="TypeBridge"
BINARY_NAME="type-bridge-rs"
BUNDLE_ID="com.typebridge.rs"
MIN_MACOS="12.0"

MARKETING_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
CURRENT_PROJECT_VERSION="${MARKETING_VERSION}"
if [ -z "$MARKETING_VERSION" ]; then
  echo "✗ 无法从 Cargo.toml 读取版本"
  exit 1
fi

BUILD_DIR="target/release"
APP_BUNDLE="${BUILD_DIR}/bundle/${APP_NAME}.app"
INSTALL="${INSTALL:-1}"
COPYRIGHT="Copyright © 2026 TypeBridge. MIT License."
ICON_SVG="${ICON_SVG:-}"  # user can override, otherwise auto-detect

echo "▶ 构建版本 ${MARKETING_VERSION} (bundle ${BUNDLE_ID})"

# ── 1. cargo build --release ──────────────────────────────────────
echo "▶ cargo build --release"
cargo build --release

# ── 2. 清理旧 bundle ─────────────────────────────────────────────
rm -rf "$APP_BUNDLE"

# ── 3. 目录结构 ──────────────────────────────────────────────────
mkdir -p "${APP_BUNDLE}/Contents/MacOS"
mkdir -p "${APP_BUNDLE}/Contents/Resources"

# ── 4. 复制二进制 ────────────────────────────────────────────────
cp "${BUILD_DIR}/${BINARY_NAME}" "${APP_BUNDLE}/Contents/MacOS/"

# ── 5. 生成 .icns 图标 ───────────────────────────────────────────
ICONSET="$(mktemp -d)"

# Copy app icon (macOS accepts PNG since 10.5, no iconutil needed)
ICON_SRC="${ICON_SRC:-assets/icons/icon_512x512.png}"
if [ -f "$ICON_SRC" ]; then
  cp "$ICON_SRC" "${APP_BUNDLE}/Contents/Resources/${APP_NAME}.png"
  echo "✓ 已复制 app 图标"
else
  echo "⚠ 未找到 $ICON_SRC"
fi
rm -rf "$ICONSET"

# ── 6. 写入 Info.plist ───────────────────────────────────────────
sed \
  -e "s/\$(MARKETING_VERSION)/${MARKETING_VERSION}/g" \
  -e "s/\$(CURRENT_PROJECT_VERSION)/${CURRENT_PROJECT_VERSION}/g" \
  -e "s/\$(BUNDLE_ID)/${BUNDLE_ID}/g" \
  -e "s|\$(COPYRIGHT)|${COPYRIGHT}|g" \
  scripts/Info.plist > "${APP_BUNDLE}/Contents/Info.plist"

# ── 7. ad-hoc 签名 ───────────────────────────────────────────────
echo "▶ 签名 (ad-hoc)"
codesign --force --deep --sign - "${APP_BUNDLE}" 2>/dev/null || true

# ── 8. 清理 quarantine ───────────────────────────────────────────
xattr -cr "${APP_BUNDLE}" 2>/dev/null || true

# ── 9. 装到 /Applications ────────────────────────────────────────
if [ "$INSTALL" = "1" ]; then
  echo "▶ 装到 /Applications"
  pkill -f "${BINARY_NAME}" 2>/dev/null || true
  sleep 1
  rm -rf "/Applications/${APP_NAME}.app"
  cp -R "${APP_BUNDLE}" "/Applications/"
  xattr -dr com.apple.quarantine "/Applications/${APP_NAME}.app" 2>/dev/null || true
  echo "✓ 装好了: /Applications/${APP_NAME}.app"
  echo "  打开方式: open /Applications/${APP_NAME}.app"
else
  echo "✓ 产物: ${APP_BUNDLE}"
  echo "  打开方式: open ${APP_BUNDLE}"
fi

echo "✓ 完成"
