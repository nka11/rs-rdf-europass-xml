# DESIGN: `europass-xml` (CV JSON -> Europass XML v3.4.1, Rust -> wasm-bindgen)

Status: contract for two parallel implementers.
- **Implementer R (Rust)**: everything under `crates/europass-xml/` (sections 1-12, 14).
- **Implementer T (TypeScript)**: everything under `src/lib/semcv/europass/` and the Svelte UI (section 13).

The two meet at exactly two interfaces: the JSON input (`docs/cv-input.md`, restated as serde structs in section 3) and the wasm API (section 2). Neither side may change those without editing this file.

Inputs read: `docs/xsd-map.md` (XSD facts, element order, tested constraints), `docs/cv-input.md` (JSON contract), `docs/build.md` (toolchain decisions), `scripts/validate.sh`. Where this document and `xsd-map.md` disagree about the XSD, the XSD files in `schema/v3.4.1/` win; report the discrepancy.

## 0. Decisions (summary)

| Topic | Decision |
|---|---|
| Wasm flavour | wasm-bindgen module, `--target web`, one `.wasm` for browser, prerender and vitest (as `build.md`). |
| Core design | Typed Rust model that mirrors the XSD complex types, one `to_node()` per type that pushes children **in schema order**, then a tiny generic `Node` tree, then one serializer. No XML is ever produced by `format!` of tags. |
| XML writer | Hand-written serializer (about 80 lines, escaping in one place). `quick-xml` is NOT a dependency. Fewer deps, byte-exact determinism, and sanitising and escaping sit in one function. |
| Dependencies | `wasm-bindgen`, `serde` (derive), `serde_json`. Nothing else at runtime (no chrono, regex, getrandom, quick-xml). |
| One call | One locale, one document. Bilingual = two calls (EN then FR). The crate never reads the clock; `meta.generatedAt` is mandatory. |
| Output | UTF-8 string, XML declaration, 2-space indent, `\n` newlines, trailing newline, default namespace on the root. |
| Failure policy | Hard errors only for input that cannot yield a valid document (bad JSON, bad `schemaVersion`, bad `lang`, missing/invalid `generatedAt`). Everything else is **degraded with a warning**: the offending optional piece is omitted, the document stays XSD-valid. |
| Extra content | Free-text sections (`Activities`, `Skills/*/Description`, `Achievement/Description`) carry composed plain text. ESCO IRIs are dropped by default (the XSD has no slot), option to append. |

## 1. Crate layout

```
crates/europass-xml/
  Cargo.toml                 (skeleton in build.md; section 11 lists the deltas)
  Cargo.lock                 (committed)
  src/
    lib.rs                   wasm-bindgen exports only (thin): parses options, calls core, maps errors
    input.rs                 serde structs for CvInput (section 3) + validate_input()
    codesets.rs              generated code sets (section 4.9); tests/codesets.rs holds the staleness + regenerate tests
    options.rs               Options struct + defaults (section 2.2)
    error.rs                 ExportError, Warning, codes (section 9)
    i18n.rs                  fixed label tables en/fr (section 7)
    text.rs                  sanitize(), clamp(), join helpers, URL/email checks (section 6)
    date.rs                  parse_iso_date(), DatePrecision, to_date_attrs(), xsd_datetime check (section 5)
    xml/
      node.rs                Node { name, attrs: Vec<(&'static str,String)>, kids: Kids }, Kids::{Text(String), Elems(Vec<Node>), Empty}
      write.rs               serialize(&Node) -> String, escape_text(), escape_attr()
    model/                   one file per XSD file, types named after XSD complex types
      mod.rs                 SkillsPassport, DocumentInfo, LearnerInfo
      identification.rs      Identification, PersonName, ContactInfo, Address, Email, WebsiteList, Demographics
      headline.rs            Headline
      work.rs                WorkExperience, Employer
      education.rs           Education, Organisation
      skills.rs              SkillSet, Linguistic, MotherTongue, ForeignLanguage, GenericSkill, ComputerSkill
      achievement.rs         Achievement
      label.rs               Label { code: Option<String>, label: Option<String> } helper
    map/                     input -> model (the mapping table of section 4 lives here, one fn per row group)
      mod.rs                 fn build(input, opts, &mut Report) -> SkillsPassport
      person.rs  languages.rs  experience.rs  education.rs  skills.rs  projects.rs  activities.rs
  schema/                    (vendored, exists)
  scripts/validate.sh        (exists)
  docs/
  tests/
    codesets.rs              staleness test and `--ignored regenerate_code_sets` (4.9)
    common/mod.rs            validate_with_xmllint(xml)->Result<(),String> (calls scripts/validate.sh), fixtures loader
    fixtures/
      cv_full_en.json  cv_full_fr.json  cv_min.json  cv_edge_*.json
      golden/  cv_full_en.xml  cv_full_fr.xml  cv_min.xml
    golden.rs  xsd_validation.rs  edge_cases.rs  determinism.rs  input_validation.rs
```

Rules:
1. `model/*` types have **no** knowledge of the input JSON; `map/*` is the only place that knows both. This keeps the schema side auditable against the XSD.
2. Every model struct's `to_node()` is the single place where child order is encoded, with a comment citing the XSD file and complex type (`// EuropassLearnerInformation.xsd LearnerInfoType`).
3. `lib.rs` contains no logic besides argument handling. All logic is a pure Rust function `pub fn export(input: &CvInput, opts: &Options) -> Result<Export, ExportError>` where `Export { xml: String, warnings: Vec<Warning> }`, testable natively (`cargo test`) without wasm.

## 2. Public wasm API

All exports are string in, string out (no serde-wasm-bindgen).

```rust
/// Main entry. `input_json`: CvInput JSON (section 3). `options_json`: optional Options JSON (2.2), "" / undefined = defaults.
/// Returns the XML text. Warnings are discarded; use the _report variant to read them.
#[wasm_bindgen(js_name = exportEuropassXml)]
pub fn export_europass_xml(input_json: &str, options_json: Option<String>) -> Result<String, JsError>;

/// Same, but returns a JSON string: {"xml": "...", "warnings": [Warning...]}.
#[wasm_bindgen(js_name = exportEuropassXmlReport)]
pub fn export_europass_xml_report(input_json: &str, options_json: Option<String>) -> Result<String, JsError>;

/// Validates the INPUT only (no XML produced). Never throws for content problems:
/// returns JSON {"ok": bool, "errors": [Issue...], "warnings": [Issue...]}.
/// Never throws: unparseable JSON is also reported as an error Issue (code "json_syntax").
#[wasm_bindgen(js_name = validateCvInput)]
pub fn validate_cv_input(input_json: &str) -> String;

/// Static info for UI/tests (`crate` = `env!("CARGO_PKG_VERSION")`, never a hard-coded literal): {"crate":"<CARGO_PKG_VERSION>","xsdVersion":"3.4.1","documentXsdVersion":"V3.4","schemaVersion":1}
#[wasm_bindgen(js_name = europassXmlInfo)]
pub fn europass_xml_info() -> String;
```

Naming: the wasm-bindgen `js_name` values are camelCase (`exportEuropassXml`, ...). `build.md` used `cv_to_europass_xml` in an illustrative snippet; the canonical name is `exportEuropassXml` and the TS wrapper in section 13 uses it. `JsError` message = `ExportError` display string, prefixed with the error code: `"[invalid_lang] lang must be \"en\" or \"fr\", got \"de\""`. The TS wrapper parses the `[code]` prefix.

### 2.1 `Issue` / `Warning` JSON shape (shared by `validateCvInput` and the report)

```json
{ "code": "date_invalid", "path": "experiences[3].end", "message": "..." }
```
`path` is a JSON-path-like string into the input (`experiences[3].positions[0].projects[1].name`). Codes are in section 9. Ordering of issues: input traversal order (deterministic).

