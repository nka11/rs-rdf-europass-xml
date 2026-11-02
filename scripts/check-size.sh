#!/usr/bin/env bash
# Print the size of the built wasm and fail above 400 KB (DESIGN section 11/12.7).
set -eu
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
W="${1:-$HERE/../pkg/europass_xml_bg.wasm}"
[ -f "$W" ] || { echo "no such file: $W (run wasm-pack build --release --target web)" >&2; exit 2; }
B=$(wc -c < "$W"); G=$(gzip -9 -c "$W" | wc -c)
echo "europass_xml_bg.wasm: $B bytes raw, $G bytes gzip"
[ "$B" -le 409600 ] || { echo "too large (> 400 KB)" >&2; exit 1; }
