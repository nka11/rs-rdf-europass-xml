# CV input contract for the `europass-xml` crate

Status: proposal. Describes the language-resolved JSON document that the Rust/WASM crate consumes
to produce a Europass XML CV (Europass XML Schema v3.4.1, root `SkillsPassport`).

Sources read for this contract: `src/lib/semcv/models.ts`, all of `src/lib/semcv/adapters/`,
`src/lib/semcv/pdfprint/typstMarkup.ts` and `ATSCV_typst.ts` (the existing consumer),
`src/routes/cv/+page.svelte`, `src/lib/components/schemaorgcv/*`, `src/lib/semcv/europass/europassExport.ts`,
and `static/cv/{schemaorg,skills,esco-alignment,ontology}.ttl`, `orgroles/*.ttl`, `projects/*.ttl`.
XSD element names quoted below were read from `schema/v3.4.1/*.xsd`.

## 1. Design principles

1. **The crate never sees RDF or languages.** The TypeScript side resolves everything for one locale
   (`lang` = `"en"` or `"fr"`) and hands over plain strings. One call = one locale = one XML document
   (`SkillsPassport/@locale` is a required attribute in the XSD). A bilingual CV is two calls.
2. **Omit, never fabricate.** A field that has no value in the data is absent from the JSON (key missing or
   `null`). The crate must not emit placeholders, default countries, guessed CEF levels, etc. Every XSD
   element used by the crate is `minOccurs="0"`, except `DocumentInfo/DocumentType` and
   `DocumentInfo/CreationDate` (and the date `year` attribute), so omission is always schema-valid.
3. **Dates are ISO strings** (`YYYY-MM-DD`, as stored in the TTL as `xsd:date`). The crate converts to the
   `year`/`month`/`day` attributes of `DateType` (only `year` is required).
4. **Text is plain text.** Multi-line descriptions keep `\n`. Bullet lists are pre-split into arrays so the
   crate does not have to parse Markdown-ish prefixes (see 3.3).
5. **Producer is a new TS module**, proposed as `src/lib/semcv/europass/cvInput.ts`, exporting
   `buildCvInput(personNode, lang, variant?) : CvInput` and the matching TS interface. It follows the same
   orchestration as `generateATS_CV_Typst` (`pdfprint/ATSCV_typst.ts`), which is the reference for ordering and for
   which adapters are called. A vitest test can build it from the real `static/cv` files (import
   `oxigraph/node.js` in tests).

## 2. How `lang` selects bilingual fields

All existing adapters use the same rule, inlined in SPARQL as
`FILTER(LANG(?x) = "<lang>" || LANG(?x) = "")`:

- a literal tagged with the requested language is kept (`"Founder"@en` for `en`);
- an **untagged** literal is kept for every language (examples: person `schema:name`, education names, skill
  labels such as `"IAM"`, project names such as `"Site perso example.org"`);
- literals tagged with another language are dropped.

Consequences the producer must handle:

- **Duplicates.** If a property has both a tagged and an untagged value, both rows come back. De-duplicate and
  prefer the tagged value.
- **No cross-language fallback.** If a field only exists in `fr` and `en` is requested, the adapter returns
  nothing for it (or, for `listProjects`, drops the whole project because `?projectName` is required and
  filtered). The contract keeps that behaviour: the field is omitted and the producer records the gap in
  `meta.omitted` (see 3.9) instead of silently mixing languages. A `--fallback-lang` option may be added later;
  it is off by default.
- **Variants.** `variant` (a `cv:variant-*` IRI) changes `jobTitle`, role names, credential names and some
  descriptions through `schema:DefinedTerm` blank nodes with `schema:inDefinedTermSet` (see `getPerson`,
  `listOrgRoles`, `listCredentials`). The default export uses `variant = undefined` (the neutral CV). The same
  parameter can be passed through unchanged; `meta.variant` records it.

Which data is bilingual (verified in the TTL):

