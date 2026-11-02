# Candidate XML (the format the current Europass editor imports)

The Europass editor at europass.europa.eu refuses the legacy `SkillsPassport` XML ("obsolete version"). It
reads and writes a different document, `Candidate` (namespace `http://www.europass.eu/1.0`, built on HR-XML 3 and
OAGIS 9), also embedded as `attachment.xml` in the PDFs it produces.

```sh
europass-xml-cli cv.json -o cv.xml                      # Candidate (default)
europass-xml-cli cv.json --format legacy -o legacy.xml  # or {"format":"legacy"} in the options JSON
```

`Options.format` is `candidate` (default) or `legacy`, the old `SkillsPassport` XML kept for compatibility
(`--format legacy`). The Candidate output is the only one the current Europass editor accepts.

## Sources and licence

There is no official schema. The structure follows an XML exported by the editor and the reverse-engineered
reference <https://github.com/J0ker98/europass-cv> (CC-BY-4.0, by J0ker98), whose XSD is used only for validation:
`scripts/fetch-candidate-schema.sh` downloads it at a pinned commit into `schema/candidate/` (not stored here).
Validation against it proves the document is well formed and follows the observed structure, not that the editor
accepts it.

## Mapping

| CV JSON | Candidate |
| --- | --- |
| `person` name, email, websites, phones, address | `CandidatePerson/Communication` (Email, Web, Social Media, Telephone, Address) |
| social profiles (LinkedIn, Facebook, X/Twitter, Instagram, YouTube, Pinterest, Tumblr) | `ChannelCode Social Media` with `UseCode`; other URLs are `Web` |
| phones | `CountryDialing` is mandatory; taken from `+nn`/`00nn` or from the address country. A phone without a determinable prefix is skipped with `phone_prefix_unknown` |
| `person.birthDate`, `gender`, `nationalities` | `hr:BirthDate`, `GenderCode`, `NationalityCode` (`EL`/`UK` become `gr`/`gb`) |
| mother tongues | `PrimaryLanguageCode` (`NORMAL` + ISO 639-2, else `FREE_TEXT`) |
| `headline`, `summary` | `hr:ExecutiveSummary` (escaped HTML) |
| `experiences` | one `EmployerHistory` per experience, newest first; descriptions, bullets and positions as escaped HTML in `oa:Description` |
| `education` | `EducationOrganizationAttendance`; numeric `eqf` becomes `EducationLevelCode` |
| foreign `languages` | `PersonQualifications/PersonCompetency`, CEFR in `eures:CompetencyDimension` |
| `skills` | `Skills` (`Digital_Skill`; `ESCO_Skill` with `CompetencyID` when `includeEsco` and an ESCO skill IRI is present), grouped by category; duplicates dropped |
| `projects` | custom sections (`Others`), one per project |

Not representable (reported as warnings): free-text language levels without CEFR grades, EQF labels without a
number. `SectionsOrder` lists only the sections that have content.
