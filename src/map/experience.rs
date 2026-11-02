//! experiences -> WorkExperienceList (DESIGN section 4.3).
use super::activities::{esco_line, exp_parts, finish, full_text, pos_blocks};
use super::{DateField, Report, CAP_WORK, LIM_LINE};
use crate::error::short;
use crate::input::{CvInput, Experience};
use crate::model::label::Period;
use crate::model::work::{Employer, WorkExperience};
use crate::text::clean_uri;

fn period(rep: &mut Report, path: &str, e: &Experience) -> Option<Period> {
    let start = rep.date(&format!("{path}.start"), &e.start);
    let end = rep.date(&format!("{path}.end"), &e.end);
    // open = no end given and not explicitly finished; an INVALID end never makes the role current
    let current = e.current == Some(true) || (end == DateField::Absent && e.current != Some(false));
    if let (DateField::Ok(s), DateField::Ok(en)) = (start, end) {
        if s.key() > en.key() {
            rep.warn(
                "date_order",
                path,
                format!(
                    "start {} is after end {}",
                    s.to_text(crate::options::DatePrecision::Day),
                    en.to_text(crate::options::DatePrecision::Day)
                ),
            );
        }
    }
    let p = rep.opts.date_precision;
    let period = Period {
        from: start.ok().map(|d| d.to_attrs(p)),
        to: if current {
            None
        } else {
            end.ok().map(|d| d.to_attrs(p))
        },
        current,
    };
    Some(period).filter(|p| p.to_node().is_some())
}

pub fn work_experiences(input: &CvInput, rep: &mut Report) -> Vec<WorkExperience> {
    let mut items: &[Experience] = &input.experiences;
    if let Some(n) = rep.opts.max_experiences {
        items = items.get(..n.min(items.len())).unwrap_or(items);
    }
    let n = rep.cap("experiences", items, CAP_WORK);
    let mut out = Vec::new();
    for (i, e) in items.iter().take(n).enumerate() {
        let path = format!("experiences[{i}]");
        let mut position = rep.opt_line(&format!("{path}.role"), &e.role, LIM_LINE);
        if position.is_none() {
            if let Some(t) = e.positions.first().and_then(|p| p.title.clone()) {
                position = rep.line(&format!("{path}.positions[0].title"), &t, LIM_LINE);
            }
        }
        let period = period(rep, &path, e);
        let parts = exp_parts(rep, &path, e);
        let full = full_text(rep, &parts, e);
        let name = rep.opt_line(&format!("{path}.organization"), &e.organization, LIM_LINE);
        let website = rep
            .opt_line(&format!("{path}.url"), &e.url, 2000)
            .and_then(|u| {
                let c = clean_uri(&u);
                if c.is_none() {
                    rep.warn(
                        "url_invalid",
                        &format!("{path}.url"),
                        format!("not an http, https or ftp URL: \"{}\"", short(&u)),
                    );
                }
                c
            });
        let employer = Some(Employer { name, website }).filter(|e| e.to_node().is_some());
        if full.chars().count() > rep.opts.max_activities_chars && parts.positions.len() >= 2 {
            // too long for one Activities element: one WorkExperience per credential, each with its own
            // Position and Period, instead of cutting the oldest roles away
            rep.warn(
                "experience_split",
                &path,
                format!(
                    "experience \"{}\" is too long for one entry and was split into {} entries",
                    short(&e.id),
                    parts.positions.len()
                ),
            );
            let p = rep.opts.date_precision;
            for (k, pos) in parts.positions.iter().enumerate() {
                let mut blocks: Vec<String> = Vec::new();
                if k == 0 {
                    blocks.extend(parts.top.iter().cloned());
                }
                blocks.extend(pos_blocks(rep, pos, false));
                let mut text = blocks.join("\n\n");
                if k == 0 {
                    if let Some(l) = esco_line(rep, e) {
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(&l);
                    }
                }
                let activities = finish(rep, &format!("{path}.positions[{k}]"), text);
                let own = Period {
                    from: pos.start.ok().map(|d| d.to_attrs(p)),
                    to: pos.end.ok().map(|d| d.to_attrs(p)),
                    current: k == 0
                        && pos.end == DateField::Absent
                        && period.as_ref().is_some_and(|x| x.current),
                };
                let own = Some(own).filter(|x| x.to_node().is_some()).or_else(|| {
                    if k == 0 {
                        period.clone()
                    } else {
                        None
                    }
                });
                let w = WorkExperience {
                    period: own,
                    position: pos.title.clone().or_else(|| position.clone()),
                    activities,
                    employer: employer.clone(),
                };
                out.push(w);
            }
            continue;
        }
        let activities = finish(rep, &path, full);
        let w = WorkExperience {
            period,
            position,
            activities,
            employer,
        };
        if w.position.is_none() && w.employer.is_none() && w.period.is_none() {
            rep.warn(
                "empty_entry",
                &path,
                format!(
                    "experience \"{}\" has no position, employer or period, skipped",
                    short(&e.id)
                ),
            );
            continue;
        }
        out.push(w);
    }
    sort_newest_first(&mut out);
    out
}