| Data | Bilingual? |
| --- | --- |
| `schema:jobTitle`, person `schema:description` | yes (`@en`/`@fr`) |
| role `schema:roleName`, role `schema:description` | yes, except a few roles that are untagged (e.g. `unicaenbut` roleName "Université de Caen") |
| credential `schema:name` | partly: new files (dataspace, telco...) are tagged, old ones in `schemaorg.ttl` are untagged French ("Architecte Technique Senior") |
| project `schema:name`/`description`/`roleName` | mostly yes; a few are untagged (`author-example`, `contrib-*`, `teacher-*`) |
| language names and levels (`schema:knowsLanguage`) | yes |
| skill `elm:label` | mostly yes; many are untagged (identical in both locales) |
| employer / organisation name | **no**, untagged (`"Holiday Resorts SA"`) |
| education `schema:name`, `schema:educationalLevel`, school name, city | **no**, untagged French only |
| skill category (`skos:prefLabel` of `nkonto:*Skill`) | **no**, untagged French ("Méthodologique"); only the scheme title is `@fr` |
| person name, email, url | not language dependent |

## 3. JSON shape

Top level (`CvInput`):

```
{
  "schemaVersion": 1,
  "lang": "en",
  "person": { ... },
  "languages": [ ... ],
  "experiences": [ ... ],
  "education": [ ... ],
  "skills": [ ... ],
  "projects": [ ... ],
  "meta": { ... }
}
```

Unknown keys must be ignored by the crate (forward compatibility); `schemaVersion` is checked.

### 3.1 `person`

| Field | Type | Source (function / query) | Notes |
| --- | --- | --- | --- |
| `name` | string | `getPerson(node, lang, variant)` -> `IPersonDetails.name` | Single literal "Alex Martin". Europass wants `FirstName` + `Surname` (`PersonName`). The data has no split. Proposed rule in the producer: expose `givenName`/`familyName` only if the TTL gains `schema:givenName`/`schema:familyName` (recommended data addition). Until then send `name` only and the crate splits on the **last** space (`FirstName`="Alex", `Surname`="Martin") and documents that heuristic. |
| `headline` | string | `getPerson(...).jobTitle` | Bilingual (`@en`/`@fr`); with a variant, from the `DefinedTerm` name. Maps to `Headline/Description`. |
| `summary` | string | `getPerson(...).description` | Bilingual, multi-line, contains " - " bullets. Kept as is. The crate may place it in `Headline` or in an `Achievement`; see crate design. |
| `email` | string | **No adapter returns it.** `IPersonDetails.email` exists in `models.ts` but the `getPerson` SELECT only has `?name ?jobTitle ?description`, so it is always undefined. Needs a small new query on `schema:email`. | The TTL value is `"mailto:alex.martin@example.org"`: strip the `mailto:` prefix in the producer. |
| `websites` | `[{ url, label? }]` | **No adapter.** New query on `schema:url` of the person (`<https://github.com/example-alex>`) plus the site constant `https://example.org/cv` already used in `buildHeader` (typstMarkup.ts). | There is no `use` field (the crate derives `Website/Use` from `label`; see DESIGN 4.2). Europass restricts `Website/Use/Code` to an enumeration, so the producer sends a free `label` only (`"GitHub"`, `"CV"`), and the crate maps to a code only if it is certain. |
| `country` | string or null | **No adapter.** Only `schema:birthPlace/schema:address/schema:addressCountry = "France"` exists. | That is a **birth** country, not a residence address. Do **not** use it as `Address/Contact/Country`. Send it as `birthCountry` if at all (Europass has no birth place field in `Demographics`; it only has `Birthdate`, `Gender`, `NationalityList`), so effectively it is not emitted. |

Missing and **required by no one, but usually expected** by Europass consumers (all optional in the XSD):

| Europass element | In our data? | Handling |
| --- | --- | --- |
| `ContactInfo/Address` (`AddressLine`, `PostalCode`, `Municipality`, `Country`) | no | omit; never infer from the birth country or employers |
| `ContactInfo/TelephoneList` | no | omit |
| `Demographics/Birthdate` | no | omit (not even a year) |
| `Demographics/Gender` | no | omit |
| `Demographics/NationalityList` | no | omit; do not derive from birth country |
| `Photo`, `Signature` | no | omit |
| `PersonName/Title` | no | omit |

The JSON schema reserves optional keys for these (`person.address`, `person.phones`, `person.birthDate`,
`person.gender`, `person.nationalities`) so the crate can serve other people's data or a future enriched
TTL, but the producer for this site never sets them. Privacy note: a published CV should not carry a phone,
address or birth date unless the owner adds them deliberately; the omission is intended and should not be
"fixed" by defaults.

### 3.2 `languages`

