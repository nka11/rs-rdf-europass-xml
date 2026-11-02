#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod common;
use common::*;
use europass_xml::export::export_json;
use europass_xml::options::Options;

#[test]
fn same_input_same_bytes() {
    for f in ["cv_full_en.json", "cv_full_fr.json", "cv_all_fields.json"] {
        let a = export_default(&fixture(f)).xml;
        let b = export_default(&fixture(f)).xml;
        assert_eq!(a, b);
    }
}

/// Serialise an object with its keys in reverse order (textual, independent of the map type).
fn to_json_reversed(v: &serde_json::Value) -> String {
    use serde_json::Value::{Array, Object};
    match v {
        Object(m) => {
            let mut parts: Vec<String> = m
                .iter()
                .map(|(k, x)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap(),
                        to_json_reversed(x)
                    )
                })
                .collect();
            parts.reverse();
            format!("{{{}}}", parts.join(","))
        }
        Array(a) => format!(
            "[{}]",
            a.iter().map(to_json_reversed).collect::<Vec<_>>().join(",")
        ),
        o => o.to_string(),
    }
}

#[test]
fn object_key_order_is_irrelevant() {
    for f in ["cv_full_en.json", "cv_all_fields.json"] {
        let v: serde_json::Value = serde_json::from_str(&fixture(f)).unwrap();
        let reversed = to_json_reversed(&v);
        assert_ne!(reversed, fixture(f));
        let a = export_default(&fixture(f)).xml;
        let b = export_default(&reversed).xml;
        assert_eq!(a, b, "{f}");
    }
}

#[test]
fn null_equals_absent() {
    let a = r#"{"schemaVersion":1,"lang":"en","person":{"name":"A B"},"experiences":[{"id":"x","role":"R","start":"2020-01-01"}],"meta":{"generatedAt":"2026-11-02T10:00:00Z"}}"#;
    let b = r#"{"schemaVersion":1,"lang":"en","person":{"name":"A B","email":null,"headline":null},"experiences":[{"id":"x","role":"R","start":"2020-01-01","end":null,"current":null,"description":null,"url":null}],"education":null,"meta":{"generatedAt":"2026-11-02T10:00:00Z","generator":null}}"#;
    assert_eq!(
        export_json(a, &Options::legacy()).map(|e| e.xml),
        export_json(b, &Options::legacy()).map(|e| e.xml)
    );
}

#[test]
fn pretty_false_is_one_line_after_declaration() {
    let o = Options::from_json(Some(r#"{"pretty":false,"format":"legacy"}"#)).unwrap();
    let xml = export_json(&fixture("cv_min.json"), &o).unwrap().xml;
    let body = xml.split_once('\n').unwrap().1;
    assert_eq!(body.matches('\n').count(), 1);
    assert!(body.ends_with('\n'));
}
