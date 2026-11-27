#!/usr/bin/env bash
# 生成可直接提交到 macOS 应用商店的 .app bundle（含 CFBundleIdentifier）
# 用法: build-app.sh <version> <binary-path> <output.app> [output.zip]
#
# 环境变量:
#   SIGN_IDENTITY  可选。传给 codesign --sign，例如 "-"（ad-hoc）或 "Developer ID Application: ..."
#                  留空则不签名（CI 默认；上架前需自行用正式证书重签）
set -euo pipefail

VERSION="$1"
BIN="$2"
OUT="$3"
ZIP="${4:-}"

NUMERIC="${VERSION#v}"
BUNDLE_ID="com.0x0021.rust-ping"
APP_NAME="rust-ping"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

rm -rf "$OUT"
mkdir -p "$OUT/Contents/MacOS" "$OUT/Contents/Resources"

# ditto 避免 macOS 生成 ._ AppleDouble 伴随文件
ditto --norsrc --noextattr --noacl "$BIN" "$OUT/Contents/MacOS/$APP_NAME"
chmod 755 "$OUT/Contents/MacOS/$APP_NAME"

for f in README.md config.example.toml; do
  ditto --norsrc --noextattr --noacl "$ROOT/$f" "$OUT/Contents/Resources/$f"
done
ditto --norsrc --noextattr --noacl "$ROOT/assets/icon.icns" "$OUT/Contents/Resources/icon.icns"
ditto --norsrc --noextattr --noacl "$ROOT/assets/icon.png" "$OUT/Contents/Resources/AppIcon.png"

cat > "$OUT/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>$APP_NAME</string>
  <key>CFBundleIconFile</key>
  <string>icon.icns</string>
  <key>CFBundleIdentifier</key>
  <string>$BUNDLE_ID</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>$APP_NAME</string>
  <key>CFBundleDisplayName</key>
  <string>$APP_NAME</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$NUMERIC</string>
  <key>CFBundleVersion</key>
  <string>$NUMERIC</string>
  <key>CFBundleSupportedPlatforms</key>
  <array>
    <string>MacOSX</string>
  </array>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>LSBackgroundOnly</key>
  <true/>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSUpdateSecurityPolicy</key>
  <dict>
    <key>AllowProcesses</key>
    <dict>
      <key>MEHY5QF425</key>
      <array>
        <string>com.setapp.DesktopClient.SetappAgent</string>
      </array>
    </dict>
  </dict>
</dict>
</plist>
PLIST

# 自检：plist 必须可解析，且 Bundle ID 非空
plutil -lint "$OUT/Contents/Info.plist" >/dev/null
ACTUAL="$(plutil -extract CFBundleIdentifier raw -o - "$OUT/Contents/Info.plist")"
if [ "$ACTUAL" != "$BUNDLE_ID" ]; then
  echo "Bundle ID 校验失败: $ACTUAL" >&2
  exit 1
fi

if [ -n "${SIGN_IDENTITY:-}" ]; then
  codesign --force --sign "$SIGN_IDENTITY" "$OUT"
  codesign --verify --strict "$OUT"
fi

if [ -n "$ZIP" ]; then
  rm -f "$ZIP"
  ZIP_ABS="$(cd "$(dirname "$ZIP")" && pwd)/$(basename "$ZIP")"
  # zip 根目录 = rust-ping.app/ + AppIcon.png，无 __MACOSX（Setapp 要求）
  TMPDIR_ZIP="$(mktemp -d)"
  trap 'rm -rf "$TMPDIR_ZIP"' EXIT
  cp -R "$OUT" "$TMPDIR_ZIP/$(basename "$OUT")"
  ditto --norsrc --noextattr --noacl "$ROOT/assets/icon.png" "$TMPDIR_ZIP/AppIcon.png"
  (cd "$TMPDIR_ZIP" && /usr/bin/zip -q -r -y "$ZIP_ABS" "$(basename "$OUT")" AppIcon.png)
  echo "已生成: $ZIP (Bundle ID: $ACTUAL)"
fi

echo "已生成: $OUT (Bundle ID: $ACTUAL)"
