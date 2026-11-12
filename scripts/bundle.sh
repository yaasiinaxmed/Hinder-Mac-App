#!/usr/bin/env bash
# Builds Hinder.app (universal: Apple silicon + Intel) and packages it as a
# .zip and a .dmg in ./dist.
#
#   ./scripts/bundle.sh            # universal build
#   ./scripts/bundle.sh --native   # only this Mac's architecture (faster)
#
# The app is ad-hoc signed, not notarized. See README → "Opening the app".
set -euo pipefail

cd "$(dirname "$0")/.."

APP_NAME="Hinder"
BUNDLE_ID="app.hinder"
VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/version = "(.*)"/\1/')"
DIST="dist"
APP="$DIST/$APP_NAME.app"

echo "==> Building $APP_NAME $VERSION"

if [[ "${1:-}" == "--native" ]]; then
  cargo build --release
  BIN="target/release/hinder"
  ARCH="$(uname -m)"
else
  rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
  cargo build --release --target aarch64-apple-darwin
  cargo build --release --target x86_64-apple-darwin
  mkdir -p target/universal
  lipo -create \
    target/aarch64-apple-darwin/release/hinder \
    target/x86_64-apple-darwin/release/hinder \
    -output target/universal/hinder
  BIN="target/universal/hinder"
  ARCH="universal"
fi

echo "==> Assembling $APP"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/$APP_NAME"

echo "==> Making the icon"
ICONSET="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$ICONSET"
"$BIN" --export-icon "$ICONSET/icon_512x512@2x.png" 1024
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$ICONSET/icon_512x512@2x.png" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  if [[ "$size" != 512 ]]; then
    sips -z "$double" "$double" "$ICONSET/icon_512x512@2x.png" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
  fi
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>               <string>$APP_NAME</string>
  <key>CFBundleDisplayName</key>        <string>$APP_NAME</string>
  <key>CFBundleExecutable</key>         <string>$APP_NAME</string>
  <key>CFBundleIdentifier</key>         <string>$BUNDLE_ID</string>
  <key>CFBundleIconFile</key>           <string>AppIcon</string>
  <key>CFBundlePackageType</key>        <string>APPL</string>
  <key>CFBundleShortVersionString</key> <string>$VERSION</string>
  <key>CFBundleVersion</key>            <string>$VERSION</string>
  <key>LSMinimumSystemVersion</key>     <string>11.0</string>
  <key>LSApplicationCategoryType</key>  <string>public.app-category.utilities</string>
  <key>NSHighResolutionCapable</key>    <true/>
  <key>NSDesktopFolderUsageDescription</key>   <string>Hinder shows the files on your Desktop.</string>
  <key>NSDocumentsFolderUsageDescription</key> <string>Hinder shows the files in your Documents folder.</string>
  <key>NSDownloadsFolderUsageDescription</key> <string>Hinder shows the files in your Downloads folder.</string>
</dict>
</plist>
PLIST

echo "==> Signing (ad-hoc)"
codesign --force --deep --sign - "$APP"

echo "==> Packaging"
ZIP="$DIST/$APP_NAME-$VERSION-macos-$ARCH.zip"
DMG="$DIST/$APP_NAME-$VERSION-macos-$ARCH.dmg"
rm -f "$ZIP" "$DMG"
ditto -c -k --keepParent "$APP" "$ZIP"

STAGE="$(mktemp -d)"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null

echo "==> Done"
ls -lh "$DIST"
