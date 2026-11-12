#!/usr/bin/env bash
# Renders assets/dmg/background.svg to the PNGs the installer uses.
# Run it after editing the SVG and commit the PNGs (CI uses them as-is).
# Uses headless Google Chrome so the text gets the real macOS system font.
set -euo pipefail
cd "$(dirname "$0")/.."

CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
[[ -x "$CHROME" ]] || { echo "Google Chrome is needed to render the background"; exit 1; }

SVG="$PWD/assets/dmg/background.svg"
TMP="$(mktemp -d)"
cat > "$TMP/page.html" <<HTML
<!doctype html><html><body style="margin:0;background:#140B08">
<img src="file://$SVG" width="660" height="420" style="display:block">
</body></html>
HTML

for scale in 1 2; do
  out="assets/dmg/background.png"
  [[ $scale == 2 ]] && out="assets/dmg/background@2x.png"
  "$CHROME" --headless=new --disable-gpu --hide-scrollbars --allow-file-access-from-files \
    --force-device-scale-factor=$scale --window-size=660,420 \
    --screenshot="$PWD/$out" "file://$TMP/page.html" >/dev/null 2>&1
  echo "wrote $out ($(sips -g pixelWidth -g pixelHeight "$out" | awk '/pixel/{printf "%s ", $2}'))"
done