/// Sort key of a period start: (year, month). Entries without a start keep their relative place at the end.
fn start_key(w: &WorkExperience) -> Option<(u32, u32)> {
    let f = w.period.as_ref()?.from.as_ref()?;
    let year = f.year.trim_matches('-').parse().ok()?;
    let month = f
        .month
        .as_deref()
        .map_or(0, |m| m.trim_matches('-').parse().unwrap_or(0));
    Some((year, month))
}

/// Europass lists work experience newest first. Stable, so equal starts keep the input order; this runs
/// after long experiences were split into one entry per credential.
fn sort_newest_first(out: &mut [WorkExperience]) {
    out.sort_by(|a, b| match (start_key(a), start_key(b)) {
        (Some(x), Some(y)) => y.cmp(&x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::i18n::Locale;
    use crate::options::Options;

    fn run(e: Experience) -> (Vec<WorkExperience>, Vec<String>) {
        let input = CvInput {
            experiences: vec![e],
            ..Default::default()
        };
        let mut rep = Report::new(Options::default(), Locale::En);
        let w = work_experiences(&input, &mut rep);
        (w, rep.issues.iter().map(|i| i.code.clone()).collect())
    }

    fn exp(start: Option<&str>, end: Option<&str>, current: Option<bool>) -> Experience {
        Experience {
            id: "x".into(),
            role: Some("R".into()),
            start: start.map(Into::into),
            end: end.map(Into::into),
            current,
            ..Default::default()
        }
    }

    #[test]
    fn open_role_is_current_without_to() {
        let (w, _) = run(exp(Some("2020-01-01"), None, None));
        let p = w[0].period.as_ref().unwrap();
        assert!(p.current && p.to.is_none() && p.from.is_some());
    }

    #[test]
    fn current_true_with_end_has_no_to() {
        let (w, _) = run(exp(Some("2020-01-01"), Some("2021-01-01"), Some(true)));
        let p = w[0].period.as_ref().unwrap();
        assert!(p.current && p.to.is_none());
    }

    #[test]
    fn invalid_end_is_neither_current_nor_to() {
        let (w, codes) = run(exp(Some("2020-01-01"), Some("20014-12-01"), None));
        let p = w[0].period.as_ref().unwrap();
        assert!(!p.current && p.to.is_none());
        assert_eq!(codes, vec!["date_invalid"]);
    }

    #[test]
    fn finished_role_and_order_warning() {
        let (w, codes) = run(exp(Some("2022-01-01"), Some("2021-01-01"), Some(false)));
        let p = w[0].period.as_ref().unwrap();
        assert!(!p.current && p.to.is_some());
        assert_eq!(codes, vec!["date_order"]);
        let (w, _) = run(exp(Some("2020-01-01"), None, Some(false)));
        assert!(!w[0].period.as_ref().unwrap().current);
    }

    #[test]
    fn empty_entry_skipped() {
        let (w, codes) = run(Experience {
            id: "e".into(),
            ..Default::default()
        });
        // no end and no current flag means open, which is still a Period: kept
        assert_eq!(w.len(), 1);
        assert!(codes.is_empty());
        let (w, codes) = run(Experience {
            id: "e".into(),
            current: Some(false),
            ..Default::default()
        });
        assert!(w.is_empty());
        assert_eq!(codes, vec!["empty_entry"]);
    }

    #[test]
    fn long_experience_is_split_per_position() {
        use crate::input::PositionEntry;
        let pos = |t: &str, s: &str, e: Option<&str>, d: &str| PositionEntry {
            title: Some(t.into()),
            start: Some(s.into()),
            end: e.map(Into::into),
            description: Some(d.into()),
            ..Default::default()
        };
        let mut e = exp(Some("2001-06-01"), None, None);
        e.organization = Some("Org".into());
        e.positions = vec![
            pos("Lead", "2011-06-01", None, &"alpha ".repeat(40)),
            pos(
                "Junior",
                "2001-06-01",
                Some("2005-06-01"),
                &"beta ".repeat(40),
            ),
        ];
        let input = CvInput {
            experiences: vec![e],
            ..Default::default()
        };
        let mut rep = Report::new(Options::default(), Locale::En);
        rep.opts.max_activities_chars = 300;
        let w = work_experiences(&input, &mut rep);
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].position.as_deref(), Some("Lead"));
        assert!(w[0].period.as_ref().unwrap().current);
        assert_eq!(w[1].position.as_deref(), Some("Junior"));
        assert!(
            !w[1].period.as_ref().unwrap().current && w[1].period.as_ref().unwrap().to.is_some()
        );
        assert!(w[1].activities.as_ref().unwrap().contains("beta"));
        assert!(rep.issues.iter().any(|i| i.code == "experience_split"));
        assert!(!rep.issues.iter().any(|i| i.code == "text_truncated"));
    }

    #[test]
    fn entries_are_sorted_newest_first() {
        let mk = |t: &str, s: &str| Experience {
            role: Some(t.into()),
            start: Some(s.into()),
            id: t.into(),
            ..Default::default()
        };
        let input = CvInput {
            experiences: vec![
                mk("old", "2001-06-01"),
                mk("new", "2023-10-01"),
                mk("mid", "2013-09-01"),
            ],
            ..Default::default()
        };
        let mut rep = Report::new(Options::default(), Locale::En);
        let w = work_experiences(&input, &mut rep);
        let names: Vec<_> = w.iter().map(|x| x.position.clone().unwrap()).collect();
        assert_eq!(names, vec!["new", "mid", "old"]);
    }
}
