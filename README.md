# rs-rdf-europass-xml

Rust crate (`europass-xml`) that turns a CV given as JSON into the **Europass Candidate XML**, the file the
Europass editor at europass.europa.eu imports. It builds as a native library and CLI, and as WebAssembly
(wasm-bindgen, `--target web`) so a browser can do the export. The JSON is a CV already resolved to one language,
as produced from an RDF/Turtle CV (for example by the [nka11 website](https://nka11.github.io/) or the
`eona-x-europass-cv-db` project).

The older `SkillsPassport` XML (schema v3.4.1) is no longer written: the editor refuses it as an obsolete version.

Licence: GPL-3.0-or-later (see `LICENSE`). The fixtures are fictional.

Documentation:
- [docs/terms.md](docs/terms.md): definitions of the terms used here (Candidate XML, EQF, ESCO, CEFR, golden file, ...)
- [docs/cv-input.md](docs/cv-input.md): the input JSON
- [docs/candidate.md](docs/candidate.md): the output format, mapping table, sources and licence of the schema used for validation

## Quick start

```
scripts/fetch-candidate-schema.sh          # unofficial Candidate XSD, needed by the tests (not stored here)
REQUIRE_XMLLINT=1 cargo test --locked      # needs xmllint; without REQUIRE_XMLLINT a missing xmllint skips validation
UPDATE_GOLDEN=1 cargo test --test candidate    # after an intended output change
wasm-pack build --release --target web     # -> pkg/
scripts/check-size.sh                      # wasm size gate (400 KB)
scripts/validate-candidate.sh file.xml     # xmllint against the Candidate XSD (exit 0 only if valid)
```

## Command line

`cargo build --release --bin europass-xml-cli`, then `europass-xml-cli cv.json -o cv.xml [--options '{...}']`.
Exit 0 ok, 1 export error (`[code] message`), 2 usage or I/O error. Warnings go to stderr.

## WebAssembly API

String in, string out: `exportEuropassXml(json, options?)`, `exportEuropassXmlReport(json, options?)` (returns
`{"xml", "warnings"}`), `validateCvInput(json)` and `europassXmlInfo()`. Errors are thrown as `[code] message`.

## QA

CI (`.github/workflows/ci.yml`) runs on every push and pull request: `cargo fmt --check`, `cargo clippy -D warnings`
(native, all targets, and the wasm32 library), `cargo test` with schema validation (`REQUIRE_XMLLINT=1`), and a wasm
build with the size gate. The same locally:
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.

## Schema

There is no official schema for the Candidate XML. `scripts/fetch-candidate-schema.sh` downloads an unofficial one,
reverse-engineered by J0ker98 (CC-BY-4.0, <https://github.com/J0ker98/europass-cv>), at a pinned commit into
`schema/candidate/`. It is not stored in this repository. Passing it shows the structure is right; it does not prove
the editor accepts the file.

## Known limits

- Acceptance by the live editor can only be tested by uploading a file to it; the tests cannot.
- The schema requires both `GivenName` and `FamilyName`. A one-word name is written as the family name with an empty
  given name, and the export reports `name_single`.
- Headline and summary are merged into the `ExecutiveSummary`. `summaryPlacement: "drop"` leaves the summary out.
- Websites that are not `http`, `https` or `ftp` URLs are skipped (`url_invalid`). Text is stripped of characters
  that XML 1.0 forbids (`text_sanitized`).
- Skills are not capped. An editor limit, if there is one, is not known.
- Experiences are written newest first, at month precision by default.
- Skill groups follow the source category: `TechnicalSkill` and `ToolSkill` go to computer skills,
  `ManagementSkill` to organisational skills, `MethodologicalSkill` and `ComplianceSkill` to job-related skills.
- Options must be a JSON object (or empty, or `null`). Unknown keys are ignored, except `format`, which only accepts
  `"candidate"`.

## wasm-opt

`wasm-opt` is disabled on purpose. The system `wasm-opt` is binaryen v108: with `-Os --all-features` it saves about
7%, but the module then fails at `initSync()` (`Table.set(): function-typed object must be null`). Do not re-enable it
without a newer binaryen and a run of a load test. wasm-pack prints a harmless warning about `strip = true`.
