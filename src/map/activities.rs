//! Composition of the plain-text blobs (DESIGN section 4.4). Deterministic, ASCII separators.
use super::{DateField, Report, LIM_LINE};
use crate::i18n::Key;
use crate::input::{PositionEntry, Project, SkillRef};
use crate::text::{clean_uri, dedup_ci};

/// `<start> - <end|present>`; an invalid end prints the start only; no start, no range.
pub fn range_text(rep: &Report, start: DateField, end: DateField, open: bool) -> Option<String> {
    let p = rep.opts.date_precision;
    let s = start.ok()?.to_text(p);
    Some(match end {
        DateField::Ok(e) => format!("{s} - {}", e.to_text(p)),
        DateField::Absent if open => format!("{s} - {}", rep.locale.t(Key::Present)),
        _ => s,
    })
}

/// `Skills: a, b, c` (de-duplicated, first occurrence wins).
pub fn skills_line(rep: &mut Report, path: &str, skills: &[SkillRef]) -> Option<String> {
    let mut labels = Vec::new();
    for (i, s) in skills.iter().enumerate() {
        if let Some(l) = rep.line(&format!("{path}[{i}]"), s.label(), LIM_LINE) {
            labels.push(l);
        }
    }
    let labels = dedup_ci(labels);
    if labels.is_empty() {
        None
    } else {
        Some(format!(
            "{} {}",
            rep.locale.t(Key::SkillsPrefix),
            labels.join(", ")
        ))
    }
}

fn bullets(rep: &mut Report, path: &str, items: &[String], lines: &mut Vec<String>) {
    for (i, b) in items.iter().enumerate() {
        if let Some(t) = rep.line(&format!("{path}[{i}]"), b, rep.opts.max_activities_chars) {
            lines.push(format!("- {t}"));
        }
    }
}

/// What is already said around a project: role words and descriptions that must not be repeated.
#[derive(Default, Clone)]
pub struct Known {
    pub roles: Vec<String>,
    pub descs: Vec<String>,
}

fn words(s: &str) -> Vec<String> {
    let mut w: Vec<String> = s
        .split(|c: char| !c.is_alphanumeric())
        .filter(|x| !x.is_empty())
        .map(str::to_lowercase)
        .collect();
    w.sort();
    w.dedup();
    w
}

/// True when two texts say the same thing: equal word sets, or one is (almost) contained in the other.
/// Fuzzy on purpose: a project description often restates the experience paragraph with a word changed.
pub fn near_duplicate(a: &str, b: &str) -> bool {
    let (wa, wb) = (words(a), words(b));
    let (small, big) = if wa.len() <= wb.len() {
        (&wa, &wb)
    } else {
        (&wb, &wa)
    };
    if small.len() < 6 {
        return small == big;
    }
    let common = small
        .iter()
        .filter(|w| big.binary_search(w).is_ok())
        .count();
    common * 100 >= small.len() * 85
}

