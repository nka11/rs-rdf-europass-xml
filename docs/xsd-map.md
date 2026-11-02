# Europass XML Schema v3.4.1: map for a CV generator

Source of truth: `schema/v3.4.1/*.xsd` (vendored from github.com/europass/ewa-cedefop, `services/core/src/main/resources/schema/v3.4.1/`).
Everything below was read from the XSDs, and the validity claims marked **[tested]** were checked with `scripts/validate.sh` (xmllint, offline).

## 0. Conventions that apply everywhere

* Target namespace `http://europass.cedefop.europa.eu/Europass`, `elementFormDefault="qualified"`. Use it as the **default namespace** on the root, so every element is unprefixed. Attributes are unqualified. A document without the namespace is rejected **[tested]**.
* **Every `xsd:sequence` is ordered.** Children must appear in the order listed below; optional ones can be skipped but not reordered **[tested: `Skills` before `Headline` fails]**. There is no `xsd:all` and no `xsd:choice` in the CV part of the schema.
* Almost everything is `minOccurs="0"`. The only mandatory items are: `SkillsPassport/@locale`, `DocumentInfo` and its required children, `LearnerInfo` (which may be empty), `DateType/@year`, and `Attachment` internals. So an empty `<LearnerInfo/>` validates **[tested]**.
* **No length limits exist.** `maxLength` / `minLength` appear nowhere. Free text is plain `xsd:string` (no markup, no HTML; escape `& < >`). Any length or newline conventions come from the Europass renderer, not from the XSD.
* **Wrappers.** `LabelType` = `sequence(Code?, Label?)`, both optional, both strings. Derived types keep the same two elements but narrow `Code` (an enumeration or pattern). `Label` is always free text, i.e. the human-readable (localised) string. `Code` is machine-readable. Use cases:
  * Code list with enforced values: `Title`, `Gender`, `Country`, `Nationality`, language codes, `Headline/Type`, `Achievement/Title`, telephone/website/IM `Use`, EQF `Level`.
  * Code with a pattern only: `Headline/Description/Code` and `Position/Code` (a UUID pattern, see 3).
  * Free `Label`-only is fine for most (the shipped example uses `<Description><Label>..</Label></Description>`).
* **Dates.** `DateType` is an element with **attributes only** and no content: `year` (xsd:gYear, **required**), `month` (xsd:gMonth, optional, written `--MM`), `day` (xsd:gDay, optional, written `---DD`). Example: `<From year="2020" month="--03" day="---15"/>`. Plain `month="03"` is invalid **[tested]**. You cannot give a day without a month at the schema level, but the schema does not stop it.
* **Period** (`PeriodType`, used by work/education/linguistic experience) is a sequence `From?, To?, Current?, Duration?, DurationEquivalent?`. `From`/`To` are `DateType`; `Current` is `xsd:boolean` (`true`/`false`/`1`/`0`); `Duration` is `xsd:duration` (e.g. `P2Y`); `DurationEquivalent` is a string. The schema does **not** forbid `To` together with `Current` **[tested: validates]**; emit `To` only when not current.
* **Repeatable** items are only those that say so below (`maxOccurs="unbounded"`). Singletons cannot be repeated (e.g. one `Address`, one `Email`, one `Headline`, one `Photo`).
* `IDREF`: `ReferenceTo/@idref` must match an `Attachment/@id` (an `xsd:ID`, so a valid NCName: starts with a letter or `_`, no spaces or colon) in the same file. Only use if you actually embed attachments.

## 1. Root: `SkillsPassport`

```
SkillsPassport  @locale (required)  @xsi:schemaLocation (optional, ignored by validation)
  DocumentInfo          1   required
  PrintingPreferences   0..1
  LearnerInfo           1   required
  AttachmentList        0..1
  CoverLetter           0..1   (out of scope for a CV)
```

* `locale`: `LocaleType` = pattern `[a-zA-Z]{2}(-[a-zA-Z]{3})?` (so `en`, `fr`, `fr-FRA`; NOT `fr-FR`, NOT `fr_FR`). Required, only one per file: the whole document has one locale, there is no per-element `xml:lang`. For the bilingual CV, generate **two files** (`locale="en"` and `locale="fr"`), each in one language.
* Typical root: `<SkillsPassport xmlns="http://europass.cedefop.europa.eu/Europass" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://europass.cedefop.europa.eu/Europass http://europass.cedefop.europa.eu/xml/v3.4.0/EuropassSchema.xsd" locale="en">`. The `xsi:schemaLocation` is what the shipped examples carry; it is optional for validity (our minimal docs without it validate **[tested]**).

## 2. `DocumentInfo` (DocumentInfoType)