Source: `listPersonLangs(personNode, lang)` -> `IPersonLangDetails { lang: Literal, level: Literal }`
(both literals are bilingual: "French"/"Français", "Native"/"Langue maternelle").

```
{ "label": "French", "level": "Native", "code": "fr", "motherTongue": true }
```

- `label`, `level`: straight from the adapter (already in the requested language).
- `code` (ISO 639-1) and `motherTongue`: **not in the data.** The adapter returns only display strings.
  Recommended data fix: add `schema:inLanguage "fr"` (or `schema:identifier`) and an explicit marker for the
  mother tongue on each `schema:Language` node in `schemaorg.ttl`, then extend the query. Fallback until then:
  the producer maps the **English label** through a small fixed table (French->fr, English->en) and treats the
  level strings "Native"/"Langue maternelle" as `motherTongue`. Any language outside the table is sent without
  `code` and the crate omits it (Europass `Code` for foreign languages is a closed list).
- CEF levels (A1..C2 per skill: `Listening`, `Reading`, `SpokenInteraction`, `SpokenProduction`, `Writing`)
  are **missing**: "Bilingual" is not a CEF level. Do not map it to C2. English is therefore emitted as a
  `ForeignLanguage` with `Description` only and no `ProficiencyLevel` (valid: `ProficiencyLevel` is
  `minOccurs="0"`), and the textual level ("Bilingual") is not emitted into a CEF field. A `cef: {listening, reading, ...}` object is reserved for when the owner supplies it.

### 3.3 `experiences`

Order: `listOrgRoles(...).sort(compareExperience)` (`semantic_cv_store.ts`: start date descending, then end
date descending). The Typst consumer does exactly this; keep it.

One entry per `IOrganizationRole` (subject = person node, attribute `"schema:hasOccupation"`, as in
`ATSCV_typst.ts`):

| Field | Type | Source | Notes |
| --- | --- | --- | --- |
| `id` | string | `IOrganizationRole.identifier` | e.g. `"dataspace"`, `"hr-ate"`; stable key, also useful for ids in XML comments/tests. Required by the SPARQL (`schema:identifier`), so always present. |
| `role` | string | `roleName` | Bilingual (variant aware). Maps to `Position/Label`. |
| `organization` | string | `employer` | From `nkonto:withinOrganization/schema:name`, **not bilingual**. Maps to `Employer/Name`. |
| `organizationDescription` | string, optional | not in any adapter | Exists only for Dataspace-Y (`schema:description@en/fr`); skip. |
| `start` | string | `startDate` | ISO date, always present (query requires it). |
| `end` | string or null | `endDate` | Optional in the query. **Open ended = absent**: Dataspace-Y, Tractio Labs, Founder Studio, Novembre Studio... The crate then writes `Period/Current = true` and no `To`. |
| `current` | boolean | derived: `end == null` | Producer sets it so the crate does not guess. Caveat: a few finished roles lack `endDate`; the data is trusted as is. |
| `url` | string, optional | `IOrganizationRole.url` | `schema:url`; employer website candidate (`Employer/ContactInfo/Website`). |
| `place` | string, optional | `IOrganizationRole.place` | The model has it but the SPARQL never selects it, so it is always absent. Skip. |
| `description` | string, optional | `IOrganizationRole.description` | Role-level text. Only some roles have one (Dataspace-Y, BetaSystems). On most others it is commented out in `schemaorg.ttl`, and the substance is in credentials and projects. |
| `bullets` | string[] | split of `description` | Lines starting with `-` or `•` after trimming become bullets, the lead-in line(s) stay in `description`. Mirrors the line splitting in `buildOrganizationRole`. |
| `escoOccupation` | string, optional | **No adapter.** `schema:occupationalCategory` in `static/cv/esco-alignment.ttl` (also read by `experienceExportQuery` in `europass/europassExport.ts`) | Full IRI `http://data.europa.eu/esco/occupation/<uuid>`. Present for the roles aligned in `esco-alignment.ttl` (including `exp4` BetaSystems); absent otherwise, and absent for the credential-less teaching roles' sub-entries. Needs a tiny query or reuse of `experienceExportQuery`. Note below: the XSD has **no** ESCO slot. |
| `skills` | `[{label}]` | `listSkills(role, "schema:skills", lang)` -> `skillLabel` | Same call as `buildOrganizationRole`. Many roles have none. |
| `positions` | array | `listCredentials(role, "schema:hasCredential", lang, variant)` | Sub-roles/stints (see below). Same call as `buildOrganizationRole`. |

