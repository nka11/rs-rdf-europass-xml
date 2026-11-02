#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Every generated document must validate with scripts/validate.sh (xmllint, offline).
mod common;
use common::*;
use europass_xml::export::export_json;
use europass_xml::options::Options;

fn opts(json: &str) -> Options {
    let mut o = Options::from_json(Some(json)).expect("options");
    o.format = europass_xml::options::Format::Legacy;
    o
}

#[test]
fn goldens_and_fixtures_validate() {
    for f in [
        "cv_full_en.json",
        "cv_full_fr.json",
        "cv_min.json",
        "cv_all_fields.json",
        "cv_edge_bad.json",
    ] {
        let e = export_default(&fixture(f));
        assert_well_formed(&e.xml);
        assert_valid(&e.xml, f);
    }
    for g in [
        "cv_full_en.xml",
        "cv_full_fr.xml",
        "cv_min.xml",
        "cv_all_fields.xml",
    ] {
        let xml = std::fs::read_to_string(golden_path(g)).expect("golden");
        assert_valid(&xml, g);
    }
}

#[test]
fn option_variants_validate() {
    let full = fixture("cv_full_en.json");
    let all = fixture("cv_all_fields.json");
    for o in [
        r#"{"datePrecision":"year"}"#,
        r#"{"datePrecision":"day"}"#,
        r#"{"includeEsco":true}"#,
        r#"{"summaryPlacement":"drop"}"#,
        r#"{"includeProjects":false,"includeSkills":false}"#,
        r#"{"maxExperiences":2}"#,
        r#"{"maxActivitiesChars":300}"#,
        r#"{"pretty":false}"#,
        r#"{"schemaLocationHint":true}"#,
        r#"{"creationDate":"2020-01-01T00:00:00Z"}"#,
    ] {
        for json in [&full, &all] {
            let e = export_json(json, &opts(o)).expect("export");
            assert_well_formed(&e.xml);
            assert_valid(&e.xml, o);
        }
    }
}

#[test]
fn negative_control_misordered_document_fails() {
    // Skills before Headline violates the xsd:sequence of LearnerInfoType.
    let bad = r#"<?xml version="1.0" encoding="UTF-8"?>
<SkillsPassport xmlns="http://europass.cedefop.europa.eu/Europass" locale="en">
  <DocumentInfo><DocumentType>ECV</DocumentType><CreationDate>2026-11-02T10:00:00Z</CreationDate><LastUpdateDate>2026-11-02T10:00:00Z</LastUpdateDate><XSDVersion>V3.4</XSDVersion><Generator>x</Generator></DocumentInfo>
  <LearnerInfo><Skills><Other><Description>x</Description></Other></Skills><Headline><Description><Label>y</Label></Description></Headline></LearnerInfo>
</SkillsPassport>
"#;
    if xmllint_available() {
        assert!(
            validate_with_xmllint(bad).is_err(),
            "harness must reject a misordered document"
        );
        let good = bad.replace(
            "<Skills><Other><Description>x</Description></Other></Skills><Headline><Description><Label>y</Label></Description></Headline>",
            "<Headline><Description><Label>y</Label></Description></Headline><Skills><Other><Description>x</Description></Other></Skills>",
        );
        assert!(validate_with_xmllint(&good).is_ok());
    }
}

#[test]
fn negative_controls_for_rules() {
    // email without a dot in the domain: warning, no Email element
    let json = r#"{"schemaVersion":1,"lang":"en","person":{"name":"A B","email":"a@b"},"meta":{"generatedAt":"2026-11-02T10:00:00Z"}}"#;
    let e = export_default(json);
    assert!(e.warnings.iter().any(|w| w.code == "email_invalid"));
    assert!(!e.xml.contains("<Email>"));
    assert_valid(&e.xml, "email control");
    // eqf 0 omits Level/Code
    let json = r#"{"schemaVersion":1,"lang":"en","person":{"name":"A B"},"education":[{"title":"T","eqf":0}],"meta":{"generatedAt":"2026-11-02T10:00:00Z"}}"#;
    let e = export_default(json);
    assert!(e.warnings.iter().any(|w| w.code == "eqf_out_of_range"));
    assert!(!e.xml.contains("<Level>"));
    assert_valid(&e.xml, "eqf control");
    // website URL with a space is skipped
    let json = r#"{"schemaVersion":1,"lang":"en","person":{"name":"A B","websites":[{"url":"https://a b.org"}]},"meta":{"generatedAt":"2026-11-02T10:00:00Z"}}"#;
    let e = export_default(json);
    assert!(e.warnings.iter().any(|w| w.code == "url_invalid"));
    assert!(!e.xml.contains("<Website>"));
    assert_valid(&e.xml, "url control");
}

#[test]
fn schema_location_hint_does_not_change_validity() {
    let e = export_json(
        &fixture("cv_min.json"),
        &opts(r#"{"schemaLocationHint":true}"#),
    )
    .expect("export");
    assert!(e.xml.contains("xsi:schemaLocation=\"http://europass.cedefop.europa.eu/Europass http://europass.cedefop.europa.eu/xml/v3.4.1/EuropassSchema.xsd\""));
    assert_valid(&e.xml, "hint");
}

#[test]
fn child_order_follows_the_xsd_sequence() {
    // Tag lists are hard-coded from the XSDs, not derived from the code.
    let xml = export_default(&fixture("cv_all_fields.json")).xml;
    let seqs: &[&[&str]] = &[
        &["<DocumentInfo>", "<LearnerInfo>"],
        &[
            "<DocumentType>",
            "<CreationDate>",
            "<LastUpdateDate>",
            "<XSDVersion>",
            "<Generator>",
            "<Comment>",
        ],
        &[
            "<Identification>",
            "<Headline>",
            "<WorkExperienceList>",
            "<EducationList>",
            "<Skills>",
            "<AchievementList>",
        ],
        &["<PersonName>", "<ContactInfo>", "<Demographics>"],
        &["<Address>", "<Email>", "<TelephoneList>", "<WebsiteList>"],
        &[
            "<AddressLine>",
            "<AddressLine2>",
            "<PostalCode>",
            "<Municipality>",
            "<Country>",
        ],
        &["<Birthdate", "<Gender>", "<NationalityList>"],
        &[
            "<Linguistic>",
            "<Organisational>",
            "<JobRelated>",
            "<Computer>",
            "<Other>",
        ],
        &["<MotherTongueList>", "<ForeignLanguageList>"],
        &[
            "<Listening>",
            "<Reading>",
            "<SpokenInteraction>",
            "<SpokenProduction>",
            "<Writing>",
        ],
    ];
    for seq in seqs {
        let mut at = 0;
        for tag in *seq {
            let p = xml[at..]
                .find(tag)
                .unwrap_or_else(|| panic!("{tag} missing or out of order in {seq:?}"));
            at += p;
        }
    }
    // Work experience: Period, Position, Activities, Employer
    let w = &xml[xml.find("<WorkExperience>").expect("we")..];
    let order: Vec<usize> = ["<Period>", "<Position>", "<Activities>", "<Employer>"]
        .iter()
        .map(|t| w.find(t).expect(t))
        .collect();
    assert!(order.windows(2).all(|p| p[0] < p[1]));
    // Education: Period, Title, Activities, Organisation, Level
    let w = &xml[xml.find("<Education>").expect("edu")..];
    let order: Vec<usize> = [
        "<Period>",
        "<Title>",
        "<Activities>",
        "<Organisation>",
        "<Level>",
    ]
    .iter()
    .map(|t| w.find(t).expect(t))
    .collect();
    assert!(order.windows(2).all(|p| p[0] < p[1]));
}
