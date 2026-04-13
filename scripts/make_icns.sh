#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "Usage: $0 <input-image> <output-dir>"
  exit 1
fi

INPUT="$1"
OUTDIR="$2"

mkdir -p "$OUTDIR"

BASENAME="$(basename "$INPUT")"
STEM="${BASENAME%.*}"
ICONSET_DIR="$OUTDIR/${STEM}.iconset"
ICNS_PATH="$OUTDIR/$STEM.icns"

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT
WORK_1024="$TMPDIR/icon_1024.png"
sips -s format png -z 1024 1024 "$INPUT" --out "$WORK_1024" >/dev/null

rm -rf "$ICONSET_DIR"
mkdir -p "$ICONSET_DIR"

sips -z 16 16     "$WORK_1024" --out "$ICONSET_DIR/icon_16x16.png"      >/dev/null
sips -z 32 32     "$WORK_1024" --out "$ICONSET_DIR/icon_16x16@2x.png"   >/dev/null
sips -z 32 32     "$WORK_1024" --out "$ICONSET_DIR/icon_32x32.png"      >/dev/null
sips -z 64 64     "$WORK_1024" --out "$ICONSET_DIR/icon_32x32@2x.png"   >/dev/null
sips -z 128 128   "$WORK_1024" --out "$ICONSET_DIR/icon_128x128.png"    >/dev/null
sips -z 256 256   "$WORK_1024" --out "$ICONSET_DIR/icon_128x128@2x.png" >/dev/null
sips -z 256 256   "$WORK_1024" --out "$ICONSET_DIR/icon_256x256.png"    >/dev/null
sips -z 512 512   "$WORK_1024" --out "$ICONSET_DIR/icon_256x256@2x.png" >/dev/null
sips -z 512 512   "$WORK_1024" --out "$ICONSET_DIR/icon_512x512.png"    >/dev/null
cp "$WORK_1024"         "$ICONSET_DIR/icon_512x512@2x.png"

iconutil -c icns "$ICONSET_DIR" -o "$ICNS_PATH"

echo "ICNS: $ICNS_PATH"