`positions[]` (from `ICredentialDetails`): `{ title, start, end?, description?, bullets[], projects[] }`

- `title` <- `credentialName`, `start` <- `credentialStartDate`, `end` <- `credentialEndDate`,
  `description` <- `credentialDescription`. Names are partly untagged French in `schemaorg.ttl` (Holiday Resorts
  credentials), so the EN export shows "Architecte Technique Senior". That is a data gap, reported in
  `meta.omitted`/`meta.untagged`, not patched in code.
- Holiday Resorts (`hr-ate`) demonstrates why: its 4 credentials (Architecte Technique, Senior, Lead DevOps,
  Lead DevSecOps) have descriptions only inside variant `DefinedTerm`s, so the **default** (no variant) export
  yields titles and dates but no description text. Projects (`hr-road99`, `hr-SSO`, ...) carry the real text.
- `projects[]` <- `listProjects(credential, "schema:subjectOf", lang)` -> `IProjectDetail`:
  `{ name, role?, description?, bullets[], start?, end?, url?, skills[] }` where
  `skills` <- `listSkills(project, "schema:about", lang)`. Same as `buildProjectDetails`.
  Note `IProjectDetail` declares `url` and `roleName`, and the query selects them, so they are available.
- Optionally `impacts` <- `getProjectImpacts(project, lang)` (`IProjectImpact{type,name,description}`). Not used by
  the PDF; **not part of v1** (verbose; Europass `Activities` is one string). Reserved key `impacts`.

How the crate should use it (informative): one `WorkExperience` per `experiences[]` entry; `Period` from
`start`/`end`/`current`; `Position/Label` from `role`; `Employer/Name` from `organization`; `Activities` is a
single `xsd:string`, so the crate concatenates `description`, `bullets`, then each position/project as lines.
`escoOccupation` can only be carried in free text or dropped, see 3.8.

Caveat to flag: for `unicaenbut` the role name is the institution ("Université de Caen", untagged) and the
employer is "Campus Ifs - BUT Informatique"; the real job title is the credential "Part-time Teacher" /
"Enseignant Vacataire". This is how the data is modelled for the website too; the producer sends it as is and a
data cleanup is recommended rather than special-casing.

### 3.4 `education`

Source: `listEducations(personNode, "schema:alumniOf", lang)` -> `IEducationDetails`
(`educ, educName, educLocationName, educLocationAddress, educLevel, educStart, educEnd`). The query has **no
ORDER BY and no language filter**; the producer sorts by `end` descending (then `start`).

| Field | Source | Notes |
| --- | --- | --- |
| `title` | `educName` | "DUT Informatique", "Bac S option Technologie". Untagged French; not translated (omit-or-keep: kept, flagged in `meta.untagged`). |
| `school` | `educLocationName` | "IUT Paris XII", "Lycée George Sand". |
| `city` | `educLocationAddress` | A bare string ("Fontainebleau", "Le Mée sur Seine"); the model does not split municipality/country. Maps to `Organisation/Address/Contact/Municipality` only if the producer is sure it is a city; here it is. |
| `levelLabel` | `educLevel` | Free text ("Diplôme Universitaire de Technologie"). Maps to `Level/Label`. |
| `eqf` | **No adapter.** `elm:educationLevel eqf:<n>` in `esco-alignment.ttl` (also `educationExportQuery`) | IRI `http://data.europa.eu/snb/eqf/5` -> number 5 (DUT), 4 (Bac). The XSD `Level/Code` is the `eqf:level` enumeration from the imported EQF schema, so the crate must validate the value is 1..8. |
| `start`, `end` | `educStart`, `educEnd` | Optional. The Bac has only an end date (1996-06-30). |
| `awardingBody` | not in `listEducations` | exists as `elm:awardingBody/schema:name` in `esco-alignment.ttl`; equals `school` today, so skip in v1. |
| `field` (ISCED) | not in data | omit; never guess an ISCED-F code. |
| `skills` | `listSkills(educ, "schema:competencyRequired", lang)` | Optional, as `buildEducationDetails` uses projects only (`schema:subjectOf` -> `iut-stage`), producer may also send `projects[]` like in 3.3. |

Note the Typst version computes `projects` through `listProjects(education.educ, "schema:subjectOf", lang)`.