### 2.2 Options (all optional; unknown keys ignored)

```json
{
  "datePrecision": "month",        // "year" | "month" | "day"      default "month"
  "includeEsco": false,            // append ESCO IRIs as text lines in the free-text blocks (section 4.4)   default false
  "summaryPlacement": "headline",  // "headline" | "drop"           default "headline" (section 4.2)
  "includeProjects": true,         // emit AchievementList/projects                                          default true
  "includeSkills": true,           // emit Skills/* (Linguistic is always emitted when languages exist)  default true
  "maxExperiences": null,          // number | null: keep first N (input order) for a one-page variant       default null
  "maxActivitiesChars": 6000,      // clamp for Activities / Description blobs (section 6.3)                 default 6000
  "pretty": true,                  // indent 2 spaces; false = no whitespace between elements               default true
  "schemaLocationHint": false,     // emit xsi:schemaLocation on root (4.1)                                   default false
  "creationDate": null             // override DocumentInfo/CreationDate; default = meta.generatedAt
}
```
The option set is exactly these keys. There is **no** `projectsAsSingleAchievement` option (deleted: projects are one `Achievement` each, 4.7). Invalid option values produce `ExportError::BadOptions` (`bad_options`), not silent defaults, because options are written by developers, not data.

## 3. serde input structs (must match `cv-input.md` section 4)

`#[serde(rename_all = "camelCase")]` on all; every `Option<T>` field tolerates `null` or absence (`#[serde(default)]`); every `Vec<T>` has `#[serde(default)]`; **no** `deny_unknown_fields` (forward compatibility). Strings are not trimmed at deserialization; `text::sanitize` handles that during mapping.

```rust
pub struct CvInput {
    pub schema_version: i64,            // required; must be 1  (else unsupported_schema_version); wrong JSON type ("1", 1.5) -> also unsupported_schema_version via a custom check, see below
    pub lang: String,                   // required; "en" | "fr" (case-insensitive input, normalised to lowercase)
    #[serde(default)] pub person: Option<Person>, // required, checked by hand -> missing_field
    #[serde(default)] pub languages: Vec<LanguageEntry>,
    #[serde(default)] pub experiences: Vec<Experience>,
    #[serde(default)] pub education: Vec<EducationEntry>,
    #[serde(default)] pub skills: Vec<SkillEntry>,
    #[serde(default)] pub projects: Vec<Project>,
    #[serde(default)] pub meta: Option<Meta>, // required, checked by hand in validate_input() -> missing_field
}
pub struct Person {
    pub name: Option<String>, pub given_name: Option<String>, pub family_name: Option<String>,
    pub headline: Option<String>, pub summary: Option<String>, pub email: Option<String>,
    #[serde(default)] pub websites: Vec<Website>,         // { url, label? }
    // reserved by cv-input.md section 3.1; the crate supports them although this site never sets them
    pub address: Option<AddressIn>,                       // { line1?, line2?, postalCode?, municipality?, countryCode? (ISO 3166-1 alpha-2, e.g. "FR"), country? (label) }
    #[serde(default)] pub phones: Vec<Phone>,             // { number, use? ("home"|"work"|"mobile") }
    pub birth_date: Option<String>,                       // ISO date
    pub gender: Option<String>,                           // "M" | "F"
    #[serde(default)] pub nationalities: Vec<Nationality>,// { code (ISO alpha-2), label? }
}
pub struct LanguageEntry {
    pub label: String, pub level: Option<String>, pub code: Option<String>,
    #[serde(default)] pub mother_tongue: bool,
    pub cef: Option<Cef>,   // { listening?, reading?, spokenInteraction?, spokenProduction?, writing? }, each "A1".."C2"
}
pub struct Experience {
    pub id: String, pub role: Option<String>, pub organization: Option<String>,
    pub start: Option<String>, pub end: Option<String>, pub current: Option<bool>,
    pub url: Option<String>, pub description: Option<String>,
    #[serde(default)] pub bullets: Vec<String>,
    pub esco_occupation: Option<String>,
    #[serde(default)] pub skills: Vec<SkillRef>,           // accept string OR {label}  (custom Deserialize: untagged enum)
    #[serde(default)] pub positions: Vec<PositionEntry>,
}
pub struct PositionEntry { pub title: Option<String>, pub start: Option<String>, pub end: Option<String>,
    pub description: Option<String>, #[serde(default)] pub bullets: Vec<String>, #[serde(default)] pub projects: Vec<Project> }
pub struct Project { pub name: String, pub role: Option<String>, pub description: Option<String>,
    #[serde(default)] pub bullets: Vec<String>, pub start: Option<String>, pub end: Option<String>,
    pub url: Option<String>, #[serde(default)] pub skills: Vec<SkillRef> }
pub struct EducationEntry { pub title: String, pub school: Option<String>, pub city: Option<String>,
    pub level_label: Option<String>, pub eqf: Option<serde_json::Number>, pub start: Option<String>, pub end: Option<String>,
    #[serde(default)] pub projects: Vec<Project> }
pub struct SkillEntry { pub label: String, pub category: Option<String>, pub uses: Option<serde_json::Number>,
    pub esco: Option<String>, pub esco_match: Option<String> }
pub struct Meta { #[serde(default)] pub generated_at: String, pub generator: Option<String>, pub variant: Option<String>,
    pub source_url: Option<String>, #[serde(default)] pub omitted: Vec<String>, #[serde(default)] pub untagged: Vec<String> }
```
`SkillRef` is `#[serde(untagged)] enum { Plain(String), Obj{ #[serde(default)] label: String } }` because `cv-input.md` shows experience `skills` as `[{label}]` in 3.3 and as plain strings in the TS interface/examples; accept both.
**Leniency rule (resolves the "degrade with a warning" policy)**: every `String` field that is shown as non-Option above (`Experience.id`, `Project.name`, `EducationEntry.title`, `LanguageEntry.label`, `SkillEntry.label`, `Meta.generated_at`, `Website.url`, `Phone.number`, `Nationality.code`) carries `#[serde(default)]`; empty = absent, and `validate_input()` then emits `empty_entry` (entry skipped) or, for `meta.generatedAt`, the hard `missing_field`. Numeric fields (`eqf`, `uses`) are `serde_json::Number` so `9.5`, `-1`, `256` deserialize and are range-checked by hand (`eqf_out_of_range` warning; `uses` is advisory and unused). `schemaVersion` is read as `i64`; a missing/non-integer value gives `unsupported_schema_version`, a missing `person` or `meta` gives `missing_field` (path `person`, `meta`, `meta.generatedAt`). `json_syntax` therefore means only "not JSON, or a wrong container type" (e.g. `experiences: 3`, `person: []`). The `eqf` edge tests (0, 9, 255, 256, 9.5, -1) all expect a warning, never a hard error.
`end: null` and `end` absent are identical. `current` absent: derived as `end.is_none()` only for experiences (matches `cv-input.md`); for positions/projects `current` is never emitted.

## 4. Mapping table (CV field -> exact XSD path)

Paths are relative to the root `SkillsPassport` (default namespace `http://europass.cedefop.europa.eu/Europass`). `[A]` = attribute. Element order below is the emission order. Types come from `docs/xsd-map.md`, which was read from the XSDs.

### 4.1 Document level

