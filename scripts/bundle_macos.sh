#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

HOST_ARCH="$(uname -m)"
if [[ "${HOST_ARCH}" == "arm64" ]]; then
  DEFAULT_TARGET="aarch64-apple-darwin"
else
  DEFAULT_TARGET="x86_64-apple-darwin"
fi

TARGET="${1:-${DEFAULT_TARGET}}"
VERSION="${2:-$(sed -n 's/^version = "\(.*\)"/\1/p' crates/pivot-desktop/Cargo.toml | head -n 1)}"
VERSION="${VERSION#v}"

case "${TARGET}" in
  aarch64-apple-darwin)
    ARCH_LABEL="aarch64"
    ;;
  x86_64-apple-darwin)
    ARCH_LABEL="x64"
    ;;
  *)
    echo "Unsupported macOS target: ${TARGET}" >&2
    exit 1
    ;;
esac

BIN_PATH="${ROOT_DIR}/target/${TARGET}/release/pivot"
if [[ ! -f "${BIN_PATH}" ]]; then
  FALLBACK_BIN="${ROOT_DIR}/target/release/pivot"
  if [[ -f "${FALLBACK_BIN}" ]]; then
    BIN_PATH="${FALLBACK_BIN}"
  else
    echo "Release binary not found at ${BIN_PATH}. Run: cargo build --release --target ${TARGET} -p pivot-desktop" >&2
    exit 1
  fi
fi

ICON_ICNS="${ROOT_DIR}/crates/pivot-desktop/assets/icon.icns"
if [[ ! -f "${ICON_ICNS}" ]]; then
  echo "Missing app icon at ${ICON_ICNS}" >&2
  exit 1
fi

BUNDLE_ROOT="${ROOT_DIR}/target/bundle/${TARGET}"
APP_DIR="${BUNDLE_ROOT}/Pivot.app"
DMG_STAGE="${BUNDLE_ROOT}/dmg-stage"
OUT_DIR="${ROOT_DIR}/target/bundle"
OUT_DMG="${OUT_DIR}/Pivot_${VERSION}_${ARCH_LABEL}.dmg"

rm -rf "${BUNDLE_ROOT}" "${OUT_DMG}"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources" "${OUT_DIR}"

cp "${BIN_PATH}" "${APP_DIR}/Contents/MacOS/pivot"
chmod +x "${APP_DIR}/Contents/MacOS/pivot"
cp "${ICON_ICNS}" "${APP_DIR}/Contents/Resources/icon.icns"

cat > "${APP_DIR}/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>Pivot</string>
  <key>CFBundleDisplayName</key>
  <string>Pivot</string>
  <key>CFBundleIdentifier</key>
  <string>com.pivot.desktop</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleExecutable</key>
  <string>pivot</string>
  <key>CFBundleIconFile</key>
  <string>icon.icns</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key>
  <true/>
</dict>
</plist>
EOF

echo "==> Ad-hoc signing Pivot.app (${TARGET})"
codesign --force --deep --sign - "${APP_DIR}"

echo "==> Creating DMG installer: ${OUT_DMG}"
mkdir -p "${DMG_STAGE}"
cp -R "${APP_DIR}" "${DMG_STAGE}/Pivot.app"
ln -s /Applications "${DMG_STAGE}/Applications"

hdiutil create \
  -volname "Pivot" \
  -srcfolder "${DMG_STAGE}" \
  -ov \
  -format UDZO \
  "${OUT_DMG}"

rm -rf "${DMG_STAGE}"
echo "==> Packaged successfully: ${OUT_DMG}"