### 3.5 `skills`

The PDF currently prints `skillsCounts(lang)` (`adapters/skillsAdapter.ts`) as a comma list: it returns only
`skillName` and a usage `count`, ordered by count descending. It does **not** return the skill IRI, the
category or the ESCO link, because the query groups by name. For Europass a new adapter function is needed,
proposed `listCvSkills(lang)`, built from the same patterns as `skillsExportQuery` in `europass/europassExport.ts`
and `listSkills`:

```
{ "label": "IAM", "category": "Compliance", "categoryLabel": "Sécurité & Conformité",
  "uses": 12, "esco": "http://data.europa.eu/esco/skill/ab49f767-296b-47d5-af56-0b4a69515b03",
  "escoMatch": "exact" }
```

| Field | Source | Notes |
| --- | --- | --- |
| `label` | `elm:label` filtered by `lang` | `skillLabel` as in `listSkills`; bilingual or untagged. |
| `uses` | `COUNT` as in `skillsCounts` | Keeps the ordering of the PDF (most used first). |
| `category` | `skos:broader` (one of `nkonto:ManagementSkill`, `ToolSkill`, `MethodologicalSkill`, `ComplianceSkill`, `TechnicalSkill`) | Use the local name as a stable key. `parentSkillName` from `listSkills` is the label, **French only and untagged**, so `categoryLabel` is not localisable; the crate maps `category` to the Europass skill group itself. |
| `esco` | `elm:relatedESCOSkill` (exact link, 67 skills) else `skos:broadMatch` (32 skills) | IRI. `escoMatch` is `"exact"` or `"broad"`. Absent for the rest. The XSD cannot carry it (3.8). |

Suggested crate-side grouping (a design choice, not a data fact): `TechnicalSkill` and `ToolSkill` ->
`Skills/Computer`; `ManagementSkill` -> `Skills/Organisational`; `MethodologicalSkill` and `ComplianceSkill` ->
`Skills/JobRelated`; anything unknown -> `Skills/Other`. These are the `SkillSetType` children in
`EuropassLearnerInformation.xsd` (`Linguistic`, `Communication`, `Organisational`, `JobRelated`, `Computer`,
`Driving`, `Other`). `Communication` and `Driving` stay empty (no data, never invented).

### 3.6 `projects` (personal projects)

Source: `listProjects(personNode, "schema:producer", lang)` -> `IProjectDetail` (what the PDF prints in its
"projects" section through `buildProjectDetails(..., showDates=true)` and what `PersonalProjects.svelte`
lists). Entries: `author-example`, `author-AcmeCos`, `contrib-examplevillages`, `contrib-alpha-editor`,
`contrib-kittyhyper`.

```
{ "name", "role", "description", "bullets", "start", "end", "url", "skills": [labels] }
```

`skills` <- `listSkills(project, "schema:about", lang)`. Europass has no "personal projects" section; the
crate emits them as `AchievementList/Achievement` (title code `projects`, if present in
`AchievementTypesEnumeration`; the crate must check the enumeration rather than assume), keeping
`description`, `url` and dates as text. Caveat: `listProjects` drops projects whose name is tagged only with the
other language, and most of these are untagged, so it works in practice.

### 3.7 `meta`

```
"meta": {
  "generatedAt": "2026-11-02T10:00:00Z",   // becomes DocumentInfo/CreationDate (required, xsd:dateTime)
  "generator": "example.org",          // free text
  "variant": null,                          // or "cyber" etc. (slug as in VARIANT_PREFIX)
  "sourceUrl": "https://example.org/cv",
  "omitted": [ "person.email: no query", "experiences[hr-ate].positions[0].description: variant-only" ],
  "untagged": [ "organization names", "education.*", "skill categories" ]
}
```

`generatedAt` is passed in by the caller (the crate must not read the clock; it is deterministic and
WASM-friendly). `omitted` and `untagged` are advisory strings for logs/tests, not emitted into the XML.

### 3.8 What Europass cannot represent from our data

- **ESCO URIs** (occupations and skills). Searching the v3.4.1 schema (`grep -ri esco`) finds only the language
  schema; there is no element or attribute for an ESCO or ELM identifier. The JSON keeps them (`escoOccupation`,
  `esco`) because they are the semantic value of this CV, and the semantic export (`/cv/europass.ttl`) already
  carries them, but the XML can only mention them as text (for example in `Activities`) or drop them. Default:
  drop, option to append. This is a limit of the 3.4.1 format, not of the pipeline.