| Source | XSD path | Rule |
|---|---|---|
| `lang` | `SkillsPassport[@locale]` | `en` / `fr` lowercase. Pattern `[a-zA-Z]{2}(-[a-zA-Z]{3})?` is satisfied. |
| constant | `DocumentInfo/DocumentType` | `ECV` |
| (skipped) | `DocumentInfo/Bundle` | omit |
| `meta.generatedAt` (or `options.creationDate`) | `DocumentInfo/CreationDate` | `xsd:dateTime`; must match `YYYY-MM-DDThh:mm:ss(.fff)?(Z|±hh:mm)?`, calendar-validated (5.3); output unchanged if valid. |
| same value | `DocumentInfo/LastUpdateDate` | identical to CreationDate (we have no separate edit time). |
| constant | `DocumentInfo/XSDVersion` | literal `V3.4` (XSD pattern forbids anything else) |
| `meta.generator` (default `concat!("europass-xml ", env!("CARGO_PKG_VERSION"))`) | `DocumentInfo/Generator` | sanitised, clamped to 200 chars |
| `meta.sourceUrl` | `DocumentInfo/Comment` | `"Source: <url>"` + (variant ? `" (variant: <slug>)"`); only if `sourceUrl` present; omitted otherwise |
| (skipped) | `DocumentInfo/Copyright` | never emitted (fixed-literal rule; see xsd-map 2) |
| (skipped) | `DocumentInfo/EuropassLogo` | omit |
| (skipped) | `PrintingPreferences`, `AttachmentList`, `CoverLetter` | never emitted |

Root attributes, in this order: `xmlns`, `xmlns:xsi`, `xsi:schemaLocation` (only if `schemaLocationHint` is true, default false; value `http://europass.cedefop.europa.eu/Europass http://europass.cedefop.europa.eu/xml/v3.4.1/EuropassSchema.xsd`, the same pattern as the shipped examples, which use a different host and v3.4.0; the hint is informational only because `validate.sh` passes `--schema` explicitly and validation was checked with and without it), `locale`.

### 4.2 `LearnerInfo/Identification` and `Headline`

LearnerInfo child order: `Identification, Headline, WorkExperienceList, EducationList, Skills, AchievementList` (`Documentation` never emitted). `LearnerInfo` is always emitted, empty (`<LearnerInfo/>`) if nothing maps.

| Source | XSD path | Rule |
|---|---|---|
| `person.givenName`, `familyName` | `Identification/PersonName/FirstName`, `/Surname` | preferred when present |
| `person.name` (fallback) | same | split on the **last** ASCII-space (after collapsing whitespace): everything before = `FirstName`, last token = `Surname`. One token only: `FirstName` = token, no `Surname`. Both given/family absent and name absent: no `PersonName`. If only one of givenName/familyName is present, emit just that one. |
| `person.email` | `Identification/ContactInfo/Email/Contact` | strip leading `mailto:` (case-insens.), trim; must match the XSD pattern `[^@]+@[^\.]+\..+` AND contain no whitespace, else warning `email_invalid` and omit. |
| `person.address` | `ContactInfo/Address/Contact/{AddressLine,AddressLine2,PostalCode,Municipality,Country}` | only the sub-fields present. `Country`: emit `<Code>` only if `countryCode` is two ASCII letters (uppercased) AND present in the compiled-in country set (section 4.9); `<Label>` from `country`/i18n when given. Unknown code: keep `Label` if present, drop `Code`, warn `country_code_unknown`. |
| `person.phones[]` | `ContactInfo/TelephoneList/Telephone/{Contact,Use/Code}` | `Use/Code` only if in `home|work|mobile` (else omit `Use`, warn `enum_unknown`). |
| `person.websites[]` | `ContactInfo/WebsiteList/Website/{Contact,Use/{Code,Label}}` | `Contact` = `url`, must pass `text::valid_uri` (http/https/ftp scheme, no whitespace, <= 2000 chars) else warning `url_invalid` and skip that entry. `Use`: `label` -> code by the fixed table: GitHub -> `portfolio`, CV -> `portfolio`, case-insensitive match of `blog` -> `blog`, `linkedin`/`personal` -> `personal`; anything else -> `personal`. `Use/Label` = the input `label` verbatim (visible text). Rationale: the XSD code list is closed (`personal|business|blog|portfolio`) and `Use` is optional; `cv-input.md` says "map to a code only if certain" so the table is intentionally tiny and `personal` is the fallback only when a label is present; no label -> no `Use`. |
| `person.birthDate` | `Demographics/Birthdate[@year,@month,@day]` | full precision regardless of `datePrecision` (a birth date is exactly what was given). |
| `person.gender` | `Demographics/Gender/{Code,Label}` | `M` or `F` only; Label from i18n (`Male`/`Homme`, `Female`/`Femme`). |
| `person.nationalities[]` | `Demographics/NationalityList/Nationality/{Code,Label}` | code must be in the nationality set (4.9) else dropped with warning. |
| (skipped) | `Photo`, `Signature`, `PersonName/Title`, `ContactInformation/InstantMessagingList`, `ContactMethodList` | never |

`ContactInfo` child order: `Address, Email, TelephoneList, WebsiteList` (then IM, ContactMethod, never emitted). `Demographics` order: `Birthdate, Gender, NationalityList`. An element whose children all end up omitted is itself omitted (see 8).

**Headline** (`LearnerInfo/Headline`, singleton):

| Case | Output |
|---|---|
| `summary` present and `summaryPlacement == "headline"` | `Type/Code=personal_statement`, `Type/Label` (i18n: `Personal statement`/`Profil`), `Description/Label` = `headline` + `"\n\n"` + `summary` (if headline present), else `summary` alone. |
| only `headline` | `Type/Code=position`, `Type/Label` (i18n `Position`/`Poste`), `Description/Label = headline`. |
| `summaryPlacement == "drop"` | as if `summary` absent. |
| neither | no `Headline`. |

`Description/Code` is **never** emitted (UUID pattern; we have no Europass occupation ids). Reason for merging headline + summary: the XSD allows one `Headline` and has no other prose slot for a personal statement. Known trade-off (to be recorded in the crate README, owner may reverse it): Europass renders this block under "Personal statement", so the job title is no longer shown as a Position headline and a summary of up to 3000 chars sits in a field normally holding a short label. It validates and loses no data. Alternative if the owner prefers: emit `Headline` as `position` (title only) and move the summary elsewhere (Achievement or Skills/Other).

### 4.3 `WorkExperienceList/WorkExperience`

One `WorkExperience` per `experiences[]` entry, input order preserved (the producer already sorts, `cv-input.md` 3.3). Child order (ExperienceType base then extension): `Period, Description, Documentation, Position, Activities, Employer`. Only `Period`, `Position`, `Activities`, `Employer` are emitted (`Description` is not used: `Activities` carries the text so Europass prints it; `Documentation` never).

| Source | XSD path | Rule |
|---|---|---|
| `start`, `end`, `current` | `Period/From[@year,@month,@day]`, `Period/To[...]`, `Period/Current` | `From` if `start` valid. If `current` (explicit true, or `end` null/absent and `current` not false): `<Current>true</Current>`, **no `To`**. Else `To` from `end` if valid. An `end` that is present but invalid does NOT make the experience current: no `Current`, no `To` (warning `date_invalid`). Neither date valid and not current: no `Period`. `Duration`/`DurationEquivalent` never emitted. A `start` after `end` is kept as is with warning `date_order`. |
| `role` (fallback: `positions[0].title` if non-empty, else omitted) | `Position/Label` | no `Position/Code`. |
| composed (4.4) | `Activities` | single string. Omitted if composition is empty. |
| `organization` | `Employer/Name` | |
| `url` | `Employer/ContactInfo/Website/Contact` | valid URI only; no `Use`. (`Employer/ContactInfo/Address` is never emitted.) |
| (skipped) | `Employer/Sector` | |

An entry with no `Position`, no `Employer` and no `Period` is skipped with warning `empty_entry`.

### 4.4 Composing `Activities` (plain text, deterministic)

