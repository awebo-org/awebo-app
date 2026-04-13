#!/usr/bin/env bash
set -euo pipefail

APP_NAME="Awebo"
ICON_SRC="assets/awebo.png"
ICON_DST="assets/awebo.icns"

# Regenerate .icns if PNG is newer
if [[ ! -f "$ICON_DST" ]] || [[ "$ICON_SRC" -nt "$ICON_DST" ]]; then
    echo "🎨 Generating .icns from $ICON_SRC..."
    ./scripts/make_icns.sh "$ICON_SRC" assets
fi

# Build release + bundle
echo "🔨 Building release..."
cargo bundle --release

APP="./target/release/bundle/osx/${APP_NAME}.app"

echo ""
echo "✅ Bundle ready: $APP"
echo ""

# Optional: install to /Applications
if [[ "${1:-}" == "--install" ]]; then
    echo "📦 Installing to /Applications..."
    cp -R "$APP" /Applications/
    echo "✅ Installed to /Applications/${APP_NAME}.app"
elif [[ "${1:-}" == "--open" ]]; then
    echo "🚀 Launching..."
    open "$APP"
fi
