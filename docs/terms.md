# Terms

Words used in this crate, in its documentation and in the CV database project that calls it
(`eona-x-europass-cv-db` on GitLab). Entries are in alphabetical order.

**attachment.xml**
The name under which the Europass editor embeds the Candidate XML in the PDFs it exports. Our PDFs use the same
name so that they can be imported back. See *Typst*.

**Candidate XML**
The XML document the current Europass editor (europass.europa.eu) reads and writes. Root element `Candidate`,
default namespace `http://www.europass.eu/1.0`. It is built on HR-XML 3 and OAGIS 9, with EURES extensions. There is
no official schema; see *Candidate XSD*. This crate writes this format and no other.

**Candidate XSD**
An unofficial XML Schema for the Candidate XML, reverse-engineered from the editor's exports by J0ker98
(<https://github.com/J0ker98/europass-cv>, CC-BY-4.0). Used here only to check structure. A file that validates is
well formed and follows the observed structure; it is not proof that the editor accepts it.

**CEFR (CEF)**
Common European Framework of Reference for Languages. Levels A1 to C2 for listening, reading, spoken interaction,
spoken production and writing. In the CV JSON these are the `cef` fields; in the XML they are
`eures:CompetencyDimension` entries.

**CountryDialing**
The international calling code of a telephone number. Mandatory in the Candidate XML: the editor rejects the whole
file if a telephone has none. The crate takes it from a `+nn` or `00nn` prefix or from the address country, and
skips the phone (`phone_prefix_unknown`) when it cannot tell.

**CV JSON**
The input of this crate: one CV, already resolved to a single language, as JSON. Described in
[cv-input.md](cv-input.md). `schemaVersion` is currently 1.

**EQF (CEC in French)**
European Qualifications Framework. Eight levels. In the CV JSON, `eqf` is a number from 1 to 8; in the XML it is
`EducationLevelCode`.

**ESCO**
European Skills, Competences, Qualifications and Occupations: the EU classification of skills and occupations.
Identified by IRIs such as `http://data.europa.eu/esco/skill/...`. A skill entry with such an IRI becomes an
`ESCO_Skill` when the option `includeEsco` is on.

**ELM**
European Learning Model: the data model behind the Europass platform and its credentials. The `elm:` prefix is used
in the website's RDF for education levels and ESCO links.

**export / warning**
`export` turns a CV JSON into XML plus a list of warnings. A warning (`code`, `path`, `message`) reports something
dropped or repaired, for example `url_invalid` or `date_order`. A hard error (`[code] message`) stops the export.

**golden file**
An expected output stored in `tests/fixtures/golden/`. The tests compare the generated XML to it byte for byte.
Regenerate with `UPDATE_GOLDEN=1 cargo test --test candidate` after an intended change.

**HR-XML, OAGIS, EURES**
The standards the Candidate XML borrows from. HR-XML 3 (prefix `hr:`) and OAGIS 9 (prefix `oa:`) provide names,
dates and communication channels. EURES (prefix `eures:`) adds employment periods and competency dimensions.

**includeEsco**
Option. When true, skills that carry an ESCO IRI are written as `ESCO_Skill` with a `CompetencyID`.

**legacy XML (SkillsPassport)**
The older Europass format (XML Schema v3.4.1, root `SkillsPassport`). The editor refuses it as an obsolete version,
so it is no longer supported. Older versions of this crate wrote it.

**Options**
Settings passed with the export, as a JSON object: `datePrecision`, `includeEsco`, `summaryPlacement`,
`includeProjects`, `includeSkills`, `maxExperiences`, `maxActivitiesChars`, `pretty`, `schemaLocationHint`, `creationDate`. Unknown keys
are ignored; `format` is accepted only with the value `candidate`.

**RDF / Turtle (TTL)**
The graph data model and its text syntax. The CV database stores each CV as Turtle files. They are converted to CV
JSON before they reach this crate.

**SectionsOrder**
The list of sections the editor displays. A section that is not listed is hidden, so the crate lists exactly the
sections that have content.

**TaxonomyID**
Skill kind in the Candidate XML: `Digital_Skill` for a plain skill name, `ESCO_Skill` when an ESCO IRI is attached.

**Typst**
The typesetting tool used by the CV database to produce the PDF. Its `pdf.attach` function embeds the Candidate XML
in the PDF. Typst names an attachment after the path it is given, so the file is staged as `attachment.xml`.

**WASM**
WebAssembly. The crate is also compiled with wasm-bindgen so that a browser can export a CV without a server.
Functions: `exportEuropassXml`, `exportEuropassXmlReport`, `validateCvInput`, `europassXmlInfo`.
