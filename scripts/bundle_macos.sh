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

EXPLICIT_TARGET="${1:-}"
TARGET="${EXPLICIT_TARGET:-${DEFAULT_TARGET}}"
VERSION="${2:-$(sed -n 's/^version = "\(.*\)"/\1/p' crates/pivot-desktop/Cargo.toml | head -n 1)}"
VERSION="${VERSION#v}"

case "${TARGET}" in
  universal|universal-apple-darwin)
    TARGET="universal-apple-darwin"
    ARM64_BIN="${ROOT_DIR}/target/aarch64-apple-darwin/release/pivot"
    X64_BIN="${ROOT_DIR}/target/x86_64-apple-darwin/release/pivot"
    if [[ ! -f "${ARM64_BIN}" ]]; then
      echo "Missing arm64 release binary at ${ARM64_BIN}. Run: cargo build --release --target aarch64-apple-darwin -p pivot-desktop" >&2
      exit 1
    fi
    if [[ ! -f "${X64_BIN}" ]]; then
      echo "Missing x86_64 release binary at ${X64_BIN}. Run: cargo build --release --target x86_64-apple-darwin -p pivot-desktop" >&2
      exit 1
    fi
    ;;
  aarch64-apple-darwin|x86_64-apple-darwin)
    BIN_PATH="${ROOT_DIR}/target/${TARGET}/release/pivot"
    if [[ ! -f "${BIN_PATH}" ]]; then
      if [[ -z "${EXPLICIT_TARGET}" && -f "${ROOT_DIR}/target/release/pivot" ]]; then
        BIN_PATH="${ROOT_DIR}/target/release/pivot"
      else
        echo "Release binary not found at ${BIN_PATH}. Run: cargo build --release --target ${TARGET} -p pivot-desktop" >&2
        exit 1
      fi
    fi
    ;;
  *)
    echo "Unsupported macOS target: ${TARGET}" >&2
    exit 1
    ;;
esac

ICON_ICNS="${ROOT_DIR}/crates/pivot-desktop/assets/icon.icns"
if [[ ! -f "${ICON_ICNS}" ]]; then
  echo "Missing app icon at ${ICON_ICNS}" >&2
  exit 1
fi

BUNDLE_ROOT="${ROOT_DIR}/target/bundle/${TARGET}"
APP_DIR="${BUNDLE_ROOT}/Pivot.app"
DMG_STAGE="${BUNDLE_ROOT}/dmg-stage"
OUT_DIR="${ROOT_DIR}/target/bundle"
OUT_DMG="${OUT_DIR}/Pivot.dmg"

rm -rf "${BUNDLE_ROOT}" "${OUT_DMG}"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources" "${OUT_DIR}"

if [[ "${TARGET}" == "universal-apple-darwin" ]]; then
  echo "==> Creating universal binary via lipo"
  lipo -create -output "${APP_DIR}/Contents/MacOS/pivot" "${ARM64_BIN}" "${X64_BIN}"
  ARCHS="$(lipo -archs "${APP_DIR}/Contents/MacOS/pivot")"
  if [[ ! "${ARCHS}" =~ arm64 ]] || [[ ! "${ARCHS}" =~ x86_64 ]]; then
    echo "Universal binary verification failed, detected archs: ${ARCHS}" >&2
    exit 1
  fi
  echo "==> Verified universal binary archs: ${ARCHS}"
else
  cp "${BIN_PATH}" "${APP_DIR}/Contents/MacOS/pivot"
fi
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
  -format ULMO \
  "${OUT_DMG}"

rm -rf "${DMG_STAGE}"
echo "==> Packaged successfully: ${OUT_DMG}"
