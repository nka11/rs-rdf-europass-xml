#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
//! Byte-exact golden files. Regenerate with `UPDATE_GOLDEN=1 cargo test golden`.
mod common;
use common::*;

const CASES: &[(&str, &str)] = &[
    ("cv_full_en.json", "cv_full_en.xml"),
    ("cv_full_fr.json", "cv_full_fr.xml"),
    ("cv_min.json", "cv_min.xml"),
    ("cv_all_fields.json", "cv_all_fields.xml"),
];

#[test]
fn golden_files_match() {
    for (input, golden) in CASES {
        let out = export_default(&fixture(input)).xml;
        let path = golden_path(golden);
        if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
            std::fs::write(&path, &out).expect("write golden");
            continue;
        }
        let want = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("missing golden {golden}; run UPDATE_GOLDEN=1"));
        assert!(out == want, "{golden} differs from the generated output; review and run UPDATE_GOLDEN=1 cargo test golden");
    }
}

#[test]
fn fr_and_en_have_the_same_element_paths() {
    fn tags(xml: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack: Vec<&str> = Vec::new();
        for part in xml.split('<').skip(2) {
            let Some((tag, _)) = part.split_once('>') else {
                continue;
            };
            if let Some(n) = tag.strip_prefix('/') {
                stack.pop();
                let _ = n;
            } else {
                let name = tag
                    .trim_end_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                // WorkExperience children lists differ with the data (skills lists), compare structure
                // of the top levels only
                if stack.len() <= 2 {
                    out.push(format!("{}/{}", stack.join("/"), name));
                }
                if !tag.ends_with('/') {
                    stack.push(name);
                }
            }
        }
        out
    }
    let en = tags(&export_default(&fixture("cv_full_en.json")).xml);
    let fr = tags(&export_default(&fixture("cv_full_fr.json")).xml);
    let uniq = |v: &Vec<String>| {
        let mut s = v.clone();
        s.sort();
        s.dedup();
        s
    };
    assert_eq!(uniq(&en), uniq(&fr));
}

#[test]
fn no_cdata_and_locale() {
    let en = export_default(&fixture("cv_full_en.json")).xml;
    let fr = export_default(&fixture("cv_full_fr.json")).xml;
    assert!(en.contains("locale=\"en\"") && fr.contains("locale=\"fr\""));
    assert!(!en.contains("<![CDATA[") && !fr.contains("<![CDATA["));
    assert!(fr.contains("Compétences :") || !fr.contains("Skills:"));
}

#[test]
fn long_experience_keeps_every_role_and_nothing_is_cut() {
    for f in ["cv_full_en.json", "cv_full_fr.json"] {
        let e = europass_xml::export::export_json(
            &fixture(f),
            &europass_xml::options::Options::legacy(),
        )
        .expect("export");
        assert!(
            e.xml.contains("PeopleSoft") && e.xml.contains("procstock"),
            "{f}: 2001-2011 roles missing"
        );
        assert!(
            !e.warnings.iter().any(|w| w.code == "text_truncated"),
            "{f}: text was cut"
        );
        assert!(
            e.warnings.iter().any(|w| w.code == "experience_split"),
            "{f}: split not reported"
        );
        assert!(
            e.warnings
                .iter()
                .any(|w| w.code == "project_outside_parent_period"),
            "{f}: DUT internship dates not flagged"
        );
    }
}