The XSD gives one string. Blocks are separated by a blank line (`\n\n`); inside a block lines are separated by `\n`. Bullets render as `- text` (ASCII hyphen-minus, so it reads the same in FR and EN and survives every font). Dates in text use `YYYY-MM` (`YYYY` if only a year, `YYYY-MM-DD` only when `datePrecision == "day"`), and an open end renders as `present`/`aujourd'hui` (i18n) **only when `end` is null or absent AND (for experiences) `current` is not false**. For positions and projects there is no `current`, so "open" there means `end` null/absent. If `end` is present but invalid (warning `date_invalid`), the end part is omitted and only the start is printed (`<start>`, no ` - `); never print `present` for an invalid end. Same rule in 4.7.

```
[description]
[- bullet]...
[Skills: a, b, c]                         // label i18n: "Skills:" / "Compétences :"  ; list = experience.skills labels, input order, de-duplicated

For each positions[] entry, in order:
[<title> (<start> - <end|present>)]       // header line; omitted if the title is empty AND there are no dates;
[position.description]                    // if title empty, dates alone are not printed
[- position.bullets]...
  For each project in position.projects:
  [<name> - <role> (<start> - <end|present>)]
  [project.description]
  [- project.bullets]...
  [Skills: ...]
```
- A position with an empty title and a single project (the Telco Corp case in `cv-input.md`) prints the project block only, no header.
- Date range separator is ` - ` (space, hyphen, space), not an en dash, to keep ASCII; an en dash is allowed in input text and passes through.
- `includeEsco`: append a final line `ESCO: <iri>` when `escoOccupation` is a syntactically valid `http://data.europa.eu/esco/occupation/<uuid>` IRI, otherwise ignored.
- Clamped to `options.maxActivitiesChars` Unicode scalar values (6.3).

### 4.5 `EducationList/Education`

Order of emission: `Period, Title, Activities, Organisation, Level` (full XSD order is `Period, Description, Documentation, Title, Activities, Organisation, Level, Field`).

| Source | XSD path | Rule |
|---|---|---|
| `start`, `end` | `Period/From`, `Period/To` | same date rules as work; no `Current`. |
| `title` | `Title` | |
| `projects[]` (composed like 4.4 project blocks) | `Activities` | omitted if no projects. |
| `school` | `Organisation/Name` | |
| `city` | `Organisation/ContactInfo/Address/Contact/Municipality` | only if present. (Country is not known: not emitted.) |
| `eqf` (1..8) | `Level/Code` | out of range or non-integer: omit `Code`, warn `eqf_out_of_range`. |
| `levelLabel` | `Level/Label` | if `eqf` valid and `levelLabel` absent: `Label` = `EQF <n>`. |
| (skipped) | `Field` | never guessed (ISCED). |

Sorted as given by the producer (end desc). The crate never re-sorts anything; order is input order.

### 4.6 `Skills`

`Skills` child order: `Linguistic, Communication, Organisational, JobRelated, Computer, Driving, Other`. Emitted: `Linguistic`, `Organisational`, `JobRelated`, `Computer`, and `Other`. Never `Communication`, `Driving`.

**Linguistic** (`Linguistic/MotherTongueList/MotherTongue/Description/{Code,Label}`; `Linguistic/ForeignLanguageList/ForeignLanguage/Description/{Code,Label}`)

| Rule | |
|---|---|
| `motherTongue == true` | goes to `MotherTongue`; others to `ForeignLanguage`. Order in each list = input order. |
| `Code` | lower-cased ISO 639-1 `code`, emitted only if in the compiled-in set for that element (`motherCode` set for MotherTongue, `foreignCode` set for ForeignLanguage; sets extracted from `included/EuropassISOLanguages_V1.10.0.xsd` by a build-time test, section 12). Not in set: no `Code`, keep `Label`, warn `language_code_unknown`. |
| `Label` | `label` verbatim (already localised). |
| `ProficiencyLevel` (foreign only) | emitted only if `cef` has at least one valid level: children in XSD order `Listening, Reading, SpokenInteraction, SpokenProduction, Writing`, each value `A1..C2`; a missing individual skill is simply omitted (the pattern also allows the empty string, but we do not emit empty elements). The textual `level` ("Bilingual") is never converted to a CEF level. |
| `level` text with no `cef` | not emitted into a structured field. Not appended to `Label`. (cv-input.md 3.2.) |
| a language entry with `motherTongue` and no valid `Code` and empty label | skipped. |

**Group mapping from `skills[]`** (category local names from `cv-input.md` 3.5): `TechnicalSkill`, `ToolSkill` -> `Computer`; `ManagementSkill` -> `Organisational`; `MethodologicalSkill`, `ComplianceSkill` -> `JobRelated`; missing or unknown category -> `Other`. Each group emits one `Description` string:
`label1, label2, label3` (input order = most used first as the producer sorted; de-duplicated case-insensitively, first occurrence wins). If `uses` is present the labels are NOT annotated with counts (keeps text clean). With `includeEsco`, ESCO is not appended per skill (too noisy); it is dropped, documented limitation.
`Computer` has no `ProficiencyLevel` (we have no DigCom data; never invent A/B/C).
Text longer than `maxActivitiesChars` is cut at the last `, ` boundary before the limit; **no ellipsis is appended** (Skills descriptions are explicitly exempt from the ellipsis rule of 6.3); warning `text_truncated`.

### 4.7 `AchievementList/Achievement` (personal projects)

If `includeProjects` and `projects` non-empty: **one** `Achievement` per project (so Europass prints each with its heading), in input order:
- `Title/Code` = `projects`, `Title/Label` = i18n (`Projects`/`Projets`).
- `Description` = block: `<name> - <role> (<start> - <end|present>)` (the `present` rule of 4.4 applies: invalid end prints only the start) / `description` / `- bullets` / `URL: <url>` (only a valid URI) / `Skills: ...`.
- Order inside `Achievement`: `Title, Description` (never `Documentation`).

Resulting layout: N achievements all headed "Projects". If the Europass renderer groups by Title code, that is acceptable; if you prefer one block, the alternative (one Achievement with all projects joined by blank lines) is behind a hidden option `"projectsAsSingleAchievement": true` (default false; implementer R adds it if trivial, otherwise leaves it out).

### 4.8 Data NOT mapped

`experiences[].place`, `impacts`, `escoOccupation` (unless `includeEsco`), `skills[].esco/escoMatch/uses`, `meta.omitted/untagged` (advisory, never in the XML), `Photo`, `Signature`, `Documentation`, `Attachment*`, `PrintingPreferences`. The XSD has no ESCO or ELM slot (`cv-input.md` 3.8).

### 4.9 Code sets compiled in

Generated once by a Rust test/`build.rs`-free script (a documented `cargo test -- --ignored regenerate_code_sets` that rewrites `src/codesets.rs`, plus a normal test that fails if `src/codesets.rs` is stale versus `schema/v3.4.1/included/*.xsd`): `COUNTRIES` (from `EuropassISOCountries_V1.7.0.xsd`), `NATIONALITIES`, `MOTHER_LANGS`, `FOREIGN_LANGS`, `EQF`=1..8, `ACHIEVEMENT_TITLES`, `WEBSITE_USES`, `TELEPHONE_USES`. Each is a sorted `&'static [&'static str]` searched with `binary_search`. Sizes are small (hundreds of 2-letter codes). The regeneration script reads the XSD with a minimal line scan, not a full XML parser, but it must: (1) track the enclosing `<xsd:simpleType name="...">` and collect only into the requested type (`foreignCode` at line 38 and `motherCode` at line 4387 of the languages XSD are two different sets; `countryCode`; `nationalityCountryCode`); (2) collect only `value=` attributes of `xsd:enumeration` elements (an enumeration opens a multi-line element with an annotation); (3) sort by byte order and de-duplicate (the XSD is ordered by language name, not code, so `binary_search` needs the sort); (4) the staleness test asserts the expected counts, verified against the vendored XSDs with `grep -o 'enumeration value='`: `foreignCode` 164 + `motherCode` 158 = 322 language enumerations, `countryCode` 198, `nationalityCountryCode` 198. (Review finding F5 quoted 644 and 396; those are twice the real counts, probably counting open and close tags, and are rejected. The scoping and sorting requirements of F5 are correct and adopted.)

