//! languages -> Skills/Linguistic (DESIGN section 4.6).
use super::{Report, CAP_LANGS, LIM_LINE};
use crate::codesets::{FOREIGN_LANGS, MOTHER_LANGS};
use crate::error::short;
use crate::input::{Cef, CvInput};
use crate::model::label::Label;
use crate::model::skills::{Cefr, ForeignLanguage, Linguistic};

const CEFR: [&str; 6] = ["A1", "A2", "B1", "B2", "C1", "C2"];

fn level(rep: &mut Report, path: &str, v: &Option<String>) -> Option<String> {
    let raw = v.as_deref()?.trim();
    if raw.is_empty() {
        return None;
    }
    let up = raw.to_ascii_uppercase();
    if CEFR.contains(&up.as_str()) {
        Some(up)
    } else {
        rep.warn(
            "cef_invalid",
            path,
            format!("CEFR level must be A1..C2, got \"{}\"", short(raw)),
        );
        None
    }
}

fn cefr(rep: &mut Report, path: &str, c: &Cef) -> Cefr {
    Cefr {
        listening: level(rep, &format!("{path}.listening"), &c.listening),
        reading: level(rep, &format!("{path}.reading"), &c.reading),
        spoken_interaction: level(
            rep,
            &format!("{path}.spokenInteraction"),
            &c.spoken_interaction,
        ),
        spoken_production: level(
            rep,
            &format!("{path}.spokenProduction"),
            &c.spoken_production,
        ),
        writing: level(rep, &format!("{path}.writing"), &c.writing),
    }
}

pub fn linguistic(input: &CvInput, rep: &mut Report) -> Option<Linguistic> {
    let n = rep.cap("languages", &input.languages, CAP_LANGS);
    let mut ling = Linguistic::default();
    for (i, l) in input.languages.iter().take(n).enumerate() {
        let path = format!("languages[{i}]");
        let Some(label) = rep.line(&format!("{path}.label"), &l.label, LIM_LINE) else {
            rep.warn(
                "empty_entry",
                &path,
                "language without label skipped".into(),
            );
            continue;
        };
        let set = if l.mother_tongue {
            MOTHER_LANGS
        } else {
            FOREIGN_LANGS
        };
        let code = l
            .code
            .as_deref()
            .map(|c| c.trim().to_ascii_lowercase())
            .filter(|c| !c.is_empty())
            .and_then(|c| {
                if set.binary_search(&c.as_str()).is_ok() {
                    Some(c)
                } else {
                    rep.warn(
                        "language_code_unknown",
                        &format!("{path}.code"),
                        format!("no Europass language code \"{}\" for this list", short(&c)),
                    );
                    None
                }
            });
        if l.mother_tongue {
            // the MotherTongue element already says it; "Native" would only repeat it
            ling.mother.push(Label::new(code, Some(label)));
        } else {
            let level = l
                .cef
                .as_ref()
                .map(|c| cefr(rep, &format!("{path}.cef"), c))
                .filter(|c| !c.is_empty());
            // no CEFR level can be inferred from free text, so the text itself goes into the label:
            // "English (Bilingual)". With real CEFR levels the text is dropped, the grid says it better.
            let label = match (
                &level,
                rep.opt_line(&format!("{path}.level"), &l.level, LIM_LINE),
            ) {
                (None, Some(t)) if !t.eq_ignore_ascii_case(&label) => {
                    let (joined, _) = crate::text::clamp(&format!("{label} ({t})"), LIM_LINE, true);
                    joined
                }
                _ => label,
            };
            let description = Label::new(code, Some(label));
            ling.foreign.push(ForeignLanguage { description, level });
        }
    }
    Some(ling).filter(|l| l.to_node().is_some())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::i18n::Locale;
    use crate::input::LanguageEntry;
    use crate::options::Options;

    #[test]
    fn level_text_goes_into_the_foreign_label() {
        let lang = |label: &str, level: &str, mt: bool| LanguageEntry {
            label: label.into(),
            level: Some(level.into()),
            mother_tongue: mt,
            ..Default::default()
        };
        let input = CvInput {
            languages: vec![
                lang("French", "Native", true),
                lang("English", "Bilingual", false),
            ],
            ..Default::default()
        };
        let mut rep = Report::new(Options::default(), Locale::En);
        let l = linguistic(&input, &mut rep).unwrap();
        assert_eq!(l.mother[0].label.as_deref(), Some("French"));
        assert_eq!(
            l.foreign[0].description.label.as_deref(),
            Some("English (Bilingual)")
        );
    }
}
