//! education -> EducationList (DESIGN section 4.5).
use super::activities::{clip_project, finish, project_block, Known};
use super::{Report, CAP_EDU, LIM_LINE};
use crate::error::short;
use crate::input::{CvInput, EducationEntry};
use crate::model::education::{Education, Organisation};
use crate::model::label::{Label, Period};

fn level(rep: &mut Report, path: &str, e: &EducationEntry) -> Option<Label> {
    let label = rep.opt_line(&format!("{path}.levelLabel"), &e.level_label, LIM_LINE);
    let code = e.eqf.as_ref().and_then(|n| match n.as_i64() {
        Some(v) if (1..=8).contains(&v) => Some(v),
        _ => {
            rep.warn(
                "eqf_out_of_range",
                &format!("{path}.eqf"),
                format!(
                    "EQF level must be an integer 1..8, got {}",
                    short(&n.to_string())
                ),
            );
            None
        }
    });
    let label = match (label, code) {
        (None, Some(n)) => Some(rep.locale.eqf_label(n)),
        (l, _) => l,
    };
    Some(Label::new(code.map(|c| c.to_string()), label)).filter(|l| !l.is_empty())
}

pub fn educations(input: &CvInput, rep: &mut Report) -> Vec<Education> {
    let n = rep.cap("education", &input.education, CAP_EDU);
    let mut out = Vec::new();
    for (i, e) in input.education.iter().take(n).enumerate() {
        let path = format!("education[{i}]");
        let Some(title) = rep.line(&format!("{path}.title"), &e.title, LIM_LINE) else {
            rep.warn(
                "empty_entry",
                &path,
                "education entry without title skipped".into(),
            );
            continue;
        };
        let start = rep.date(&format!("{path}.start"), &e.start);
        let end = rep.date(&format!("{path}.end"), &e.end);
        if let (super::DateField::Ok(s), super::DateField::Ok(en)) = (start, end) {
            if s.key() > en.key() {
                rep.warn("date_order", &path, "start is after end".into());
            }
        }
        let p = rep.opts.date_precision;
        let period = Period {
            from: start.ok().map(|d| d.to_attrs(p)),
            to: end.ok().map(|d| d.to_attrs(p)),
            current: false,
        };
        let blocks: Vec<String> = e
            .projects
            .iter()
            .enumerate()
            .filter_map(|(j, pr)| {
                let pp = format!("{path}.projects[{j}]");
                let pr = clip_project(rep, &pp, pr, start.ok(), end.ok());
                project_block(rep, &pp, &pr, false, &Known::default())
            })
            .collect();
        let activities = finish(rep, &format!("{path}.projects"), blocks.join("\n\n"));
        let name = rep.opt_line(&format!("{path}.school"), &e.school, LIM_LINE);
        let municipality = rep.opt_line(&format!("{path}.city"), &e.city, LIM_LINE);
        let organisation =
            Some(Organisation { name, municipality }).filter(|o| o.to_node().is_some());
        out.push(Education {
            period: Some(period).filter(|p| p.to_node().is_some()),
            title: Some(title),
            activities,
            organisation,
            level: level(rep, &path, e),
        });
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::i18n::Locale;
    use crate::options::Options;

    fn lvl(eqf: &str, label: Option<&str>) -> (Option<Label>, usize) {
        let mut rep = Report::new(Options::default(), Locale::En);
        let e = EducationEntry {
            title: "T".into(),
            eqf: serde_json::from_str(eqf).unwrap(),
            level_label: label.map(Into::into),
            ..Default::default()
        };
        (level(&mut rep, "education[0]", &e), rep.issues.len())
    }

    #[test]
    fn eqf_matrix() {
        for bad in ["0", "9", "255", "256", "9.5", "-1"] {
            let (l, w) = lvl(bad, None);
            assert!(l.is_none(), "{bad}");
            assert_eq!(w, 1, "{bad}");
        }
        let (l, w) = lvl("5", None);
        assert_eq!(
            (l.unwrap(), w),
            (Label::new(Some("5".into()), Some("EQF 5".into())), 0)
        );
        let (l, _) = lvl("9", Some("Master"));
        assert_eq!(l.unwrap(), Label::new(None, Some("Master".into())));
        let (l, _) = lvl("4", Some("Bac"));
        assert_eq!(l.unwrap(), Label::new(Some("4".into()), Some("Bac".into())));
    }
}