- Impacts, product-cycle statements, force-graph links, skill relations (`elm:relatedSkill`).
- Variants other than through the resolved strings.

### 3.9 Data gaps summary (recommended data fixes, not code fixes)

1. `schema:givenName`/`familyName` on `nka:me`.
2. `schema:inLanguage` + mother-tongue marker on `schema:Language`; optional CEF levels.
3. Language tags on education names, levels, school names, `Holiday Resorts` credentials.
4. Descriptions of the Holiday Resorts credentials in the neutral (no-variant) view.
5. Real job title for the `unicaenbut` role (currently the institution name).
6. Address/phone/birth date/gender: owner decision; default stays omitted.

## 4. TypeScript contract (informative)

```ts
export interface CvInput {
  schemaVersion: 1;
  lang: 'en' | 'fr';
  person: {
    name: string; givenName?: string; familyName?: string;
    headline?: string; summary?: string; email?: string;
    websites?: { url: string; label?: string }[];
  };
  languages: { label: string; level?: string; code?: string; motherTongue?: boolean }[];
  experiences: {
    id: string; role: string; organization: string;
    start: string; end?: string | null; current: boolean;
    url?: string; description?: string; bullets: string[];
    escoOccupation?: string; skills: string[];
    positions: {
      title: string; start?: string; end?: string | null;
      description?: string; bullets: string[];
      projects: Project[];
    }[];
  }[];
  education: {
    title: string; school?: string; city?: string; levelLabel?: string;
    eqf?: 1|2|3|4|5|6|7|8; start?: string; end?: string;
    projects?: Project[];
  }[];
  skills: { label: string; category?: string; uses?: number; esco?: string; escoMatch?: 'exact'|'broad' }[];
  projects: Project[];
  meta: { generatedAt: string; generator?: string; variant?: string | null;
          sourceUrl?: string; omitted?: string[]; untagged?: string[] };
}
export interface Project {
  name: string; role?: string; description?: string; bullets: string[];
  start?: string; end?: string | null; url?: string; skills: string[];
}
```

## 5. Example (EN, built from the real data)

Abridged to a few entries of each kind; values are what the producer yields for `lang = "en"`, no variant.
Everything shown exists in the TTL, except `person.email` (present in the TTL but not currently returned by
`getPerson`) and `languages[].code/motherTongue` (derived by the fallback table in 3.2). Note the absent
`address`, `phones`, `birthDate`, `gender`, CEF levels.

