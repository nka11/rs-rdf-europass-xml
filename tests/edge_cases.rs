#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Boundary inputs and property-style fuzzing (deterministic xorshift, no external crates).
mod common;
use common::*;
use europass_xml::export::export_json;
use europass_xml::options::Options;
use serde_json::{json, Value};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const PIECES: &[&str] = &[
    "a",
    "Z",
    "0",
    " ",
    "é",
    "ç",
    "ü",
    "日本語",
    "😀",
    "&",
    "<",
    ">",
    "\"",
    "'",
    "]]>",
    "<![CDATA[",
    "-->",
    "\n",
    "\r\n",
    "\t",
    "\u{0}",
    "\u{1}",
    "\u{8}",
    "\u{B}",
    "\u{C}",
    "\u{1F}",
    "\u{7F}",
    "\u{85}",
    "\u{9F}",
    "\u{FFFE}",
    "\u{FFFF}",
    "\u{2028}",
    "\u{200B}",
    "\u{FEFF}",
    "\u{A0}",
    "@",
    ".",
    "a@b.co",
    "2020-01-01",
    "2020-13-01",
    "https://",
    "%zz",
    "#",
    "[",
    "{",
    "\\",
    "e\u{301}",
];

fn random_string(r: &mut Rng) -> String {
    let n = match r.below(10) {
        0 => 0,
        1 => 10_000,
        _ => r.below(40),
    };
    let mut s = String::new();
    if r.below(4) == 0 {
        s.push_str("https://");
    }
    for _ in 0..n {
        s.push_str(PIECES[r.below(PIECES.len())]);
    }
    s
}

fn fuzz(v: &mut Value, r: &mut Rng, path: &str) {
    match v {
        Value::String(s) => {
            // structural fields stay valid so the run reaches the XML writer
            if path != "lang" && path != "meta.generatedAt" && r.below(3) != 0 {
                *s = random_string(r);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| fuzz(x, r, &format!("{path}[]"))),
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                fuzz(x, r, &p);
            }
        }
        _ => {}
    }
}

#[test]
fn fuzzed_strings_never_panic_and_stay_well_formed() {
    let base: Value = serde_json::from_str(&fixture("cv_all_fields.json")).unwrap();
    let mut validated = 0;
    for seed in 1..=500u64 {
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut v = base.clone();
        fuzz(&mut v, &mut r, "");
        let e = export_json(&v.to_string(), &Options::legacy())
            .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        assert_well_formed(&e.xml);
        if seed % 20 == 0 {
            assert_valid(&e.xml, &format!("fuzz seed {seed}"));
            validated += 1;
        }
    }
    assert_eq!(validated, 25);
}

#[test]
fn fuzzed_strings_in_the_real_cv_validate() {
    let base: Value = serde_json::from_str(&fixture("cv_full_fr.json")).unwrap();
    for seed in [7u64, 99, 12345] {
        let mut r = Rng(seed | 1);
        let mut v = base.clone();
        fuzz(&mut v, &mut r, "");
        let e = export_json(&v.to_string(), &Options::legacy()).unwrap();
        assert_well_formed(&e.xml);
        assert_valid(&e.xml, &format!("real fuzz {seed}"));
    }
}

fn minimal() -> Value {
    json!({"schemaVersion":1,"lang":"en","person":{"name":"A B"},"meta":{"generatedAt":"2026-11-02T10:00:00Z"}})
}

fn run_valid(v: &Value) -> europass_xml::export::Export {
    let e = export_json(&v.to_string(), &Options::legacy()).expect("export");
    assert_well_formed(&e.xml);
    assert_valid(&e.xml, "edge");
    e
}

#[test]
fn empty_and_minimal_inputs() {
    let e = run_valid(
        &json!({"schemaVersion":1,"lang":"fr","person":{},"meta":{"generatedAt":"2026-11-02T10:00:00Z"}}),
    );
    assert!(e.xml.contains("<LearnerInfo/>"));
    let mut v = minimal();
    for k in [
        "languages",
        "experiences",
        "education",
        "skills",
        "projects",
    ] {
        v[k] = json!([]);
    }
    let e = run_valid(&v);
    assert!(!e.xml.contains("<WorkExperienceList>") && !e.xml.contains("<Skills>"));
}

