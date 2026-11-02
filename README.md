# rs-rdf-europass-xml

Rust crate (`europass-xml`) compiled to WebAssembly (wasm-bindgen, `--target web`) that turns a language-resolved CV in
JSON, as produced from an RDF/Turtle CV by the [nka11 website](https://nka11.github.io/), into a **Europass XML CV valid
against the Europass XML Schema v3.4.1** (`SkillsPassport`, `XSDVersion V3.4`).
Contract and decisions: `docs/DESIGN.md`. Input JSON: `docs/cv-input.md`. XSD facts: `docs/xsd-map.md`.
Licence: GPL-3.0-or-later (see `LICENSE`). The fixtures are fictional.

## Quick start
```
scripts/fetch-schema.sh        # download the official XSDs (not shipped, see "Schema" below); needs gh + network
cargo test --locked            # needs xmllint; REQUIRE_XMLLINT=1 makes a missing xmllint fail
wasm-pack build --release --target web    # -> pkg/
scripts/check-size.sh          # wasm size gate
scripts/validate.sh file.xml   # xmllint against the XSD, offline (exit 0 only if valid)
UPDATE_GOLDEN=1 cargo test golden         # after an intended output change
cargo test --test codesets -- --ignored regenerate_code_sets
```

## Command line
`cargo build --release --bin europass-xml-cli` then `europass-xml-cli cv.json -o cv.xml [--options '{...}']`.
Exit 0 ok, 1 export error (`[code] message`), 2 usage/I-O; warnings go to stderr. Used by the CI pipeline of the
an internal CV database project.

## QA
CI (`.github/workflows/ci.yml`) runs on every push and pull request: `cargo fmt --check`, `cargo clippy -D warnings`
(native all-targets and the wasm32 library), `cargo test` with XSD validation (schema fetched, `REQUIRE_XMLLINT=1`), and a
wasm build with the 400 KB size gate. Run the same locally: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
`src/codesets.rs` is generated and excluded from rustfmt.

## Schema
The Europass XSDs are **not included** in this repository: their upstream repository
([europass/ewa-cedefop](https://github.com/europass/ewa-cedefop)) states no licence. `scripts/fetch-schema.sh` downloads
them at a pinned commit. Without them the schema-dependent tests fail with an explicit message.

API (string in, string out): `exportEuropassXml(json, options?)`, `exportEuropassXmlReport(json, options?)`,
`validateCvInput(json)`, `europassXmlInfo()`. Errors are thrown as `[code] message`.

## Measured (2026-11-02, rustc 1.96.1, wasm-pack 0.15.0)

| | |
|---|---|
| wasm size | 285,244 bytes raw (279 KiB), about 100 KB gzip (`opt-level = "z"`, lto, `strip = true`) |
| export time | 1.5 ms average for the real EN CV (60 KB JSON, 13 experiences, 105 skills), node 24, 200 runs after warm-up |
| tests | 28 unit + 35 integration, all XSD-validated through `scripts/validate.sh` |

**wasm-opt is disabled on purpose.** The system `wasm-opt` is binaryen v108. With `-Os --all-features` it saves
about 7% (264 KB) but the module then fails at `initSync()` (`Table.set(): function-typed object must be null`, the
externref table gets mangled). Do not re-enable it without a newer binaryen and a run of the smoke test
(vitest `europassXml.test.ts`, or load the wasm in node and compare with `tests/fixtures/golden/`). This overrides the
`wasm-opt = ["-Os", "--all-features"]` suggestion in `docs/build.md`. wasm-pack prints a harmless warning about
`strip = true` and the missing `target_features` section.

## Known limits (not bugs)

* The Europass website may no longer import the legacy XML; XSD validity is guaranteed, import into the live editor
  was not testable (`docs/xsd-map.md` section 5).
* The ISCED and NACE schemas used for validation are local stand-ins (`schema/imported/README.md`); the crate never
  emits `Field` or `Sector`, so this does not matter.
* Headline: `headline` and `summary` are merged into the single `Headline` (type `personal_statement`), Europass
  renders it under "Personal statement". Alternative: `summaryPlacement: "drop"`.
* ESCO IRIs have no slot in the 3.4.1 schema; dropped unless `includeEsco` (appended as `ESCO: <iri>` lines; experience occupations only, skill IRIs are never exported).
* Websites: non-ASCII characters, spaces, `"`, `<` and `>` are percent-encoded (a space or quote in the host is not) and malformed URLs are skipped (`url_invalid`), because
  libxml2's `xsd:anyURI` check is strict (single `#`, numeric port, no `[`). Found by the fuzz test.
* The official example `schema/v3.4.1/examples/Europass_CV_ESP_CL_LP_V3.4.0_Example.xml` does NOT validate (Code
  `12332` is not a UUID, and two `FieldName` values are invalid): the shipped example data is bad. Do not use it as a
  validator self-test; `Europass_CV_V3.4.0_Example.xml` validates and is the one to use.
* Work experience is sorted newest first by period start (stable) after long roles are split per credential.
* `exportEuropassXml` discards warnings; use `exportEuropassXmlReport` to see dropped data (`url_invalid`, ...).
* `meta.omitted` lists what the TS bridge leaves out (product-cycle statements, impacts, degree competencies);
  `meta.untagged` lists text without a language tag that follows the data, not the locale (organization names,
  education, skill categories, project names/descriptions, achievement descriptions).
* Skill groups follow the source category: `ComplianceSkill` and `MethodologicalSkill` go to `JobRelated`, so
  protocol/tool skills (LDAP, SAML, RACF) sit there; a semantic choice, not a loss.
* Options JSON must be an object (or empty / `null`); `[]` and scalars give `[bad_options]`.

## Schema version, provenance and licence

* Schema: Europass XML Schema **v3.4.1** (`XSDVersion V3.4`), fetched unmodified into `schema/v3.4.1/` by `scripts/fetch-schema.sh`
  (`EuropassSchema.xsd`, `included/`, `examples/`, `documentation/change-log.txt`).
* Source: <https://github.com/europass/ewa-cedefop>, path `services/core/src/main/resources/schema/`.
* The `xsd:import` locations (`europass.cedefop.europa.eu/xml/imported/...`) no longer resolve (the host serves HTML).
  `scripts/validate.sh` redirects them with a temporary XML catalog to local copies in `schema/imported/` (fetched, plus two stand-in stubs kept in the repo)
  (EQF and EUDrivingLicence from older schema versions, ISCED97 and NACE_COM as stand-ins; see `schema/imported/README.md`).
* **Licence warning:** the ewa-cedefop repository does not state a licence for these schema files, so they are
  downloaded by `scripts/fetch-schema.sh` rather than redistributed here. The GPL-3.0-or-later licence of this
  repository covers the Rust code, docs and fictional fixtures only, not the Europass schemas.

## Europass editor (Candidate XML)

The current Europass editor only imports the `Candidate` XML, not the legacy `SkillsPassport` one. `candidate` is
now the default output; `--format legacy` writes the old one (see [docs/candidate.md](docs/candidate.md)). Validation uses an unofficial schema
(`scripts/fetch-candidate-schema.sh`, CC-BY-4.0, J0ker98/europass-cv).