## 5. Dates

0. Real data defect: `static/cv/projects/author-AcmeCos.ttl:21` and `static/cv/projects.ttl:278` hold `endDate "20014-12-01"` (typo for 2014). The crate rejects it (`date_invalid`) as designed; T must fix the typo in both TTL files (section 13) and `buildCvInput` must drop (or flag in `meta.omitted`) any date not matching `^\d{4}(-\d{2}(-\d{2})?)?$`.
1. Accepted input shapes: `YYYY-MM-DD`, `YYYY-MM`, `YYYY` (ASCII digits only, year 1000..9999 after parse; `0000` rejected). Anything else (`2020/03/15`, `15-03-2020`, empty) -> warning `date_invalid` at the JSON path, field treated as absent.
2. Calendar validation by hand: month 1..12, day 1..days_in_month (leap year rule), no chrono.
3. Output `DateType`: `year="YYYY"` always (4 digits, zero padded); `month="--MM"` when precision >= month and month known; `day="---DD"` when precision == day and day known. An unknown finer part is never invented. Never `month="03"` (tested invalid in `xsd-map.md`). A day is never emitted without a month.
4. Text dates (4.4): `YYYY`, `YYYY-MM`, `YYYY-MM-DD` according to the same precision and what is known.
5. `generatedAt` -> `xsd:dateTime`: regex-free validator for `YYYY-MM-DDThh:mm:ss`, optional `.` 1+ digits, optional `Z` or `±hh:mm`; hour 0..23 (24:00:00 rejected), min/sec 0..59. Failure = hard error `invalid_generated_at`, because `CreationDate` is mandatory. The crate does not normalise the string (emits it as received after validation) so output stays a pure function of input.

## 6. Escaping and sanitising

All character data flows through `text::sanitize` then `write::escape_text` / `escape_attr`. Nothing is written raw.

### 6.1 `sanitize(&str) -> String` (applied to every string from input before it enters the model)
1. Normalise newlines: `\r\n` and lone `\r` -> `\n`. (Without this, XML parsers normalise them anyway; we do it ourselves so output is stable.)
2. Remove characters illegal in XML 1.0: anything outside `#x9 | #xA | #xD | [#x20-#xD7FF] | [#xE000-#xFFFD] | [#x10000-#x10FFFF]`. That means C0 controls except TAB and LF, DEL is *legal* in XML 1.0 but discouraged (U+007F-U+009F): **also removed**, and U+FFFE/U+FFFF (Rust `char` can hold them). Lone surrogates cannot occur in a Rust `&str`. Removed, not replaced (no U+FFFD), and counted into one `text_sanitized` warning per field.
3. Remove U+200B, U+2060, U+FEFF (zero-width space, word joiner, BOM; they break matching and copy/paste). U+200C (ZWNJ) and U+200D (ZWJ) are kept: they are valid XML 1.0 characters and change how emoji sequences and Arabic, Persian and Indic scripts are shaped and replace U+00A0 with a normal space. U+2028/U+2029 -> `\n`.
4. Tabs -> single space.
5. Per line: trim trailing spaces; collapse runs of 3+ `\n` to exactly 2; trim leading/trailing blank lines of the whole string. Intra-line runs of spaces are left alone except in single-line fields.
6. **Single-line fields** (names, labels, titles, `Generator`, URLs, `Contact`, org names, etc.): after 1-4, replace every `\n` with a space, collapse whitespace runs to one space, trim. **Multi-line fields**: `Activities`, `Description`s, `Headline/Description/Label`, `Achievement/Description`, `Skills/*/Description`.
7. Unicode is NOT normalised (no NFC/NFKC; avoids tables and bytes). The TS side feeds NFC from the TTL already; the crate keeps what it receives. Accents (`é`, `ç`) are emitted as UTF-8, never as numeric references.
8. A field that is empty after sanitising is treated as absent.

### 6.2 Escaping
- Text nodes: `&` -> `&amp;`, `<` -> `&lt;`, `>` -> `&gt;`. Nothing else (quotes stay literal in text).
- Attribute values (double-quoted): `&`, `<`, `>`, `"` -> `&amp; &lt; &gt; &quot;`; and `\n`, `\t`, `\r` -> `&#10; &#9; &#13;` (attributes are normalised by parsers otherwise). Attribute values we write are all machine-built (years, `--03`, `true`, locale) so this is defensive.
- **CDATA is never used.** Reasons: it cannot contain `]]>` without splitting, interacts badly with sanitising, and is useless here (the XSD types are plain `xsd:string`). Escaped entities are sufficient and canonical. A test asserts `!xml.contains("<![CDATA[")`.
- No comments, no processing instructions other than the XML declaration (a comment in the output could carry unsanitised text; none are emitted).
- Declaration: `<?xml version="1.0" encoding="UTF-8"?>\n`.

### 6.3 Length limits
The XSD has **no** length limits (`xsd-map.md` 0), so limits are ours, to keep files sane and renderers happy. All counted in Unicode scalar values after sanitising; on overflow (except `Skills/*/Description`, see 4.6) the string is cut and ends with `…` (U+2026, which counts toward the limit) at the last whitespace or `, ` boundary in the last 20% of the allowed length, else a hard cut on a scalar boundary; warning `text_truncated` with path and original length.

| Field class | Limit |
|---|---|
| Names, titles, labels, org names, `Position/Label`, `Level/Label`, `Generator` | 200 |
| `Contact` (email) | 254; URL 2000 (over -> `url_invalid`, entry skipped, never truncated) |
| `Headline/Description/Label` | 3000 |
| `Activities`, `Skills/*/Description`, `Achievement/Description` | `maxActivitiesChars` (default 6000) |
| Total elements: `WorkExperience` 100, `Education` 50, `Achievement` 100, language entries 30, websites 20 | beyond the cap: dropped from the tail, warning `list_truncated` |

Structural nesting depth is fixed by the schema, so no recursion guard is needed. Input JSON larger than 4 MiB is rejected (`input_too_large`) before parsing.

## 7. Language / locale handling

