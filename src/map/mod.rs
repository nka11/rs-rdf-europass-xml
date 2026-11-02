//! Input -> model. The only place that knows both sides (DESIGN section 1, rule 1).
pub mod activities;
pub mod education;
pub mod experience;
pub mod languages;
pub mod person;
pub mod projects;
pub mod skills;

use crate::date::{parse_iso_date, IsoDate};
use crate::error::{short, Issue};
use crate::i18n::Locale;
use crate::input::CvInput;
use crate::model::{DocumentInfo, LearnerInfo, SkillsPassport};
use crate::options::Options;
use crate::text::{clamp, sanitize_line, sanitize_multi};

pub const LIM_LINE: usize = 200;
pub const LIM_HEADLINE: usize = 3000;
pub const CAP_WORK: usize = 100;
pub const CAP_EDU: usize = 50;
pub const CAP_ACHIEVEMENTS: usize = 100;
pub const CAP_LANGS: usize = 30;
pub const CAP_WEBSITES: usize = 20;

/// Outcome of reading an optional date field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateField {
    Absent,
    Invalid,
    Ok(IsoDate),
}

impl DateField {
    pub fn ok(self) -> Option<IsoDate> {
        match self {
            DateField::Ok(d) => Some(d),
            _ => None,
        }
    }
}

/// Collects warnings and carries options plus locale through the mapping.
pub struct Report {
    pub issues: Vec<Issue>,
    pub opts: Options,
    pub locale: Locale,
}

impl Report {
    pub fn new(opts: Options, locale: Locale) -> Report {
        Report {
            issues: Vec::new(),
            opts,
            locale,
        }
    }

    pub fn warn(&mut self, code: &str, path: &str, msg: String) {
        self.issues.push(Issue::new(code, path, msg));
    }

    fn after_sanitize(
        &mut self,
        path: &str,
        s: crate::text::Sanitized,
        max: usize,
        ellipsis: bool,
    ) -> Option<String> {
        if s.removed_illegal > 0 {
            self.warn(
                "text_sanitized",
                path,
                format!(
                    "{} illegal XML character(s) removed from \"{}\"",
                    s.removed_illegal,
                    short(&s.text)
                ),
            );
        }
        if s.text.is_empty() {
            return None;
        }
        let (t, orig) = clamp(&s.text, max, ellipsis);
        if let Some(n) = orig {
            self.warn(
                "text_truncated",
                path,
                format!("text of {n} characters cut to {max}: \"{}\"", short(&t)),
            );
        }
        Some(t)
    }

    /// Single-line field, empty after sanitising = absent.
    pub fn line(&mut self, path: &str, s: &str, max: usize) -> Option<String> {
        let sz = sanitize_line(s);
        self.after_sanitize(path, sz, max, true)
    }

    pub fn opt_line(&mut self, path: &str, s: &Option<String>, max: usize) -> Option<String> {
        s.as_deref().and_then(|v| self.line(path, v, max))
    }

    /// Multi-line field.
    pub fn block(&mut self, path: &str, s: &str, max: usize) -> Option<String> {
        let sz = sanitize_multi(s);
        self.after_sanitize(path, sz, max, true)
    }

    pub fn opt_block(&mut self, path: &str, s: &Option<String>, max: usize) -> Option<String> {
        s.as_deref().and_then(|v| self.block(path, v, max))
    }

    /// Read an ISO date field: absent/empty, invalid (warning) or valid.
    pub fn date(&mut self, path: &str, s: &Option<String>) -> DateField {
        let Some(raw) = s.as_deref() else {
            return DateField::Absent;
        };
        let t = sanitize_line(raw).text;
        if t.is_empty() {
            return DateField::Absent;
        }
        match parse_iso_date(&t) {
            Some(d) => DateField::Ok(d),
            None => {
                self.warn(
                    "date_invalid",
                    path,
                    format!(
                        "not a valid ISO date (YYYY, YYYY-MM, YYYY-MM-DD): \"{}\"",
                        short(&t)
                    ),
                );
                DateField::Invalid
            }
        }
    }

    /// Cap a list at `cap`, warning when entries are dropped from the tail.
    pub fn cap<T>(&mut self, path: &str, v: &[T], cap: usize) -> usize {
        if v.len() > cap {
            self.warn(
                "list_truncated",
                path,
                format!("{} entries, only the first {cap} are exported", v.len()),
            );
            cap
        } else {
            v.len()
        }
    }
}

/// Build the whole model; warnings accumulate in `rep` in input traversal order.
pub fn build(
    input: &CvInput,
    rep: &mut Report,
    creation_date: String,
    locale: Locale,
) -> SkillsPassport {
    let person = input.person.clone().unwrap_or_default();
    let identification = person::identification(&person, rep);
    let headline = person::headline(&person, rep);
    // traversal order of the input: person, languages, experiences, education, skills, projects
    let linguistic = languages::linguistic(input, rep);
    let work = experience::work_experiences(input, rep);
    let education = education::educations(input, rep);
    let skill_set = skills::skill_set(input, rep, linguistic);
    let achievements = projects::achievements(input, rep);

    let meta = input.meta.clone().unwrap_or_default();
    let generator = meta
        .generator
        .as_deref()
        .and_then(|g| rep.line("meta.generator", g, LIM_LINE))
        .unwrap_or_else(|| concat!("europass-xml ", env!("CARGO_PKG_VERSION")).to_string());
    let comment = meta
        .source_url
        .as_deref()
        .and_then(|u| rep.line("meta.sourceUrl", u, 2000))
        .map(|u| {
            let mut c = format!("{} {}", locale.t(crate::i18n::Key::SourceComment), u);
            if let Some(v) = meta
                .variant
                .as_deref()
                .and_then(|v| rep.line("meta.variant", v, LIM_LINE))
            {
                c.push_str(&format!(" (variant: {v})"));
            }
            c
        });

    SkillsPassport {
        locale: locale.code().to_string(),
        schema_location_hint: rep.opts.schema_location_hint,
        document_info: DocumentInfo {
            creation_date,
            generator,
            comment,
        },
        learner_info: LearnerInfo {
            identification,
            headline,
            work,
            education,
            skills: Some(skill_set).filter(|s| !skill_set_is_empty(s)),
            achievements,
        },
    }
}

fn skill_set_is_empty(s: &crate::model::skills::SkillSet) -> bool {
    s.linguistic.is_none()
        && s.organisational.is_none()
        && s.job_related.is_none()
        && s.computer.is_none()
        && s.other.is_none()
}