| # | Element | Card. | Type / values |
|---|---|---|---|
| 1 | `DocumentType` | 1 | enum `ECV_ESP`, `ESP`, `ECV`, `ELP`, `ECL`. For a CV only: `ECV` |
| 2 | `Bundle` | 0..1 | `sequence(Document* : DocumentTypeType)`; the example has an empty `<Bundle/>`. Lists the documents bundled in the file. Skippable |
| 3 | `CreationDate` | 1 | `xsd:dateTime`, e.g. `2026-11-02T10:00:00Z` (timezone suffix optional by XSD) |
| 4 | `LastUpdateDate` | 1 | `xsd:dateTime` |
| 5 | `XSDVersion` | 1 | pattern exactly `V3.4` (must be this literal; `V3.5` fails **[tested]**) |
| 6 | `Generator` | 1 | `xsd:string`, free text (name of the producing tool) |
| 7 | `Comment` | 0..1 | string |
| 8 | `Copyright` | 0..1 | string, **fixed** to `© European Communities \| http://europass.cedefop.europa.eu` if present. Omit it unless you copy that literal exactly |
| 9 | `EuropassLogo` | 0..1 | boolean (render the logo on PDF) |

## 3. `LearnerInfo` (LearnerInfoType), all optional, in this order

1. `Identification` 0..1
2. `Headline` 0..1
3. `WorkExperienceList` 0..1
4. `EducationList` 0..1
5. `Skills` 0..1
6. `AchievementList` 0..1
7. `Documentation` 0..1 (IntraDocument: `ReferenceTo/@idref` repeated 0..n)

Note the element names: `WorkExperienceList` / `EducationList` (not `EducationTrainingList`; there is no training element, training goes in `Education` or `Achievement` code `courses`/`certifications`).

### 3.1 `Identification`: sequence `PersonName?, ContactInfo?, Demographics?, Photo?, Signature?`

* **PersonName**: `Title?, FirstName?, Surname?`. `Title` is a LabelType with `Code` enum `mr|ms|mrs|miss|dr`. `FirstName`/`Surname` strings (one each; put middle names in `FirstName`).
* **ContactInfo**: `Address?, Email?, TelephoneList?, WebsiteList?, InstantMessagingList?, ContactMethodList?` (each singleton wrapper).
  * `Address` → `Contact` (+ no `Use`, `maxOccurs=0`) → `AddressLine?, AddressLine2?, PostalCode?, Municipality?, Country?`. `Country` = LabelType, `Code` from `EuropassISOCountries_V1.7.0.xsd` **uppercase ISO-style** (`FR`, `BE`; lower-case `fr` fails **[tested]**; the list is the Europass-filtered one in `included/EuropassISOCountries_V1.7.0.xsd`).
  * `Email` → `Contact` only, string with pattern `[^@]+@[^\.]+\..+`; no `Use` allowed.
  * `TelephoneList` → `Telephone*` → `Contact?` (string), `Use?` (LabelType code `home|work|mobile`).
  * `WebsiteList` → `Website*` → `Contact?` (`xsd:anyURI`), `Use?` (code `personal|business|blog|portfolio`).
  * `InstantMessagingList` → `InstantMessaging*` → `Contact?`, `Use?` code `gtalk|skype|icq|msn|yahoo` (`aim` removed in v3.4.1, per change-log).
  * `ContactMethodList` → `ContactMethod*` abstract base (`Contact`: anyType, `Use`: LabelType). Not usable directly in instance docs without xsi:type; skip it.
* **Demographics**: `Birthdate?` (DateType), `Gender?` (LabelType, Code `M|F`), `NationalityList?` → `Nationality+` (1..n; LabelType, Code from `EuropassNationalities_V1.7.0.xsd`, uppercase country codes like `FR`).
* **Photo** / **Signature** (identical type): `MimeType` (enum `image/jpeg|image/pjpeg|image/png|image/x-png`, required), `Data` (`xsd:base64Binary`, required), `MetadataList?` → `Metadata+` with required attributes `key`, `value`. (`Name` is not allowed in Photo.)

### 3.2 `Headline`: `Type?, Description?`

* `Type`: LabelType; `Code` enum `preferred_job | job_applied_for | studies_applied_for | position | personal_statement`.
* `Description`: OccupationalFieldType = LabelType whose `Code` must match the **UUID-like pattern** `[a-z0-9]{8}-[a-z0-9]{4}-[a-z0-9]{4}-[a-z0-9]{4}-[a-z0-9]{12}` (a Europass-internal occupational id). A non-matching code fails **[tested]**. So **do not emit `Code`** unless you hold a real id; emit only `<Label>` (as the shipped example does).
* Singleton. Use `personal_statement` for a summary line and `position` for a title.

### 3.3 `WorkExperienceList` → `WorkExperience*` (0..n)

