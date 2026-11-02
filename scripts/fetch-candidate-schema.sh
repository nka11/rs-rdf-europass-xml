#!/usr/bin/env bash
# Download the unofficial Europass Candidate XSD (reverse-engineered, CC-BY-4.0) into schema/candidate/.
# Source: https://github.com/J0ker98/europass-cv, pinned to a commit. The files are not stored in this repository.
# Attribution: "Europass Candidate XML schema" by J0ker98, CC-BY-4.0, https://github.com/J0ker98/europass-cv
set -euo pipefail
REPO="J0ker98/europass-cv"
SHA="2478b3497d1555208014e871b69b446b3800de05"   # pinned; bump deliberately
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
D="$HERE/../schema/candidate"
mkdir -p "$D"
for f in europass-candidate.xsd europass-eures.xsd hr-xml-3.xsd oagis-9.xsd; do
  curl -fsSL "https://raw.githubusercontent.com/$REPO/$SHA/schema/$f" -o "$D/$f"
done
echo "Candidate schema fetched into $D (commit $SHA)"
