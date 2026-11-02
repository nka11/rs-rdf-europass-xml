#!/usr/bin/env bash
# Download the official Europass XML Schema v3.4.1 (and the two older imported schemas) into schema/.
# The schemas are NOT shipped in this repository: their upstream repository states no licence.
# Source: https://github.com/europass/ewa-cedefop, pinned to a commit. Needs `gh` (authenticated) or curl.
set -euo pipefail
REPO="europass/ewa-cedefop"
SHA="fee3b9758c6e3784e819260216c6f6d34021d562"   # pinned; bump deliberately
BASE="services/core/src/main/resources/schema"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
S="$HERE/../schema"

fetch() { # <path in repo> <destination>
  mkdir -p "$(dirname "$2")"
  curl -fsSL "https://raw.githubusercontent.com/$REPO/$SHA/$1" -o "$2"
}

command -v gh >/dev/null || { echo "gh is required to list the schema files" >&2; exit 2; }
files=$(gh api "repos/$REPO/git/trees/$SHA?recursive=1" --jq ".tree[]|select(.type==\"blob\")|select(.path|startswith(\"$BASE/v3.4.1/\"))|.path")
[ -n "$files" ] || { echo "no schema files found at $SHA" >&2; exit 1; }
for p in $files; do fetch "$p" "$S/v3.4.1/${p#$BASE/v3.4.1/}"; done
# Older schemas that v3.4.1 still imports but that the legacy host no longer serves.
fetch "$BASE/v3.0/EQF_08_V1.0.xsd" "$S/imported/EQF_08_V1.0.xsd"
fetch "$BASE/v3.0/EUDrivinglicence.xsd" "$S/imported/EUDrivinglicence.xsd"
echo "schema fetched into $S (commit $SHA)"