fn same_ci(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

type Ym = (u16, u8);

fn lo(d: crate::date::IsoDate) -> Ym {
    (d.year, d.month.unwrap_or(1))
}

fn hi(d: crate::date::IsoDate) -> Ym {
    (d.year, d.month.unwrap_or(12))
}

fn quiet_date(s: &Option<String>) -> Option<crate::date::IsoDate> {
    let t = crate::text::sanitize_line(s.as_deref()?).text;
    crate::date::parse_iso_date(&t)
}

/// A project whose period lies completely outside its parent's period is a data error (a 1998 diploma
/// with a 2022 internship). The project dates are dropped and a warning says so.
pub fn clip_project(
    rep: &mut Report,
    path: &str,
    p: &Project,
    parent_start: Option<crate::date::IsoDate>,
    parent_end: Option<crate::date::IsoDate>,
) -> Project {
    let ps = quiet_date(&p.start);
    let pe = quiet_date(&p.end);
    let before = matches!((pe, parent_start), (Some(e), Some(s)) if hi(e) < lo(s));
    let after = matches!((ps, parent_end), (Some(s), Some(e)) if lo(s) > hi(e));
    if !(before || after) {
        return p.clone();
    }
    rep.warn(
        "project_outside_parent_period",
        path,
        format!(
            "project \"{}\" lies outside the period of its parent, its dates are not exported",
            crate::error::short(&p.name)
        ),
    );
    Project {
        start: None,
        end: None,
        ..p.clone()
    }
}

/// Block for a project. `None` when the project has no name.
pub fn project_block(
    rep: &mut Report,
    path: &str,
    p: &Project,
    include_url: bool,
    known: &Known,
) -> Option<String> {
    let max = rep.opts.max_activities_chars;
    let Some(name) = rep.line(&format!("{path}.name"), &p.name, LIM_LINE) else {
        rep.warn("empty_entry", path, "project without name skipped".into());
        return None;
    };
    let role = rep
        .opt_line(&format!("{path}.role"), &p.role, LIM_LINE)
        .filter(|r| !known.roles.iter().any(|k| same_ci(k, r)));
    let start = rep.date(&format!("{path}.start"), &p.start);
    let end = rep.date(&format!("{path}.end"), &p.end);
    let mut header = name;
    if let Some(r) = role {
        header.push_str(" - ");
        header.push_str(&r);
    }
    if let Some(r) = range_text(rep, start, end, true) {
        header.push_str(&format!(" ({r})"));
    }
    let mut lines = vec![header];
    if let Some(d) = rep.opt_block(&format!("{path}.description"), &p.description, max) {
        if !known.descs.iter().any(|k| near_duplicate(k, &d)) {
            lines.push(d);
        }
    }
    bullets(rep, &format!("{path}.bullets"), &p.bullets, &mut lines);
    if include_url {
        if let Some(u) = rep.opt_line(&format!("{path}.url"), &p.url, 2000) {
            if let Some(u) = clean_uri(&u) {
                lines.push(format!("{} {u}", rep.locale.t(Key::UrlPrefix)));
            } else {
                rep.warn(
                    "url_invalid",
                    &format!("{path}.url"),
                    format!(
                        "not an http, https or ftp URL: \"{}\"",
                        crate::error::short(&u)
                    ),
                );
            }
        }
    }
    if let Some(s) = skills_line(rep, &format!("{path}.skills"), &p.skills) {
        lines.push(s);
    }
    Some(lines.join("\n"))
}

/// One credential of an experience, already sanitised.
pub struct PosParts {
    pub title: Option<String>,
    pub start: DateField,
    pub end: DateField,
    /// The header line would repeat the experience's own role and period.
    pub header_redundant: bool,
    /// Description and bullets lines.
    pub body: Vec<String>,
    pub projects: Vec<String>,
}

pub struct ExpParts {
    pub top: Option<String>,
    pub positions: Vec<PosParts>,
}

fn pos_parts(
    rep: &mut Report,
    path: &str,
    pos: &PositionEntry,
    e: &crate::input::Experience,
    role: &Option<String>,
    top_desc: &Option<String>,
    single: bool,
) -> PosParts {
    let max = rep.opts.max_activities_chars;
    let title = rep.opt_line(&format!("{path}.title"), &pos.title, LIM_LINE);
    let start = rep.date(&format!("{path}.start"), &pos.start);
    let end = rep.date(&format!("{path}.end"), &pos.end);
    let norm = |s: &Option<String>| {
        s.as_deref()
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(str::to_string)
    };
    let same_dates = (norm(&pos.start).is_none() && norm(&pos.end).is_none())
        || (norm(&pos.start) == norm(&e.start) && norm(&pos.end) == norm(&e.end));
    let header_redundant = single
        && same_dates
        && matches!((&title, role), (Some(t), Some(r)) if r.to_lowercase().starts_with(&t.to_lowercase()));
    let mut body = Vec::new();
    let mut pos_desc: Option<String> = None;
    if let Some(d) = rep.opt_block(&format!("{path}.description"), &pos.description, max) {
        if !top_desc.as_deref().is_some_and(|t| near_duplicate(t, &d)) {
            pos_desc = Some(d.clone());
            body.push(d);
        }
    }
    bullets(rep, &format!("{path}.bullets"), &pos.bullets, &mut body);
    let mut known = Known::default();
    known.roles.extend(role.iter().cloned());
    known.roles.extend(title.iter().cloned());
    known.descs.extend(top_desc.iter().cloned());
    known.descs.extend(pos_desc);
    let (es, ee) = (quiet_date(&e.start), quiet_date(&e.end));
    let mut projects = Vec::new();
    for (j, p) in pos.projects.iter().enumerate() {
        let pp = format!("{path}.projects[{j}]");
        let p = clip_project(rep, &pp, p, es, ee);
        if let Some(b) = project_block(rep, &pp, &p, false, &known) {
            projects.push(b);
        }
    }
    PosParts {
        title,
        start,
        end,
        header_redundant,
        body,
        projects,
    }
}

/// Sanitise every piece of an experience once (warnings are emitted here, once).
pub fn exp_parts(rep: &mut Report, path: &str, e: &crate::input::Experience) -> ExpParts {
    let max = rep.opts.max_activities_chars;
    let role = rep.opt_line(&format!("{path}.role"), &e.role, LIM_LINE);
    let top_desc = rep.opt_block(&format!("{path}.description"), &e.description, max);
    let mut top = Vec::new();
    top.extend(top_desc.iter().cloned());
    bullets(rep, &format!("{path}.bullets"), &e.bullets, &mut top);
    if let Some(s) = skills_line(rep, &format!("{path}.skills"), &e.skills) {
        top.push(s);
    }
    let single = e.positions.len() == 1;
    let positions = e
        .positions
        .iter()
        .enumerate()
        .map(|(i, pos)| {
            pos_parts(
                rep,
                &format!("{path}.positions[{i}]"),
                pos,
                e,
                &role,
                &top_desc,
                single,
            )
        })
        .collect();
    ExpParts {
        top: Some(top.join("\n")).filter(|t| !t.is_empty()),
        positions,
    }
}

/// Blocks of one position: header (title and range) + body as one block, then the project blocks.
pub fn pos_blocks(rep: &Report, pos: &PosParts, header: bool) -> Vec<String> {
    let mut lines = Vec::new();
    if header && !pos.header_redundant {
        if let Some(t) = &pos.title {
            let mut h = t.clone();
            if let Some(r) = range_text(rep, pos.start, pos.end, true) {
                h.push_str(&format!(" ({r})"));
            }
            lines.push(h);
        }
    }
    lines.extend(pos.body.iter().cloned());
    let mut out = Vec::new();
    if !lines.is_empty() {
        out.push(lines.join("\n"));
    }
    out.extend(pos.projects.iter().cloned());
    out
}

fn is_esco_occupation(s: &str) -> bool {
    let Some(id) = s.strip_prefix("http://data.europa.eu/esco/occupation/") else {
        return false;
    };
    let parts: Vec<&str> = id.split('-').collect();
    let lens = [8, 4, 4, 4, 12];
    parts.len() == 5
        && parts
            .iter()
            .zip(lens)
            .all(|(p, l)| p.len() == l && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// `ESCO: <iri>` line when the option is on and the IRI is a well formed occupation IRI.
pub fn esco_line(rep: &Report, e: &crate::input::Experience) -> Option<String> {
    if !rep.opts.include_esco {
        return None;
    }
    e.esco_occupation
        .as_deref()
        .map(str::trim)
        .filter(|s| is_esco_occupation(s))
        .map(|iri| format!("ESCO: {iri}"))
}

/// The whole experience as one text (positions in order).
pub fn full_text(rep: &Report, parts: &ExpParts, e: &crate::input::Experience) -> String {
    let mut blocks: Vec<String> = Vec::new();
    blocks.extend(parts.top.iter().cloned());
    for pos in &parts.positions {
        blocks.extend(pos_blocks(rep, pos, true));
    }
    let mut text = blocks.join("\n\n");
    if let Some(l) = esco_line(rep, e) {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&l);
    }
    text
}

/// Compose `WorkExperience/Activities` (one entry for the whole experience).
pub fn compose_experience(
    rep: &mut Report,
    path: &str,
    e: &crate::input::Experience,
) -> Option<String> {
    let parts = exp_parts(rep, path, e);
    let text = full_text(rep, &parts, e);
    finish(rep, path, text)
}

/// Clamp a composed blob to `maxActivitiesChars`. The cut falls on a block boundary (blank line), else a
/// line boundary, and a marker line says that content was omitted; only a single huge line is cut inside.
pub fn finish(rep: &mut Report, path: &str, text: String) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    let max = rep.opts.max_activities_chars;
    let n = text.chars().count();
    if n <= max {
        return Some(text);
    }
    rep.warn(
        "text_truncated",
        path,
        format!("composed text of {n} characters cut to {max}"),
    );
    let marker = rep.locale.t(Key::Omitted);
    let budget = max.saturating_sub(marker.chars().count() + 2);
    for sep in ["\n\n", "\n"] {
        let cut = text
            .match_indices(sep)
            .map(|(i, _)| i)
            .filter(|&i| i > 0 && text.get(..i).is_some_and(|h| h.chars().count() <= budget))
            .last();
        if let Some(i) = cut {
            let head = text.get(..i).unwrap_or("").trim_end();
            return Some(format!("{head}\n\n{marker}"));
        }
    }
    Some(crate::text::clamp(&text, max, true).0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::i18n::Locale;
    use crate::input::*;
    use crate::options::Options;

    fn rep() -> Report {
        Report::new(Options::default(), Locale::En)
    }

    #[test]
    fn telco_shape_prints_project_block_only() {
        let e = Experience {
            id: "p".into(),
            positions: vec![PositionEntry {
                title: Some("".into()),
                projects: vec![Project {
                    name: "Audit".into(),
                    role: Some("Architect".into()),
                    start: Some("2025-11-01".into()),
                    end: Some("2026-02-28".into()),
                    description: Some("Technical audit.".into()),
                    bullets: vec!["One".into(), "Two".into()],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let t = compose_experience(&mut rep(), "e", &e).unwrap();
        assert_eq!(
            t,
            "Audit - Architect (2025-11 - 2026-02)\nTechnical audit.\n- One\n- Two"
        );
    }

    #[test]
    fn hr_ate_shape_four_positions_no_text() {
        let pos = |t: &str, s: &str, e: &str| PositionEntry {
            title: Some(t.into()),
            start: Some(s.into()),
            end: Some(e.into()),
            ..Default::default()
        };
        let e = Experience {
            id: "cm".into(),
            positions: vec![
                pos("Lead DevSecOps", "2017-06-02", "2023-09-30"),
                pos("Lead DevOps", "2011-06-02", "2017-06-01"),
            ],
            ..Default::default()
        };
        let t = compose_experience(&mut rep(), "e", &e).unwrap();
        assert_eq!(
            t,
            "Lead DevSecOps (2017-06 - 2023-09)\n\nLead DevOps (2011-06 - 2017-06)"
        );
    }

    #[test]
    fn open_end_and_invalid_end() {
        let mut r = rep();
        let p = |end: Option<&str>| Project {
            name: "X".into(),
            start: Some("2020-01-01".into()),
            end: end.map(Into::into),
            ..Default::default()
        };
        assert_eq!(
            project_block(&mut r, "p", &p(None), false, &Known::default()).unwrap(),
            "X (2020-01 - present)"
        );
        assert_eq!(
            project_block(
                &mut r,
                "p",
                &p(Some("20014-12-01")),
                false,
                &Known::default()
            )
            .unwrap(),
            "X (2020-01)"
        );
        assert_eq!(r.issues.len(), 1);
        assert_eq!(r.issues[0].code, "date_invalid");
        let mut fr = Report::new(Options::default(), Locale::Fr);
        assert_eq!(
            project_block(&mut fr, "p", &p(None), false, &Known::default()).unwrap(),
            "X (2020-01 - aujourd'hui)"
        );
    }

    #[test]
    fn skills_dedup_and_prefix() {
        let sk = vec![
            SkillRef::Plain("Rust".into()),
            SkillRef::Obj {
                label: "rust".into(),
            },
            SkillRef::Plain("Go".into()),
        ];
        assert_eq!(
            skills_line(&mut rep(), "s", &sk).unwrap(),
            "Skills: Rust, Go"
        );
        let mut fr = Report::new(Options::default(), Locale::Fr);
        assert_eq!(
            skills_line(&mut fr, "s", &sk).unwrap(),
            "Compétences : Rust, Go"
        );
    }

    #[test]
    fn clamp_applies() {
        let mut r = rep();
        r.opts.max_activities_chars = 30;
        let e = Experience {
            description: Some("word ".repeat(30)),
            ..Default::default()
        };
        let t = compose_experience(&mut r, "e", &e).unwrap();
        assert!(t.chars().count() <= 30);
        assert!(t.ends_with('\u{2026}'));
        assert!(r.issues.iter().any(|i| i.code == "text_truncated"));
    }

    #[test]
    fn esco_option() {
        let iri = "http://data.europa.eu/esco/occupation/2bef94db-0088-4507-982a-2ca717529adb";
        let e = Experience {
            description: Some("d".into()),
            esco_occupation: Some(iri.into()),
            ..Default::default()
        };
        assert_eq!(compose_experience(&mut rep(), "e", &e).unwrap(), "d");
        let mut r = rep();
        r.opts.include_esco = true;
        assert_eq!(
            compose_experience(&mut r, "e", &e).unwrap(),
            format!("d\nESCO: {iri}")
        );
        let bad = Experience {
            description: Some("d".into()),
            esco_occupation: Some("http://x".into()),
            ..Default::default()
        };
        assert_eq!(compose_experience(&mut r, "e", &bad).unwrap(), "d");
    }

    #[test]
    fn near_duplicate_is_fuzzy_but_not_loose() {
        let a = "Enterprise Architect (part-time) mission within Dataspace-Y, the European sovereign data space covering 4 verticals: transport, mobility, tourism and logistics.";
        let b = "Enterprise Architect (part-time) mission within Dataspace-Y, the European sovereign data space covering transport, mobility, tourism and logistics.";
        assert!(near_duplicate(a, b));
        assert!(!near_duplicate(
            a,
            "Migration of the DB2 warehouse from z/OS to Linux with a new delivery pipeline."
        ));
    }

    #[test]
    fn project_repeating_role_and_description_is_slimmed() {
        let known = Known {
            roles: vec!["Senior Architect".into()],
            descs: vec!["Technical audit of the whole platform and its pipelines.".into()],
        };
        let p = Project {
            name: "Audit".into(),
            role: Some("senior architect".into()),
            description: Some("Technical audit of the whole platform and its pipelines.".into()),
            ..Default::default()
        };
        assert_eq!(
            project_block(&mut rep(), "p", &p, false, &known).unwrap(),
            "Audit"
        );
    }

    #[test]
    fn project_outside_parent_period_loses_its_dates() {
        let mut r = rep();
        let p = Project {
            name: "Stage".into(),
            start: Some("2022-10-02".into()),
            end: Some("2023-09-30".into()),
            ..Default::default()
        };
        let parent = (
            crate::date::parse_iso_date("1996-09-01"),
            crate::date::parse_iso_date("1998-06-30"),
        );
        let c = clip_project(&mut r, "education[0].projects[0]", &p, parent.0, parent.1);
        assert!(c.start.is_none() && c.end.is_none());
        assert_eq!(r.issues[0].code, "project_outside_parent_period");
        let inside = Project {
            name: "X".into(),
            start: Some("1997-01".into()),
            end: Some("1998-06".into()),
            ..Default::default()
        };
        assert_eq!(
            clip_project(&mut r, "p", &inside, parent.0, parent.1).start,
            inside.start
        );
        assert_eq!(r.issues.len(), 1);
    }

    #[test]
    fn truncation_cuts_on_a_block_boundary_and_says_so() {
        let mut r = rep();
        r.opts.max_activities_chars = 80;
        let text = "first block line one\nline two\n\nsecond block that is long enough to overflow the limit\n\nthird".to_string();
        let t = finish(&mut r, "e", text).unwrap();
        assert!(t.chars().count() <= 80);
        assert_eq!(
            t,
            "first block line one\nline two\n\n[Remainder omitted: length limit]"
        );
        assert!(r.issues.iter().any(|i| i.code == "text_truncated"));
    }
}