```json
{
  "schemaVersion": 1,
  "lang": "en",
  "person": {
    "name": "Alex Martin",
    "headline": "Full-Cycle Product Engineer | Turning Ideas into Impactful Products",
    "summary": "I help visionaries transform their ideas into projects to ultimately create fully functional products.\n\nWith 20+ years of experience in software and infrastructure engineering, I specialize in:\n- Design & Architecture: Requirements analysis, UML, business consulting.\n- Full-Stack Development: PHP, Java, Python, C#, JavaScript/TypeScript, React, Vue.\n- Deployment & Industrialization: Packaging, CI/CD, and managing complex IT environments, including containerized architectures and strategic migration projects.",
    "email": "alex.martin@example.org",
    "websites": [
      { "url": "https://github.com/example-alex", "label": "GitHub" },
      { "url": "https://example.org/cv", "label": "CV" }
    ]
  },
  "languages": [
    { "label": "French",  "level": "Native",    "code": "fr", "motherTongue": true },
    { "label": "English", "level": "Bilingual", "code": "en", "motherTongue": false }
  ],
  "experiences": [
    {
      "id": "dataspace",
      "role": "Enterprise Architect — Sovereign mobility data space (part-time)",
      "organization": "Dataspace-Y",
      "start": "2026-05-01",
      "end": null,
      "current": true,
      "description": "Enterprise Architect (part-time) mission within Dataspace-Y, the European sovereign data space founded by Airline One, Airports Group, Rail Operator, Carmaker and Shipping Line, covering 4 verticals: transport, mobility, tourism and logistics.\nDefinition of the enterprise architecture vision and roadmap for a federated multi-actor ecosystem, leveraging TOGAF and ArchiMate frameworks, the Gaia-X / IDSA reference architecture and the Dataspace Protocol. Compliance by design: NIS2, DORA and GDPR embedded from inception.",
      "bullets": [],
      "escoOccupation": "http://data.europa.eu/esco/occupation/2bef94db-0088-4507-982a-2ca717529adb",
      "skills": [],
      "positions": [
        {
          "title": "Enterprise Architect",
          "start": "2026-05-01",
          "end": null,
          "bullets": [],
          "projects": [
            {
              "name": "Enterprise Architecture for the sovereign mobility data space",
              "start": "2026-05-01",
              "end": null,
              "description": "Enterprise Architect (part-time) mission within Dataspace-Y, the European sovereign data space founded by Airline One, Airports Group, Rail Operator, Carmaker and Shipping Line, covering transport, mobility, tourism and logistics.",
              "bullets": [
                "Definition of the enterprise architecture vision and roadmap aligning business, data, application and infrastructure layers.",
                "Capability mapping and architecture principles for a federated multi-actor ecosystem.",
                "Design of sovereignty connectors (Dataspace Protocol, EDC) and Gaia-X self-descriptions.",
                "Compliance by design: NIS2, DORA, GDPR embedded into component design."
              ],
              "skills": ["Enterprise Architecture", "TOGAF", "ArchiMate", "Data spaces", "NIS2", "DORA"]
            }
          ]
        }
      ]
    },
    {
      "id": "telco",
      "role": "Web Application Architect",
      "organization": "Telco Corp",
      "start": "2025-11-01",
      "end": "2026-02-28",
      "current": false,
      "bullets": [],
      "escoOccupation": "http://data.europa.eu/esco/occupation/d0aa0792-4345-474b-9365-686cf4869d2e",
      "skills": [],
      "positions": [
        {
          "title": "",
          "bullets": [],
          "projects": [
            {
              "name": "Full technical audit of example.lu website",
              "role": "Senior Web Application Architect",
              "start": "2025-11-01",
              "end": "2026-02-28",
              "description": "Comprehensive technical audit of the example.lu web application on behalf of Telco Corp.",
              "bullets": [
                "Architecture review: analysis of application architecture, technology choices and scalability.",
                "Security audit: vulnerability identification, review of authentication and access management practices.",
                "Performance audit: response time analysis, rendering and content delivery optimization.",
                "Strategic recommendations and improvement roadmap."
              ],
              "skills": []
            }
          ]
        }
      ]
    },
    {
      "id": "interpret-grc",
      "role": "GRC Consultant",
      "organization": "Interpret Services",
      "start": "2024-06-01",
      "end": "2024-12-20",
      "current": false,
      "bullets": [],
      "escoOccupation": "http://data.europa.eu/esco/occupation/0ce5a9f4-e00a-4bbe-b255-3c63407167a4",
      "skills": [],
      "positions": [
        {
          "title": "GRC Consultant",
          "start": "2024-06-01",
          "end": "2024-12-20",
          "bullets": [],
          "projects": [
            {
              "name": "Cybersecurity Maturity Audit and Compliance",
              "start": "2024-06-01",
              "end": "2024-12-20",
              "description": "Comprehensive cybersecurity maturity and IS security policy audit for an SME in the interpreting sector.",
              "bullets": [
                "Risk mapping using the EBIOS RM methodology.",
                "Identification of strengths and vulnerabilities in the cyber posture.",
                "Pragmatic strategic recommendations tailored to the company's context.",
                "Actionable roadmap to achieve NIS2 and GDPR compliance."
              ],
              "skills": ["Compliance", "NIS2", "GDPR", "EBIOS RM"]
            }
          ]
        }
      ]
    },
    {
      "id": "hr-ate",
      "role": "Senior Technical Architect",
      "organization": "Holiday Resorts SA",
      "start": "2001-01-01",
      "end": "2023-09-30",
      "current": false,
      "bullets": [],
      "escoOccupation": "http://data.europa.eu/esco/occupation/e1c72b5f-4c5c-487c-a6df-e84b64a51dae",
      "skills": [],
      "positions": [
        { "title": "Lead DevSecOps", "start": "2017-06-02", "end": "2023-09-30", "bullets": [], "projects": [] },
        { "title": "Lead DevOps", "start": "2011-06-02", "end": "2017-06-01", "bullets": [], "projects": [] },
        { "title": "Architecte Technique Senior", "start": "2005-06-02", "end": "2011-06-01", "bullets": [], "projects": [] },
        { "title": "Architecte Technique", "start": "2001-06-01", "end": "2005-06-01", "bullets": [], "projects": [] }
      ]
    },
    {
      "id": "acsp-inge-exp",
      "role": "Operations Engineer",
      "organization": "BetaSystems",
      "start": "2000-01-01",
      "end": "2001-01-01",
      "current": false,
      "description": "Monitoring, deployment, and operation of critical web applications.",
      "bullets": [
        "Ensured stability and service availability.",
        "Early experience on Linux and Solaris environments, building strong operational discipline."
      ],
      "escoOccupation": "http://data.europa.eu/esco/occupation/9e2e6e1e-363b-4e1b-a673-7bc0f7343300",
      "skills": ["Java", "Linux", "Solaris"],
      "positions": []
    }
  ],
  "education": [
    {
      "title": "DUT Informatique",
      "school": "IUT Paris XII",
      "city": "Fontainebleau",
      "levelLabel": "Diplôme Universitaire de Technologie",
      "eqf": 5,
      "start": "1996-09-01",
      "end": "1998-06-30"
    },
    {
      "title": "Bac S option Technologie",
      "school": "Lycée George Sand",
      "city": "Le Mée sur Seine",
      "levelLabel": "Baccalauréat Scientifique",
      "eqf": 4,
      "end": "1996-06-30"
    }
  ],
  "skills": [
    { "label": "IAM", "category": "ComplianceSkill", "uses": 12,
      "esco": "http://data.europa.eu/esco/skill/ab49f767-296b-47d5-af56-0b4a69515b03", "escoMatch": "exact" },
    { "label": "LDAP", "category": "ComplianceSkill", "uses": 4,
      "esco": "http://data.europa.eu/esco/skill/a57a54b6-2f2e-43e4-9621-b52f4a63cb08", "escoMatch": "exact" },
    { "label": "Pedagogy", "category": "MethodologicalSkill", "uses": 6,
      "esco": "http://data.europa.eu/esco/skill/a117f32c-662b-4dd9-b739-1230dd04cb60", "escoMatch": "exact" },
    { "label": "Claude AI (Anthropic)", "category": "ToolSkill", "uses": 3,
      "esco": "http://data.europa.eu/esco/skill/e465a154-93f7-4973-9ce1-31659fe16dd2", "escoMatch": "broad" }
  ],
  "projects": [
    {
      "name": "Site perso example.org",
      "role": "Author",
      "start": "2023-10-01",
      "url": "https://example.org/",
      "description": "Pages statiques générées (SSG), portfolio et publications personnelles (Digital gardening space).\nCV sémantique modélisé dans un graphe RDF et rendu par une SPA qui addresse un store oxigraph en SPARQL.",
      "bullets": [],
      "skills": ["TypeScript", "Svelte", "Git", "GitHub"]
    },
    {
      "name": "KittyHyper - Archiver for GNU Mailman v3",
      "role": "Contributor",
      "start": "2013-06-01",
      "end": "2014-02-01",
      "url": "https://docs.mailman3.org/projects/kittyhyper/en/stable/",
      "description": "Contributions au projet kittyhyper mailman3, l'outil d'archivage des mails dans une liste de discussion mailman3.",
      "bullets": [],
      "skills": ["Python", "JavaScript", "Django", "SQL", "PostgreSQL", "Git"]
    }
  ],
  "meta": {
    "generatedAt": "2026-11-02T10:00:00Z",
    "generator": "example.org",
    "variant": null,
    "sourceUrl": "https://example.org/cv",
    "omitted": [
      "person.address, phones, birthDate, gender, nationalities: not in data",
      "languages[].cef: not in data",
      "experiences[hr-ate].positions[*].description: variant-only text",
      "experiences[telco].positions[0].title: credential has no schema:name"
    ],
    "untagged": ["organization names", "education.*", "Holiday Resorts credential titles", "skill categories"]
  }
}
```

Notes on the example: skill labels, `uses` counts and the telco/club-med lines are illustrative of the
shape (the exact ordering and counts come from the live query); descriptions, dates, EQF levels, ESCO IRIs,
role and project names are copied from the TTL. In the Telco Corp entry the credential `:architect` has no
`schema:name`, so `ICredentialDetails.credentialName` is null: the producer should then drop the empty
`positions` entry and keep only its `projects` under the role, which is what a real build will do.