#[test]
fn list_caps() {
    let mut v = minimal();
    v["experiences"] = Value::Array(
        (0..101)
            .map(|i| json!({"id": format!("e{i}"), "role": format!("R{i}"), "start": "2020-01-01"}))
            .collect(),
    );
    let e = run_valid(&v);
    assert_eq!(e.xml.matches("<WorkExperience>").count(), 100);
    assert!(e
        .warnings
        .iter()
        .any(|w| w.code == "list_truncated" && w.path == "experiences"));
    let mut v = minimal();
    v["skills"] = Value::Array(
        (0..5000)
            .map(|i| json!({"label": format!("skill number {i}"), "category": "TechnicalSkill"}))
            .collect(),
    );
    let e = run_valid(&v);
    assert!(e.warnings.iter().any(|w| w.code == "text_truncated"));
    let desc_start = e.xml.find("<Computer>").unwrap();
    let desc = &e.xml[desc_start..e.xml[desc_start..].find("</Computer>").unwrap() + desc_start];
    assert!(desc.chars().count() < 6100 && !desc.contains('\u{2026}'));
    // max_experiences option
    let o = Options::from_json(Some(r#"{"maxExperiences":3,"format":"legacy"}"#)).unwrap();
    let mut v = minimal();
    v["experiences"] = Value::Array(
        (0..10)
            .map(|i| json!({"id": "x", "role": format!("R{i}"), "start": "2020-01-01"}))
            .collect(),
    );
    assert_eq!(
        export_json(&v.to_string(), &o)
            .unwrap()
            .xml
            .matches("<WorkExperience>")
            .count(),
        3
    );
}

#[test]
fn dates_and_current() {
    let mut v = minimal();
    v["experiences"] = json!([
        {"id":"a","role":"R","start":"2020-01-01","end":"2021-01-01","current":true},
        {"id":"b","role":"R","start":"2022-05-01","end":"2021-01-01","current":false},
        {"id":"c","role":"R","start":"2020-01-01","end":"20014-12-01"}
    ]);
    let e = run_valid(&v);
    assert_eq!(e.xml.matches("<Current>true</Current>").count(), 1);
    assert_eq!(e.xml.matches("<To ").count(), 1);
    assert!(e.warnings.iter().any(|w| w.code == "date_order"));
    assert!(e
        .warnings
        .iter()
        .any(|w| w.code == "date_invalid" && w.path == "experiences[2].end"));
    assert!(!e.xml.contains("month=\"03\""));
}

#[test]
fn urls_and_emails_are_screened() {
    let mut v = minimal();
    v["person"]["websites"] = json!([{"url":"javascript:alert(1)"},{"url":"file:///etc/passwd"},{"url":"https://ok.example.org/p?q=1#f"}]);
    v["person"]["email"] = json!("nodomain@host");
    let e = run_valid(&v);
    assert_eq!(e.xml.matches("<Website>").count(), 1);
    assert!(
        !e.xml.contains("javascript") && !e.xml.contains("passwd") && !e.xml.contains("<Email>")
    );
}

#[test]
fn skills_accept_strings_and_objects() {
    let mut v = minimal();
    v["experiences"] = json!([{"id":"a","role":"R","start":"2020-01-01","skills":["Go",{"label":"Rust"},{"label":"go"}]}]);
    let e = run_valid(&v);
    assert!(e.xml.contains("Skills: Go, Rust"));
}

#[test]
fn unknown_keys_ignored_and_ascii_hyphen_bullets() {
    let mut v = minimal();
    v["zzz"] = json!({"a": [1, 2, 3]});
    v["person"]["extra"] = json!(true);
    v["experiences"] = json!([{"id":"a","role":"R","start":"2020-01-01","description":"d","bullets":["one","two"],"future":1}]);
    let e = run_valid(&v);
    assert!(e.xml.contains("d\n- one\n- two"));
}

#[test]
fn hostile_text_is_escaped_not_interpreted() {
    let mut v = minimal();
    v["person"]["name"] = json!("A </Surname><Evil/> B & C");
    v["person"]["headline"] = json!("]]> <![CDATA[ x ]]>");
    let e = run_valid(&v);
    assert!(!e.xml.contains("<Evil/>"));
    assert!(e.xml.contains("&lt;Evil/&gt;"));
    assert!(!e.xml.contains("<![CDATA["));
}

#[test]
fn illegal_chars_are_removed_with_a_warning() {
    let mut v = minimal();
    v["person"]["headline"] = json!("Dev\u{0}elo\u{7}per\u{FFFE}");
    let e = run_valid(&v);
    assert!(e.xml.contains("<Label>Developer</Label>"));
    assert!(e.warnings.iter().any(|w| w.code == "text_sanitized"));
}

#[test]
fn long_text_is_truncated_with_ellipsis_on_a_scalar_boundary() {
    let mut v = minimal();
    v["person"]["headline"] = json!("é😀 ".repeat(2000));
    let e = run_valid(&v);
    assert!(e.warnings.iter().any(|w| w.code == "text_truncated"));
    assert!(e.xml.contains('\u{2026}'));
    let mut v = minimal();
    v["person"]["name"] = json!("x".repeat(10_000));
    let e = run_valid(&v);
    assert!(e
        .warnings
        .iter()
        .any(|w| w.code == "text_truncated" && w.path == "person.name"));
}

#[test]
fn fr_and_en_differ_only_by_labels() {
    let mut v = minimal();
    v["person"]["headline"] = json!("Dev");
    v["experiences"] = json!([{"id":"a","role":"R","start":"2020-01-01","skills":["Go"]}]);
    let en = run_valid(&v).xml;
    v["lang"] = json!("fr");
    let fr = run_valid(&v).xml;
    assert_ne!(en, fr);
    assert!(en.contains("<Label>Position</Label>") && fr.contains("<Label>Poste</Label>"));
    assert!(en.contains("Skills: Go") && fr.contains("Compétences : Go"));
}