`WorkExperience` = ExperienceType extended: `Period?, Description?, Documentation?, Position?, Activities?, Employer?` (base elements first, then the extension, in this order).

* `Period`: see section 0. `Description`: string. `Documentation`: `ReferenceTo*`.
* `Position`: OccupationalFieldType (same UUID rule for `Code`; use `<Label>` only), the job title.
* `Activities`: one string (main activities and responsibilities; use newlines as plain text).
* `Employer`: `Name?, ContactInfo?, Sector?`. `ContactInfo` = `Address?` (as above) then `Website?` (a single, not a list; `Contact` anyURI, `Use` has only `Code` fixed to `business`). `Sector` = LabelType; `Code` is `nace:sector-level1` (a NACE section letter, e.g. `Q`; **our local stand-in accepts `[A-U]`**, the official list is unavailable, see section 7), `Label` free.
* Order within the list: Europass renders as given; conventionally reverse-chronological.

### 3.4 `EducationList` → `Education*` (0..n)

Order: `Period?, Description?, Documentation?, Title?, Activities?, Organisation?, Level?, Field?`.

* `Title`: string (qualification title). `Activities`: string.
* `Organisation`: `Name?, ContactInfo?` (same OrganisationalContactInfo: `Address?`, `Website?`).
* `Level`: LabelType, `Code` = `eqf:level` enum **`1`..`8`** **[tested: 9 fails]**. `Label` free (e.g. `EQF 7`).
* `Field`: LabelType, `Code` = `isced:field-level2` (two-digit ISCED-97 field code, e.g. `21`; **our stand-in enforces `[0-9]{2}`**, official list unavailable), `Label` free.

### 3.5 `Skills` (SkillSetType), all optional, in this order

`Linguistic, Communication, Organisational, JobRelated, Computer, Driving, Other`

* **Linguistic**: `MotherTongueList?` then `ForeignLanguageList?`.
  * `MotherTongueList` → `MotherTongue*` → `Description?` (LabelType; `Code` = `motherCode`: a lower-case ISO 639-1 code from the Europass list, e.g. `fr`, `en`, includes some sign languages), `Documentation?`.
  * `ForeignLanguageList` → `ForeignLanguage*` → `Description?` (LabelType; `Code` = `foreignCode`, lower-case ISO 639-1, no dead/sign languages), `ProficiencyLevel?`, `AcquiredDuring?`, `VerifiedBy?`, `Documentation?`.
  * `ProficiencyLevel` (CEFR self-assessment): `Listening?, Reading?, SpokenInteraction?, SpokenProduction?, Writing?`, each pattern `([A-C][12])?` (`A1 A2 B1 B2 C1 C2`, or empty string). Order is fixed as listed. There is no single "overall level"; set all five.
  * `AcquiredDuring` → `Experience*` → `Period?, Description?, Documentation?, Area?` with `Area` LabelType code `studying_training_language | work_language | living_traveling_language | mediating_groups_language`.
  * `VerifiedBy` → `Certificate*` → `Title?, AwardingBody?, Date?` (DateType), `Level?` (CEFR pattern as above), e.g. DELF C1.
* **Communication / Organisational / JobRelated / Other** (GenericSkillType): `Description?` (one string), `Documentation?`. Each is a singleton with one text blob: put the whole paragraph (newlines allowed) in it.
* **Computer**: `Description?` (string), `ProficiencyLevel?`, `VerifiedBy?`, `Documentation?`. `ProficiencyLevel` = `Information?, Communication?, ContentCreation?, Safety?, ProblemSolving?` each enum `A|B|C` (A basic, B independent, C proficient). `VerifiedBy` → `Certificate*` with only `Title?`.
* **Driving**: `Description?` → `Licence*` (enum of EU categories: `A, A1, A2, AM, B, ...`; from the imported EUDriving schema, v3.0 copy), `Documentation?`.

### 3.6 `AchievementList` → `Achievement*`

`Title?, Description?, Documentation?`. `Title` is a LabelType with `Code` enum `honors_awards | presentations | publications | projects | citations | memberships | conferences | seminars | workshops | references | signature_equivalent | courses | certifications`; `Label` is the visible heading. Description is one string. Use this for publications, projects, certifications, courses, talks, plus `references`.

### 3.7 `Documentation` and `AttachmentList`

* `Documentation` (IntraDocumentDocumentationType) appears in LearnerInfo, Experience, Education, Skills, Achievement: `ReferenceTo*` with required attribute `idref` (IDREF to an `Attachment/@id`).
* `AttachmentList` (sibling of `LearnerInfo`, after it) → `Attachment*`: attribute `id` (xsd:ID, required); children `Name?, MimeType, Data, MetadataList?, Description?` (`MimeType` enum: `image/jpeg, image/pjpeg, image/png, image/x-png, application/pdf`; `Data` base64). A CV need not use it.
* (`GenericDocumentationType` with `Heading/InterDocument/IntraDocument/ExtraDocument` exists in the XSD but is not wired into any CV element.)

