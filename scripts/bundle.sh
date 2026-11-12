#!/usr/bin/env bash
# Builds Hinder.app (universal: Apple silicon + Intel) and packages it as a
# .zip and a .dmg in ./dist.
#
#   ./scripts/bundle.sh            # universal build
#   ./scripts/bundle.sh --native   # only this Mac's architecture (faster)
#
# Signing and notarization (all optional; without them the app is ad-hoc signed
# and macOS shows an "unverified developer" warning on first launch):
#   SIGN_IDENTITY   "Developer ID Application: …" (default: the first one in the
#                   keychain, if any). "-" forces ad-hoc signing.
#   Notarization credentials, one of:
#     NOTARY_PROFILE                              a `notarytool store-credentials` profile
#     APPLE_ID + APPLE_APP_PASSWORD + APPLE_TEAM_ID   Apple ID with an app-specific password
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

if [[ -z "${SIGN_IDENTITY:-}" ]]; then
  SIGN_IDENTITY="$(security find-identity -v -p codesigning 2>/dev/null | awk -F'"' '/Developer ID Application/{print $2; exit}')"
  SIGN_IDENTITY="${SIGN_IDENTITY:--}"
fi
if [[ "$SIGN_IDENTITY" == "-" ]]; then
  echo "==> Signing (ad-hoc: users will see an 'unverified developer' warning)"
  codesign --force --deep --sign - "$APP"
else
  echo "==> Signing as $SIGN_IDENTITY"
  # Hardened runtime + secure timestamp: both required for notarization.
  codesign --force --options runtime --timestamp --sign "$SIGN_IDENTITY" "$APP"
  codesign --verify --strict --verbose=1 "$APP"
fi

NOTARY=()
if [[ "$SIGN_IDENTITY" != "-" ]]; then
  if [[ -n "${NOTARY_PROFILE:-}" ]]; then
    NOTARY=(--keychain-profile "$NOTARY_PROFILE")
  elif [[ -n "${APPLE_ID:-}" && -n "${APPLE_APP_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" ]]; then
    NOTARY=(--apple-id "$APPLE_ID" --password "$APPLE_APP_PASSWORD" --team-id "$APPLE_TEAM_ID")
  fi
fi

# Sends a file to Apple's notary service and waits; fails loudly with Apple's log.
notarize() {
  local file="$1" out id status
  echo "    notarizing $(basename "$file") (usually 1–5 minutes)…"
  out="$(xcrun notarytool submit "$file" "${NOTARY[@]}" --wait --timeout 30m --output-format json)" || true
  id="$(/usr/bin/python3 -c 'import json,sys; print(json.loads(sys.stdin.read() or "{}").get("id",""))' <<<"$out")"
  status="$(/usr/bin/python3 -c 'import json,sys; print(json.loads(sys.stdin.read() or "{}").get("status",""))' <<<"$out")"
  if [[ "$status" != "Accepted" ]]; then
    echo "    notarization failed (status: ${status:-none})"
    echo "$out"
    [[ -n "$id" ]] && xcrun notarytool log "$id" "${NOTARY[@]}" || true
    exit 1
  fi
}

if [[ ${#NOTARY[@]} -gt 0 ]]; then
  echo "==> Notarizing the app"
  NZIP="$(mktemp -d)/$APP_NAME.zip"
  ditto -c -k --keepParent "$APP" "$NZIP"
  notarize "$NZIP"
  xcrun stapler staple "$APP" >/dev/null
  echo "    stapled the ticket to $APP_NAME.app"
elif [[ "$SIGN_IDENTITY" != "-" ]]; then
  echo "    (no notarization credentials: signed, but not notarized)"
fi

echo "==> Packaging"
ZIP="$DIST/$APP_NAME-$VERSION-macos-$ARCH.zip"
DMG="$DIST/$APP_NAME-$VERSION-macos-$ARCH.dmg"
rm -f "$ZIP" "$DMG"
ditto -c -k --keepParent "$APP" "$ZIP"

# Installer window: brand background, big icons, drag-to-Applications arrow.
# dmgbuild writes the layout directly (no Finder scripting). It's installed
# into a private virtualenv under target/ the first time.
VENV="target/dmgbuild-venv"
PYTHON="${PYTHON:-python3}"
if [[ ! -x "$VENV/bin/dmgbuild" ]]; then
  echo "    installing dmgbuild into $VENV (using $($PYTHON --version 2>&1))"
  rm -rf "$VENV"
  { "$PYTHON" -m venv "$VENV" && "$VENV/bin/pip" install --quiet "dmgbuild==1.6.5"; } || rm -rf "$VENV"
fi
if [[ -x "$VENV/bin/dmgbuild" ]]; then
  BG="$(mktemp -d)/background.tiff"
  tiffutil -cathidpicheck assets/dmg/background.png assets/dmg/background@2x.png -out "$BG" >/dev/null
  "$VENV/bin/dmgbuild" \
    -s scripts/dmg-settings.py \
    -D app="$APP" \
    -D background="$BG" \
    -D volicon="$APP/Contents/Resources/AppIcon.icns" \
    "$APP_NAME" "$DMG" >/dev/null
else
  # Never ship a release without a .dmg: fall back to a plain one.
  echo "    dmgbuild unavailable: making a plain .dmg instead"
  STAGE="$(mktemp -d)"
  cp -R "$APP" "$STAGE/"
  ln -s /Applications "$STAGE/Applications"
  hdiutil create -volname "$APP_NAME" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null
fi

if [[ "$SIGN_IDENTITY" != "-" ]]; then
  codesign --force --timestamp --sign "$SIGN_IDENTITY" "$DMG"
  if [[ ${#NOTARY[@]} -gt 0 ]]; then
    echo "==> Notarizing the .dmg"
    notarize "$DMG"
    xcrun stapler staple "$DMG" >/dev/null
  fi
fi

echo "==> Done"
ls -lh "$DIST"
