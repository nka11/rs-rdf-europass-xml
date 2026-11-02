#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Candidate XML (the format the current Europass editor imports). Golden files plus validation against the
//! unofficial schema fetched by scripts/fetch-candidate-schema.sh (skipped when absent unless REQUIRE_XMLLINT=1).
mod common;
use common::*;
use europass_xml::export::export_json;
use europass_xml::options::{Format, Options};
use std::process::Command;

const CASES: &[(&str, &str)] = &[
    ("cv_full_en.json", "candidate_full_en.xml"),
    ("cv_full_fr.json", "candidate_full_fr.xml"),
    ("cv_min.json", "candidate_min.xml"),
    ("cv_all_fields.json", "candidate_all_fields.xml"),
];

fn candidate(json: &str) -> europass_xml::export::Export {
    let opts = Options {
        format: Format::Candidate,
        ..Options::default()
    };
    export_json(json, &opts).expect("export")
}

fn validate(xml: &str) -> Result<bool, String> {
    let schema = root().join("schema/candidate/europass-candidate.xsd");
    let required = std::env::var("REQUIRE_XMLLINT").as_deref() == Ok("1");
    if !xmllint_available() || !schema.exists() {
        if required {
            return Err("xmllint or the Candidate schema is missing and REQUIRE_XMLLINT=1".into());
        }
        eprintln!("SKIP: xmllint or Candidate schema missing");
        return Ok(false);
    }
    let file = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("cand-{}.xml", std::process::id()));
    std::fs::write(&file, xml).map_err(|e| e.to_string())?;
    let out = Command::new("bash")
        .arg(root().join("scripts/validate-candidate.sh"))
        .arg(&file)
        .output()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&file);
    if out.status.success() {
        Ok(true)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

#[test]
fn golden_files_match() {
    for (input, golden) in CASES {
        let out = candidate(&fixture(input)).xml;
        let path = golden_path(golden);
        if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
            std::fs::write(&path, &out).expect("write golden");
            continue;
        }
        let want = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("missing golden {golden}; run UPDATE_GOLDEN=1"));
        assert!(
            out == want,
            "{golden} differs; review and run UPDATE_GOLDEN=1 cargo test --test candidate"
        );
    }
}

#[test]
fn outputs_validate_against_the_candidate_schema() {
    for (input, _) in CASES.iter().chain(&[("cv_edge_bad.json", "")]) {
        let xml = candidate(&fixture(input)).xml;
        if let Err(e) = validate(&xml) {
            panic!("{input}: {e}");
        }
    }
}

#[test]
fn root_is_candidate_not_skills_passport() {
    let xml = candidate(&fixture("cv_full_en.json")).xml;
    assert!(xml.contains("<Candidate xmlns=\"http://www.europass.eu/1.0\""));
    assert!(!xml.contains("SkillsPassport"));
    assert!(xml.contains("xsi:schemaLocation=\"http://www.europass.eu/1.0 Candidate.xsd\""));
}

#[test]
fn rich_text_is_escaped_html() {
    let xml = candidate(&fixture("cv_all_fields.json")).xml;
    assert!(xml.contains("&lt;p&gt;"));
    assert!(xml.contains("&lt;ul&gt;&lt;li&gt;"));
    // never a literal tag inside a text node
    assert!(!xml.contains("<p>") && !xml.contains("<li>"));
}

#[test]
fn every_telephone_has_a_country_dialing() {
    // the editor rejects the whole file otherwise
    for (input, _) in CASES {
        let xml = candidate(&fixture(input)).xml;
        let phones = xml.matches("<ChannelCode>Telephone</ChannelCode>").count();
        let dialing = xml.matches("<CountryDialing>").count();
        assert_eq!(phones, dialing, "{input}");
    }
}

#[test]
fn deterministic() {
    let a = candidate(&fixture("cv_full_en.json")).xml;
    assert_eq!(a, candidate(&fixture("cv_full_en.json")).xml);
}

#[test]
fn format_option_is_read_from_json() {
    let o = Options::from_json(Some(r#"{"format":"candidate"}"#)).unwrap();
    assert_eq!(o.format, Format::Candidate);
    assert!(Options::from_json(Some(r#"{"format":"nope"}"#)).is_err());
}
