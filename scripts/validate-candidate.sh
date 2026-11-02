#!/usr/bin/env bash
# Validate a Candidate XML against the unofficial schema fetched by fetch-candidate-schema.sh.
# Usage: validate-candidate.sh <file.xml>   Exit 0 only if valid.
set -u
[ $# -eq 1 ] || { echo "usage: $0 <file.xml>" >&2; exit 2; }
[ -f "$1" ] || { echo "no such file: $1" >&2; exit 2; }
command -v xmllint >/dev/null || { echo "xmllint not found" >&2; exit 2; }
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec xmllint --noout --nonet --schema "$HERE/../schema/candidate/europass-candidate.xsd" "$1"
