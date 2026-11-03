#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod common;
use common::*;
use europass_xml::error::ExportError;
use europass_xml::export::export_json;
use europass_xml::input::validate_json;
use europass_xml::options::Options;
use serde_json::{json, Value};

fn base() -> Value {
    json!({"schemaVersion":1,"lang":"en","person":{"name":"A B"},"meta":{"generatedAt":"2026-11-02T10:00:00Z"}})
}

fn err(v: &Value) -> ExportError {
    export_json(&v.to_string(), &Options::default()).expect_err("must fail")
}

#[test]
fn goldens_are_ok() {
    for f in [
        "cv_full_en.json",
        "cv_full_fr.json",
        "cv_min.json",
        "cv_all_fields.json",
    ] {
        let v = validate_json(&fixture(f));
        assert!(v.ok, "{f}: {:?}", v.errors);
        assert!(v.errors.is_empty());
    }
}

#[test]
fn bad_fixture_reports_codes_and_paths() {
    let v = validate_json(&fixture("cv_edge_bad.json"));
    assert!(v.ok);
    let has = |code: &str, path: &str| v.warnings.iter().any(|w| w.code == code && w.path == path);
    assert!(has("email_invalid", "person.email"));
    assert!(has("url_invalid", "person.websites[0].url"));
    assert!(has("url_invalid", "person.websites[1].url"));
    assert!(has("phone_prefix_unknown", "person.phones[0]"));
    assert!(has("enum_unknown", "person.gender"));
    assert!(has("date_invalid", "person.birthDate"));
    assert!(has("cef_invalid", "languages[2].cef.listening"));
    assert!(has("date_invalid", "experiences[0].start"));
    assert!(has("date_invalid", "experiences[0].end"));
    assert!(has("date_order", "experiences[1]"));
    assert!(has("empty_entry", "experiences[2]"));
    for (i, _) in ["0", "9", "9.5", "-1", "256"].iter().enumerate() {
        assert!(
            has("eqf_out_of_range", &format!("education[{i}].eqf")),
            "eqf {i}"
        );
    }
    assert!(has("empty_entry", "education[6]"));
    assert!(has("empty_entry", "skills[2]"));
    assert!(has("duplicate_skill", "skills"));
    assert!(has("url_invalid", "projects[0].url"));
    assert!(has("empty_entry", "projects[1]"));
    // all messages are one line
    assert!(v.warnings.iter().all(|w| !w.message.contains('\n')));
}

#[test]
fn hard_errors() {
    let mut v = base();
    v["lang"] = json!("de");
    assert_eq!(err(&v).code(), "invalid_lang");
    assert!(err(&v)
        .to_string()
        .starts_with("[invalid_lang] lang must be \"en\" or \"fr\", got \"de\""));

    for sv in [json!(2), json!("1"), json!(1.5), json!(null), json!(true)] {
        let mut v = base();
        v["schemaVersion"] = sv;
        assert_eq!(err(&v).code(), "unsupported_schema_version");
    }
    let mut v = base();
    v.as_object_mut().unwrap().remove("schemaVersion");
    assert_eq!(err(&v).code(), "unsupported_schema_version");

    let mut v = base();
    v.as_object_mut().unwrap().remove("person");
    assert!(matches!(
        err(&v),
        ExportError::MissingField { path: "person" }
    ));
    let mut v = base();
    v.as_object_mut().unwrap().remove("meta");
    assert!(matches!(
        err(&v),
        ExportError::MissingField { path: "meta" }
    ));
    let mut v = base();
    v["meta"] = json!({});
    assert!(matches!(
        err(&v),
        ExportError::MissingField {
            path: "meta.generatedAt"
        }
    ));
    for g in [
        "garbage",
        "2026-11-02",
        "2026-11-02T24:00:00Z",
        "2026-02-30T10:00:00Z",
    ] {
        let mut v = base();
        v["meta"]["generatedAt"] = json!(g);
        assert_eq!(err(&v).code(), "invalid_generated_at", "{g}");
    }
    // lang is case-insensitive
    let mut v = base();
    v["lang"] = json!("EN");
    assert!(export_json(&v.to_string(), &Options::default())
        .unwrap()
        .xml
        .contains("languageCode=\"en\""));
}

#[test]
fn json_level_errors() {
    for bad in [
        "",
        "{",
        "[]",
        "null",
        "{\"experiences\":3}",
        "{\"person\":[]}",
        "{\"experiences\":[{\"skills\":[5]}]}",
    ] {
        let e = export_json(bad, &Options::default()).expect_err(bad);
        assert_eq!(e.code(), "json_syntax", "{bad}");
    }
    let big = format!("{{\"x\":\"{}\"}}", "a".repeat(4 * 1024 * 1024));
    assert_eq!(
        export_json(&big, &Options::default()).unwrap_err().code(),
        "input_too_large"
    );
}

#[test]
fn validate_never_throws_and_reports_all_errors() {
    let v = validate_json("not json");
    assert!(!v.ok && v.errors[0].code == "json_syntax");
    let v = validate_json("{\"lang\":\"xx\"}");
    assert!(!v.ok);
    let codes: Vec<&str> = v.errors.iter().map(|e| e.code.as_str()).collect();
    assert_eq!(
        codes,
        vec![
            "unsupported_schema_version",
            "invalid_lang",
            "missing_field",
            "missing_field"
        ]
    );
    // same logic as the exporter
    let s = europass_xml::validate_cv_input(&fixture("cv_min.json"));
    let parsed: Value = serde_json::from_str(&s).unwrap();
    assert_eq!(parsed["ok"], json!(true));
}

#[test]
fn options_validation() {
    for bad in [
        r#"{"datePrecision":"week"}"#,
        r#"{"pretty":"yes"}"#,
        r#"{"maxExperiences":-1}"#,
        r#"{"creationDate":"x"}"#,
        "{",
        r#"{"maxActivitiesChars":1}"#,
    ] {
        assert_eq!(
            Options::from_json(Some(bad)).unwrap_err().code(),
            "bad_options",
            "{bad}"
        );
    }
    for ok in [
        None,
        Some(""),
        Some("null"),
        Some("{}"),
        Some(r#"{"unknown":1}"#),
        Some(r#"{"maxExperiences":null}"#),
    ] {
        assert!(Options::from_json(ok).is_ok(), "{ok:?}");
    }
}

#[test]
fn info_uses_the_crate_version() {
    let v: Value = serde_json::from_str(&europass_xml::europass_xml_info()).unwrap();
    assert_eq!(v["crate"], json!(env!("CARGO_PKG_VERSION")));
    assert_eq!(v["format"], json!("candidate"));
    assert_eq!(v["schemaVersion"], json!(1));
}
