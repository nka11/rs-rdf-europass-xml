#!/usr/bin/env bash
# Validate a Europass XML CV against the vendored Europass XSD v3.4.1, offline.
# Usage: validate.sh <file.xml>   Exit 0 only if valid.
# The pristine XSDs reference https://europass.cedefop.europa.eu/xml/{included,imported}/...,
# which no longer resolve. A temporary XML catalog redirects them to local copies.
set -u
[ $# -eq 1 ] || { echo "usage: $0 <file.xml>" >&2; exit 2; }
[ -f "$1" ] || { echo "no such file: $1" >&2; exit 2; }
command -v xmllint >/dev/null || { echo "xmllint not found" >&2; exit 2; }
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
S="$HERE/../schema"; S="$(cd "$S" && pwd)"
X="$S/v3.4.1"; I="$S/imported"
CAT="$(mktemp)"; trap 'rm -f "$CAT"' EXIT
H="https://europass.cedefop.europa.eu/xml"
cat > "$CAT" <<EOF
<?xml version="1.0"?>
<catalog xmlns="urn:oasis:names:tc:entity:xmlns:xml:catalog">
  <system systemId="$H/imported/EQF_08_V1.0.0.xsd" uri="file://$I/EQF_08_V1.0.xsd"/>
  <system systemId="$H/imported/EUDrivingLicence_V1.1.0.xsd" uri="file://$I/EUDrivinglicence.xsd"/>
  <system systemId="$H/imported/ISCED97_V1.0.0.xsd" uri="file://$I/stubs/ISCED97_V1.0.0.xsd"/>
  <system systemId="$H/imported/NACE_COM_V1.0.0.xsd" uri="file://$I/stubs/NACE_COM_V1.0.0.xsd"/>
  <rewriteSystem systemIdStartString="$H/included/" rewritePrefix="file://$X/included/"/>
</catalog>
EOF
XML_CATALOG_FILES="$CAT" xmllint --noout --nonet --schema "$X/EuropassSchema.xsd" "$1"
