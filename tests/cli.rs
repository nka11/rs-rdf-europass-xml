#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! The native CLI used by CI pipelines.
mod common;
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_europass-xml-cli"))
}

#[test]
fn writes_valid_xml_to_a_file() {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-out.xml");
    let input = common::root().join("tests/fixtures/cv_full_en.json");
    let st = cli().arg(&input).arg("-o").arg(&out).output().unwrap();
    assert!(
        st.status.success(),
        "{}",
        String::from_utf8_lossy(&st.stderr)
    );
    let xml = std::fs::read_to_string(&out).unwrap();
    assert_eq!(
        xml,
        common::export_default(&common::fixture("cv_full_en.json")).xml
    );
    common::assert_valid(&xml, "CLI output");
}

#[test]
fn prints_xml_to_stdout_without_output_flag() {
    let input = common::root().join("tests/fixtures/cv_min.json");
    let o = cli().arg(&input).output().unwrap();
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stdout).starts_with("<?xml"));
}

#[test]
fn bad_input_exits_1_with_a_code() {
    let bad = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("bad.json");
    std::fs::write(&bad, "{not json").unwrap();
    let o = cli().arg(&bad).output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("[json_syntax]"));
}

#[test]
fn usage_and_missing_file_exit_2() {
    assert_eq!(cli().output().unwrap().status.code(), Some(2));
    assert_eq!(
        cli()
            .arg("/nonexistent.json")
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn the_output_is_the_candidate_xml() {
    let input = common::root().join("tests/fixtures/cv_min.json");
    let o = cli().arg(&input).output().unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).contains("<Candidate "));
    // the legacy format flag is gone
    assert_eq!(
        cli()
            .arg(&input)
            .args(["--format", "legacy"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
}
