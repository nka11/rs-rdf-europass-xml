//! Shared helpers: fixture loading and XSD validation through scripts/validate.sh.
#![allow(dead_code)]
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(root().join("tests/fixtures").join(name))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

pub fn golden_path(name: &str) -> PathBuf {
    root().join("tests/fixtures/golden").join(name)
}

pub fn export_default(json: &str) -> europass_xml::export::Export {
    europass_xml::export::export_json(json, &europass_xml::options::Options::legacy())
        .expect("export")
}

pub fn xmllint_available() -> bool {
    Command::new("xmllint").arg("--version").output().is_ok()
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Validate against the vendored Europass XSD v3.4.1 with scripts/validate.sh.
/// `Ok(true)` = validated, `Ok(false)` = skipped because xmllint is missing (loud message).
/// With `REQUIRE_XMLLINT=1` a missing xmllint is an error.
pub fn validate_with_xmllint(xml: &str) -> Result<bool, String> {
    if !xmllint_available() {
        if std::env::var("REQUIRE_XMLLINT").as_deref() == Ok("1") {
            return Err("xmllint missing and REQUIRE_XMLLINT=1".into());
        }
        eprintln!("SKIP: xmllint missing");
        return Ok(false);
    }
    if !root().join("schema/v3.4.1/EuropassSchema.xsd").exists() {
        return Err("Europass XSD not found: run scripts/fetch-schema.sh first".into());
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let file = dir.join(format!("out-{}-{n}.xml", std::process::id()));
    std::fs::write(&file, xml).map_err(|e| e.to_string())?;
    let out = Command::new("bash")
        .arg(root().join("scripts/validate.sh"))
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

pub fn assert_valid(xml: &str, what: &str) {
    if let Err(e) = validate_with_xmllint(xml) {
        panic!("{what} is not XSD valid:\n{e}");
    }
}

/// Mini well-formedness checker: balanced tags, legal chars, no CDATA, one root.
pub fn assert_well_formed(xml: &str) {
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
    assert!(xml.ends_with(">\n") && !xml.ends_with(">\n\n"));
    assert!(!xml.contains("<![CDATA["));
    assert!(!xml.contains("<!--"));
    for c in xml.chars() {
        let u = c as u32;
        let ok = c == '\t'
            || c == '\n'
            || c == '\r'
            || (0x20..=0xD7FF).contains(&u)
            || (0xE000..=0xFFFD).contains(&u)
            || u >= 0x10000;
        assert!(ok && !(0x7F..=0x9F).contains(&u), "illegal char U+{u:04X}");
    }
    let body = xml.split_once("?>").map(|x| x.1).unwrap_or("");
    let mut stack: Vec<String> = Vec::new();
    let mut rest = body;
    let mut roots = 0;
    while let Some(i) = rest.find('<') {
        let after = &rest[i + 1..];
        let j = after.find('>').expect("unterminated tag");
        let tag = &after[..j];
        assert!(!tag.contains('<'), "raw < inside tag: {tag}");
        if let Some(name) = tag.strip_prefix('/') {
            assert_eq!(stack.pop().as_deref(), Some(name), "unbalanced");
        } else {
            let selfclose = tag.ends_with('/');
            let name = tag
                .trim_end_matches('/')
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            if stack.is_empty() {
                roots += 1;
            }
            if !selfclose {
                stack.push(name);
            }
        }
        rest = &after[j + 1..];
    }
    assert!(stack.is_empty(), "unclosed tags: {stack:?}");
    assert_eq!(roots, 1);
}
