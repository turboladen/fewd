#!/usr/bin/env bash
# Regenerate favicon.ico and apple-touch-icon.png from public/favicon.svg.
# No permanent deps: uses `bunx` (sharp-cli = libvips rasterizer, png-to-ico).
# Run from repo root:  ./scripts/gen-favicon.sh
set -euo pipefail
cd "$(dirname "$0")/.."

SVG="public/favicon.svg"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# High density rasterizes the 100-unit viewBox crisply, then resize to target.
echo "→ rasterizing 16px / 32px PNG frames for the .ico"
bunx sharp-cli --density 600 -i "$SVG" -o "$TMP/favicon-16.png" resize 16 16
bunx sharp-cli --density 600 -i "$SVG" -o "$TMP/favicon-32.png" resize 32 32

echo "→ packing favicon.ico (16 + 32)"
bunx png-to-ico "$TMP/favicon-16.png" "$TMP/favicon-32.png" > public/favicon.ico

# apple-touch: square corners (rx=0), full-bleed sage — iOS masks its own rounding.
echo "→ deriving square apple-touch source"
grep -q 'rx="22"' "$SVG" || { echo "ERROR: rx=\"22\" not found in $SVG — cannot derive square apple-touch source" >&2; exit 1; }
sed 's/rx="22"/rx="0"/' "$SVG" > "$TMP/apple-touch.svg"

echo "→ rasterizing apple-touch-icon.png (180x180)"
bunx sharp-cli --density 600 -i "$TMP/apple-touch.svg" -o public/apple-touch-icon.png resize 180 180

echo "✓ done: public/favicon.ico, public/apple-touch-icon.png"