- `lang` selects only (a) the `locale` attribute and (b) the fixed label strings the crate itself writes. The crate **never translates data** and never falls back across languages (matches `cv-input.md` 2).
- Default locale for options/labels/`Generator` messages: `en`; second: `fr`. An unsupported `lang` (e.g. `de`) is a hard error `invalid_lang`. Reason: the table below only exists for en/fr and shipping a mixed-language document is worse than failing. (Adding a locale = adding a column to `i18n.rs` and to `locale`'s allowed set; the XSD accepts any 2-letter code.)
- `i18n.rs` is a `match` on an enum `Locale { En, Fr }` returning `&'static str`; keys:

| Key | en | fr |
|---|---|---|
| headline_position | Position | Poste |
| headline_personal_statement | Personal statement | Profil |
| achievement_projects | Projects | Projets |
| skills_prefix (text) | Skills: | Compétences : |
| present | present | aujourd'hui |
| url_prefix | URL: | URL : |
| gender_m / gender_f | Male / Female | Homme / Femme |
| eqf_label | EQF {n} | CEC {n} |
| source_comment | Source: | Source : |

(FR uses the typographic space before `:`; it is an ordinary U+0020 here to keep ASCII-safe; implementer may use U+00A0 only if `sanitize` is bypassed for the crate's own constants, which is NOT allowed: keep U+0020.)
- Language `Label`s in `Skills/Linguistic` come from the input (`languages[].label`), already localised by the producer. The crate does not know language names.
- Country `Label` for `address.country` also comes from input; the crate has no country-name table.

## 8. What is omitted when data is missing

Principle (from `cv-input.md`): omit, never fabricate. Implementation rule: `to_node()` returns `Option<Node>`; a container returns `None` when it has no children (after its own children were pruned), so no empty wrappers (`<Identification/>`, `<TelephoneList/>`) are emitted. Exceptions that are always emitted: `SkillsPassport`, `DocumentInfo` (+ required children), `LearnerInfo` (possibly empty).

| Missing | Effect |
|---|---|
| email / websites / address / phones | no `ContactInfo` (or only the present parts) |
| `name` and given/family | no `PersonName`; `Identification` may vanish |
| headline and summary | no `Headline` |
| experience dates | no `Period`; entry still emitted if it has `Position`/`Employer` |
| experience `end` null | `Current=true`, no `To` |
| `role` empty | fall back to `positions[0].title`; else no `Position` |
| no `eqf` | `Level` has only `Label` (if any) |
| no `cef` | `ForeignLanguage/Description` only, no `ProficiencyLevel` |
| no `code` for a language | `Description/Label` only |
| unknown skill category | `Skills/Other` |
| no projects | no `AchievementList` |
| nothing at all | `<LearnerInfo/>` only, still valid |
| `meta.generator` missing | `Generator` = `europass-xml <CARGO_PKG_VERSION>` (required by XSD, so a default is correct) |

The crate never emits: placeholder text (`N/A`, `-`), default countries, guessed CEFR/DigComp levels, guessed ISCED/NACE, today's date.

### 8.1 Fidelity rules for long, repeated or inconsistent data

- **Long experiences are split, not cut.** When the composed `Activities` text of an experience exceeds
  `maxActivitiesChars` and it has two or more positions (credentials), one `WorkExperience` per position is emitted,
  each with its own `Position` (the credential title), `Period` (the credential dates) and the same `Employer`.
  The experience description and skills go with the first (latest) entry. Warning `experience_split`.
- **Truncation is visible.** If a text is still too long it is cut on a blank line, else on a line boundary, and a
  marker line (`[Remainder omitted: length limit]` / `[Suite omise : limite de longueur]`) is appended. Only a single
  line longer than the limit is cut inside. Warning `text_truncated`. The download UI shows these warnings.
- **No repeated text.** A position or project description that is a near-duplicate (word-set overlap of at least 85%)
  of an enclosing description is skipped; a project role equal to the experience role or position title is skipped; a
  single position whose title starts the experience role and whose dates equal the experience dates gets no header
  line (Position and Period already say it).
- **Project outside its parent period.** A project whose period lies completely outside the period of its education
  entry or experience loses its dates (the project stays) and warning `project_outside_parent_period` is raised.
  The source data (for example `static/cv/projects/iut-stage.ttl`, 2022-2023 under a 1996-1998 DUT) should be fixed.
- **Language level.** For a foreign language without CEFR levels, the free-text level is appended to the label:
  `English (Bilingual)`. A mother tongue gets no suffix (the `MotherTongue` element says it). No CEFR level is inferred.
- **Zero-width characters.** U+200C and U+200D are kept (see section 6).
- **ESCO.** Schema 3.4.1 has no ESCO slot. Occupation IRIs are exported only with `includeEsco` (text line); skill ESCO
  IRIs are never exported. The download UI has the toggle and says so.

## 9. Error model

```rust
pub enum ExportError {                       // hard errors, become JsError
    JsonSyntax{line:usize,col:usize,msg:String},          // "json_syntax"
    InputTooLarge{bytes:usize},                            // "input_too_large"
    UnsupportedSchemaVersion{got:i64},                     // "unsupported_schema_version"
    InvalidLang{got:String},                               // "invalid_lang"
    MissingField{path:&'static str},                       // "missing_field"   (person, meta, meta.generatedAt)
    InvalidGeneratedAt{got:String},                        // "invalid_generated_at"
    BadOptions{msg:String},                                // "bad_options"
}
```
Warning codes (never fail the export): `date_invalid`, `date_order`, `text_sanitized`, `text_truncated`, `list_truncated`, `url_invalid`, `email_invalid`, `eqf_out_of_range`, `enum_unknown`, `country_code_unknown`, `language_code_unknown`, `cef_invalid`, `empty_entry`, `unknown_category` (info), `duplicate_skill` (info).

Rules:
1. No panics: `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]` (set identically as `deny` in the section 11 `[lints.clippy]` table; use `.get()` instead of indexing` outside tests; `panic = "abort"` in release (build.md) means a panic kills the wasm instance, so nothing may panic on user data. A property-style test feeds garbage (section 12).
2. Wrong JSON *types* (e.g. `"experiences": 3`) are a `json_syntax`-class error with serde's message and path (`serde_path_to_error` is NOT used, to avoid the dependency; the message from serde_json is enough).
3. `validateCvInput` and `export()` share ONE function `collect_issues(&CvInput, &Options) -> Vec<Issue>` (in `input.rs`), so they can never disagree. `export()` calls it and stops at the first hard error; `validateCvInput` reports all issues (hard ones as `errors`, the rest as `warnings`) without building XML.
4. Every error/warning message is English, single line, includes the offending value truncated to 60 chars.
5. Postcondition (debug assertion + test): the produced string is well-formed (a mini checker in tests: balanced tags, only allowed chars) and, in CI-native tests, XSD-valid.

## 10. Determinism

Output = pure function of `(input_json semantics, options)`.
- No clock, no randomness, no `HashMap` iteration (use `Vec`/`BTreeMap`; order always = input order or a fixed XSD order).
- No locale-dependent formatting; integers via `to_string()`.
- Attribute order fixed in code; `xmlns` ordering fixed (4.1).
- De-duplication is "first occurrence wins" with a case-insensitive key from `str::to_lowercase()` (documented: it is Unicode-aware but locale-free).
- Line endings `\n`; trailing newline exactly one; no BOM.
- `pretty: false` produces a single line (after the declaration + `\n`).
- Test: export twice and from JSON with shuffled *object key order* -> byte-identical.

## 11. Cargo / wasm profile

Start from `build.md` skeleton; deltas:

```toml
[dependencies]
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
# quick-xml: not used (decision 0)

[dev-dependencies]
# none required; golden/validation use std::process::Command (xmllint). Optional: proptest = "1" only if already in ~/.cargo/registry (offline!), otherwise hand-rolled generators (section 12).

[lints.rust]
unsafe_code = "forbid"
[lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
indexing_slicing = "deny"
```
Keep `[profile.release]` (opt-level "s", lto, codegen-units 1, panic "abort", strip) and the `wasm-opt = ["-Os","--all-features"]` metadata exactly as in `build.md` (system wasm-opt v108 needs `--all-features`). `crate-type = ["cdylib","rlib"]`. Use `--locked` and commit `Cargo.lock`; resolve offline first (`cargo build --offline`), the registry already holds the crates. wasm-bindgen exports are guarded with `#[cfg(target_arch = "wasm32")]` only if native tests need to avoid linking issues; normally `#[wasm_bindgen]` functions compile natively fine, and `JsError` construction on native panics, so **core errors are mapped to `JsError` only inside the wasm-facing wrappers, and native tests call `export()` directly**.
Size budget: <= 250 KB raw wasm after wasm-opt (an estimate, not yet measured on this model; probe was 96 KB); fail the build script check above 400 KB. `unsafe_code = "forbid"` together with `#[wasm_bindgen(js_name)]` functions returning `Result<String, JsError>` and taking `Option<String>` was verified to build natively and for wasm32 and `cargo test` runs.

## 12. Test plan

All native, run by `npm run test:rust` (`cargo test --manifest-path crates/europass-xml/Cargo.toml --locked`). XSD validation shells out to `scripts/validate.sh` (needs `xmllint`); the helper skips with a visible `eprintln!("SKIP: xmllint missing")` when absent, unless env `REQUIRE_XMLLINT=1` (then it fails).

1. **Unit** (in-module `#[cfg(test)]`):
   - `text`: sanitize of `\u{0}`, `\u{1F}`, `\u{7F}`, `\u{FFFE}`, CRLF, NBSP, zero-width, tab; idempotence (`sanitize(sanitize(x)) == sanitize(x)`); truncation on scalar boundary with multi-byte chars (emoji, `é`, combining marks).
   - `write`: escaping of `& < > "` and of `]]>` in text; attribute newline escapes; empty-element form `<LearnerInfo/>`.
   - `date`: `2024-02-29` valid, `2023-02-29` invalid, `2024-13-01`, `2024-00`, `0000`, `2024-6` (invalid), `---`; DateType output `month="--03"`; datePrecision matrix; dateTime validator (`Z`, `+02:00`, fractional, `24:00:00` rejected).
   - `map::person`: name splitting (`Alex Martin`, `Jean de La Fontaine` -> FirstName `Jean de La`, Surname `Fontaine`, documented limitation; single token; extra whitespace; given/family override), email `mailto:` strip, invalid emails.
   - `map::activities`: golden strings for the telco-like shape (empty title, one project), hr-ate shape (4 positions, no text), bullets, clamp.
   - Language/code sets: staleness test against the XSD (4.9).
   - Order test: for each model type, serialise a fully-populated instance and assert child order equals the XSD sequence (list hard-coded in the test from the XSD, **not** derived from the code).
2. **Golden XML** (`tests/golden.rs`): `fixtures/cv_full_en.json` is the example in `cv-input.md` section 5 (copy, complete it); `cv_full_fr.json` the FR counterpart (accented text, `current` role, French quotes `« »`, apostrophes). Output compared byte-for-byte with `fixtures/golden/*.xml`. Regenerate with `UPDATE_GOLDEN=1 cargo test golden`; golden diffs are reviewed in PRs. Goldens are also the fixtures for the TS tests (T copies them to `src/lib/semcv/europass/__fixtures__/`, section 13).
3. **XSD validation** (`tests/xsd_validation.rs`): every golden and every edge fixture output must pass `scripts/validate.sh`. Also: (a) negative control, a hand-broken copy (swap `Skills` before `Headline`) must FAIL validation, proving the harness works; (b) the five upstream examples are not used as inputs.
4. **Edge cases / property-style** (`tests/edge_cases.rs`, no external crates; a small xorshift PRNG seeded with a constant so runs are reproducible):
   - Generated random strings from an alphabet containing: ASCII, accents, CJK, emoji, `& < > " ' ]]>`, all C0 controls, U+007F-U+009F, U+FFFE, U+2028, zero-width chars, very long runs (10k chars). Inject into every string field of a populated `CvInput` (walk the struct). For 500 seeds: export does not panic, output is valid UTF-8, contains no illegal XML 1.0 char, no `<![CDATA[`, and passes `validate.sh` (validate a sample of 25 seeds to keep CI time down; all 500 well-formedness-checked by an in-test checker).
   - Boundary inputs: empty arrays everywhere; `person` with only `name`; only `meta`; 100 + 1 experiences (cap); `eqf` 0, 9, 255; `end` before `start`; `current:true` with an `end` (To must not be emitted); duplicate skills; huge `skills` list (5000); non-http URL (`javascript:alert(1)`, `file:///etc/passwd`) dropped; email without domain dot; `lang: "EN"`; `lang: "de"` error; `schemaVersion: 2` error; `generatedAt` garbage error; unknown extra JSON keys ignored; `null` vs absent equivalence; `skills` given as strings and as `{label}` objects.
   - Idempotence/determinism (section 10) including key-order shuffle.
   - FR and EN outputs of the same data differ only where input strings and the i18n constants differ (assert the set of element paths is identical).
5. **Input validation** (`tests/input_validation.rs`): `validateCvInput` returns the right codes and JSON paths for a bad fixture; `ok:true` for goldens.
6. **Wasm smoke** (done on the TS side, section 13): vitest loads the real `.wasm` via `initSync` and re-checks the golden bytes, which also detects a stale committed binary.
7. **Size/regression check** (shell, in `package.json` `build:wasm`'s follow-up or a README note): print wasm byte size; fail > 400 KB.

Add to section 12 item 3 the negative controls seen in review so each rule is proven: email `a@b` (no dot) must produce `email_invalid` and no `Email`; `eqf` 0 must omit `Level/Code`; a website URL containing a space must be skipped with `url_invalid`.

Definition of done for R: `cargo test --locked` green (including XSD validation with xmllint present), the real wasm size after wasm-opt and one timed export of the full fixture are recorded in the crate README, `npm run build:wasm` produces `src/lib/semcv/europass/wasm/{europass_xml.js,.d.ts,europass_xml_bg.wasm,_bg.wasm.d.ts}`, the goldens validate.

## 13. TypeScript integration plan (implementer T)

Files (all new; nothing existing is modified except `package.json` scripts, `.gitignore`, and the place the button is added):

```
src/lib/semcv/europass/
  wasm/                         (generated by build:wasm, committed; do not hand-edit)
  cvInput.ts                    CvInput types (copy of cv-input.md section 4 + the extra optional keys of section 3 above) and buildCvInput()
  cvInput.test.ts               builds from the real static/cv TTL with oxigraph/node.js, snapshot of key facts
  europassXml.ts                wasm loader + exportEuropassXml(input, opts) wrappers (browser) 
  europassXml.test.ts           loads wasm with initSync (build.md snippet), compares with __fixtures__/golden, checks error prefix parsing
  __fixtures__/                 copies of the Rust goldens (cv_full_en.json/.xml, ...); a vitest test asserts the copies equal crates/.../tests/fixtures (guard against drift)
  europassDownload.ts           download helper: Blob + anchor click
src/lib/components/EuropassDownload.svelte   (Svelte 5 runes button)
```

### 13.0 Exact TS types (shared contract with the wasm API)
```ts
export interface ExportOptions {            // exactly the section 2.2 keys, all optional
  datePrecision?: 'year' | 'month' | 'day';
  includeEsco?: boolean;
  summaryPlacement?: 'headline' | 'drop';
  includeProjects?: boolean;
  includeSkills?: boolean;
  maxExperiences?: number | null;
  maxActivitiesChars?: number;
  pretty?: boolean;
  schemaLocationHint?: boolean;
  creationDate?: string | null;
}
export interface Issue { code: string; path: string; message: string }
export interface ReportResult { xml: string; warnings: Issue[] }
export interface ValidationResult { ok: boolean; errors: Issue[]; warnings: Issue[] }
```
The websites contract is `person.websites: { url: string; label?: string }[]` only; there is no `use` field (`cv-input.md` 3.1 is edited accordingly).

Data fixes owned by T: correct `20014-12-01` to `2014-12-01` in `static/cv/projects/author-AcmeCos.ttl` and `static/cv/projects.ttl`; in `buildCvInput` drop (and list in `meta.omitted`) any date not matching `^\d{4}(-\d{2}(-\d{2})?)?$`; `cvInput.test.ts` asserts every emitted date matches that regex.

### 13.1 `buildCvInput(store, personNode, lang, variant?, meta?) : Promise<CvInput>`
Orchestration copies `generateATS_CV_Typst` (`pdfprint/ATSCV_typst.ts`): same adapters, same sort (`compareExperience`), so PDF and XML always describe the same CV. Per field, source is `cv-input.md` section 3:
1. Existing adapters: `getPerson`, `listPersonLangs`, `listOrgRoles`, `listCredentials`, `listProjects`, `listSkills`, `listEducations`.
2. **New small queries** (T adds them under `adapters/`, each with a vitest against `oxigraph/node.js`): `getPersonContact` (`schema:email` with `mailto:` stripped, `schema:url` + the `https://example.org/cv` constant), `listCvSkills(lang)` (name, category local name, uses count, ESCO link, matching `skillsExportQuery` in `europassExport.ts`), `getRoleEsco(role)` and `getEducationEqf(educ)` (reuse the queries of `europassExport.ts`; do not break its exports).
3. Language codes: the fixed fallback table French->`fr`, English->`en`; "Native"/"Langue maternelle" -> `motherTongue: true`; unknown language -> no `code`.
4. Split bullets: lines starting with `-` or `•` -> `bullets[]`, as `buildOrganizationRole` does.
5. Drop empty `positions` entries that have no title, no description, no bullets, no projects (Telco Corp case); keep their `projects` by emitting a position with `title: ""`.
6. Strip `null` values (`JSON.stringify` skips `undefined`; convert `null` end to omitted or keep, the crate accepts both).
7. `meta.generatedAt = new Date().toISOString()` set by the caller (not the crate); trim milliseconds (`.replace(/\.\d{3}Z$/, 'Z')`) for readable output.
8. Gaps listed in `cv-input.md` 3.9 go into `meta.omitted`/`meta.untagged` (logged in dev, never shown to users).

### 13.2 Loader (`europassXml.ts`)
Exactly the `build.md` pattern: `import wasmUrl from './wasm/europass_xml_bg.wasm?url'`, `init({ module_or_path: wasmUrl })` memoised, guarded with `typeof window !== 'undefined'` (SSG prerender runs in node and must not load it; the download button is client-only anyway). API:
```ts
export async function exportEuropassXml(input: CvInput, options?: ExportOptions): Promise<string>
export async function exportEuropassXmlReport(input: CvInput, options?: ExportOptions): Promise<ReportResult>
export class EuropassExportError extends Error { code: string }   // parsed from the "[code] message" prefix
```
Tests use `initSync` per `build.md` Gotcha 3 (`process.cwd()`-based path, no `import.meta.url`).

### 13.3 Svelte download button
`EuropassDownload.svelte` (runes: `$state` for `busy`/`error`/`warnings`, `$props` for `lang`, `variant`): a `<button>` "Europass XML" with a small language switch following the page language; on click: `buildCvInput` -> `exportEuropassXml` -> `Blob([xml], {type:'application/xml;charset=utf-8'})` -> `<a download="alex-martin-europass-{lang}.xml">`. Shows `warnings.length` in a `title` tooltip, an inline error on failure, `aria-busy` while running. Raw CSS in the component `<style>`, matching the neighbouring PDF button style. Placed next to the existing PDF download in `src/routes/cv/+page.svelte` (T inspects how that page exposes the store and `lang`; the same component is reusable on `/cv/[variant]`). The button must not appear in the printed/ATS view.
A second link may point to the already published `/cv/europass.ttl` (ELM export) with the label "Europass ELM (Turtle)"; out of scope for the XML contract.

### 13.4 Scripts, repo hygiene (from build.md, restated so T and R agree)
- `package.json` scripts: `build:wasm` and `test:rust` exactly as in `build.md`; plus `"validate:xml": "bash crates/europass-xml/scripts/validate.sh"`. Not added to `build`/`postinstall` (CI has no Rust).
- `.gitignore`: add `crates/**/target/` and `crates/**/pkg/`.
- Commit: `crates/europass-xml/{Cargo.toml,Cargo.lock,src,tests,schema,scripts,docs}` and `src/lib/semcv/europass/wasm/*` (delete the wasm-pack generated `.gitignore`). Nothing is committed by these implementers; the owner commits.

### 13.5 TS test plan
- `europassXml.test.ts`: golden parity with the Rust fixtures (byte equal), error code parsing (`invalid_lang`), `validateCvInput` shape, `europassXmlInfo`.
- `cvInput.test.ts`: from `static/cv/*.ttl` through `oxigraph/node.js`: EN and FR produce the same experience ids in the same order; `person.email` has no `mailto:`; no `address`/`phones`/`birthDate`/`gender` keys (privacy contract); all experiences have `start`; every `eqf` in 1..8; JSON passes `validateCvInput` with `ok: true` (warnings allowed).
- End-to-end (node, no browser): TTL -> `buildCvInput` -> wasm -> string; write to a temp file and run `scripts/validate.sh` through `child_process.spawnSync`, skipped if `xmllint` is absent. This is the only test that proves "real data produces a valid Europass XML".
- `npm run check` (svelte-check) clean for the new files.

## 14. Work split and acceptance

| Owner | Delivers | Acceptance |
|---|---|---|
| R | crate per sections 1-12 | section 12 "Definition of done"; the public API strings and JSON shapes exactly as sections 2-3; `cargo build --offline` works |
| T | section 13 | vitest green on a build of R's wasm; EN and FR files for the real CV validate; button visible on `/cv` |
| Both | goldens | R owns `crates/europass-xml/tests/fixtures/`; T copies; the drift test in 13 keeps them in sync |

Order of work: R writes `input.rs`, `error.rs` and the `export()` skeleton + `info`/`validate` first and pushes the API (T can then code against `.d.ts` while R fills in the mapping). T starts with `cvInput.ts` + new adapters, which need no wasm.

Open questions deliberately closed here (do not reopen without editing this file): a single `Headline` merges headline + summary (4.2); projects are one `Achievement` each (4.7); ESCO is dropped by default (4.8); mother tongue/foreign codes are validated against compiled-in sets (4.6); no quick-xml (0).
Known limits to document in the README of the crate, not to fix: no Europass-side import test was possible (`xsd-map.md` 5); the ISCED/NACE schemas used for validation are local stand-ins (`schema/imported/README.md`), irrelevant here because the crate never emits `Field` or `Sector`.

## 15. Cost estimate (the user's question: "what will be the cost to generate a nice PDF + Europass XML CV?")

Not a monetary cost: runtime and size cost for the visitor, plus development effort.
- All figures below are estimates, not measurements (the Typst size is copied from `build.md`; effort is judgement; `npm run test:rust` must be added by T/R per 13.4). The Definition of Done in section 12 records the measured wasm size and export time.
- **Europass XML** (this crate): about 100-250 KB wasm (probe 96 KB with serde_json + quick-xml; we drop quick-xml), about 40-90 KB gzipped, loaded only on click. Export time: expected well under 10 ms (to be measured) for this CV (a few hundred small nodes). No server, no network at export time.
- **PDF**: already shipped via Typst WASM (about 28 MB of wasm per `build.md`), the heavy part; the XML feature adds under 1% on top. "Nicer" PDF is a Typst template effort, independent from this crate; the XML and the PDF share the same data builder (`buildCvInput` mirrors `generateATS_CV_Typst`), so there is no data duplication beyond the new `listCvSkills`, `getPersonContact` queries.
- **Development effort** (rough, for one engineer each side): R about 1.5-2.5 days (model+mapping 1 d, tests and goldens 0.5-1 d, wasm build and size 0.5 d); T about 1-1.5 days (builder + 4 small queries 0.5-1 d, loader/button/tests 0.5 d). Assumes no data cleanup; the `cv-input.md` 3.9 data fixes (names split, language tags, CEFR levels) are an owner decision outside this estimate.
- **Maintenance**: rebuild the wasm when `crates/` or the schema changes (`npm run build:wasm`); the XSD is frozen (v3.4.1 is archived), so no schema drift.
- **Risk**: Europass's current website may no longer import the legacy XML (`xsd-map.md` 5); XSD validity is guaranteed by tests, import into the live editor is not.