## 4. `PrintingPreferences` (optional, skip unless you want to steer the Europass renderer)

`Document+` (attribute `type` required: `ECV|ELP|ECL`) → `Field*` with attributes `name` (required, from a long `FieldNames` list, **invalid names fail**, e.g. the dotted path `LearnerInfo.Education[0].Organisation.ContactInfo.Address` is rejected, which is why two of the upstream examples fail), `show`, `format` (date formats such as `text/short`, `numeric/long`), `order` (enum of fixed strings such as `FirstName Surname`), `position`, `justify`, `enableName`, `pageBreaks`. Recommendation: omit it.

## 5. What the official tooling expects of a hand-made file

Verified from the schema and example files, not from running Europass software:

* An XML declaration is not required by the XSD, but all shipped examples start with `<?xml version="1.0" encoding="UTF-8"?>`. Emit UTF-8, with the declaration.
* `DocumentInfo` with a `DocumentType` of `ECV`, both dates, `XSDVersion` = `V3.4` and a `Generator` is mandatory. Without `DocumentInfo` the file is invalid **[tested]**. `Bundle`, `Comment`, `Copyright` can be skipped; the examples show `Bundle` empty and `Copyright` set to the fixed literal.
* The root has **no** `version` attribute; the version lives in `DocumentInfo/XSDVersion` only (a `V3.5` value fails).
* The shipped examples use `xsi:schemaLocation` pointing at the legacy host; harmless.
* Not verified (no way to run the upstream importer here): whether the current Europass website still imports v3.4 XML. Europass moved to a new platform (ELM-based JSON/credentials) in 2020; the legacy XML schema is archived in the ewa-cedefop repo. Treat the XSD validity as the contract, and the "opens in Europass editor" claim as untested.
* Dates inside text: `DateType` is attribute-based, never `2020-03-15` text. `Birthdate` is also a `DateType`.

## 6. Mapping hints for this site's data model (non-normative)

| CV concept | XML target |
|---|---|
| name | `Identification/PersonName` (FirstName, Surname) |
| title / tagline | `Headline/Type[Code=position]` + `Headline/Description/Label` |
| contact | `ContactInfo`: `Email/Contact`, `TelephoneList`, `WebsiteList/Website` (code `personal|portfolio|blog|business`), `Address/Contact` |
| roles | `WorkExperience` (`Position/Label`, `Employer/Name`, `Activities`, `Period`) |
| diplomas | `Education` (`Title`, `Organisation/Name`, `Level` 1-8, `Period`) |
| languages | `MotherTongue` + `ForeignLanguage` with CEFR five-skill levels |
| tech skills | `Computer/Description` (text) or `JobRelated/Description`; there is no per-skill structure |
| projects, publications, certifications, trainings | `Achievement` with `Title/Code` = `projects|publications|certifications|courses` |

## 7. Offline validation

`scripts/validate.sh <file.xml>` exits 0 only when valid. The XSDs point at `https://europass.cedefop.europa.eu/xml/{included,imported}/...` which no longer resolves, so the script writes a temporary XML catalog (via `XML_CATALOG_FILES`, `xmllint --nonet`) that redirects:

* `.../xml/included/*` → `schema/v3.4.1/included/*` (vendored, authentic).
* `.../imported/EQF_08_V1.0.0.xsd` → `schema/imported/EQF_08_V1.0.xsd` (authentic, from ewa-cedefop `schema/v3.0`).
* `.../imported/EUDrivingLicence_V1.1.0.xsd` → `schema/imported/EUDrivinglicence.xsd` (authentic but the v3.0 copy).
* `.../imported/ISCED97_V1.0.0.xsd`, `NACE_COM_V1.0.0.xsd` → `schema/imported/stubs/` (**NOT authentic**: those two files exist in no version directory of the repo and the host no longer serves them; they declare only `isced:field-level2` / `nace:sector-level1` with the loose patterns `[0-9]{2}` and `[A-U]`). A code the official lists reject could therefore pass here; use real ISCED-97 field and NACE section codes.

The pristine `schema/v3.4.1/` files are untouched. Result with this setup: `Europass_CV_V3.4.0_Example.xml` and `Europass_LP_V.3.4.0_Example.xml` validate. The other three upstream examples fail because of upstream data defects, not our setup: `Code` `12332` in a UUID-pattern field (CL, CVwithCL, CV_ESP_CL_LP), an invalid `Information` value `Br` (CVwithCL), and an invalid PrintingPreferences `Field name` (CV_ESP_CL_LP).
