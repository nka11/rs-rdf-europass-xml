//! Pure core: `export()` is testable natively without wasm (DESIGN section 1, rule 3).
use crate::error::{ExportError, Issue, Warning};
use crate::i18n::Locale;
use crate::input::{self, CvInput};
use crate::map::{self, Report};
use crate::model::SkillsPassport;
use crate::options::Options;
use crate::xml::write::serialize;

#[derive(Debug, Clone)]
pub struct Export {
    pub xml: String,
    pub warnings: Vec<Warning>,
}

/// Hard-error check plus mapping. Shared by `export` and `validateCvInput`.
pub fn analyze(
    input: &CvInput,
    opts: &Options,
) -> Result<(SkillsPassport, Vec<Issue>), Vec<ExportError>> {
    let errs = input::hard_errors(input);
    if !errs.is_empty() {
        return Err(errs);
    }
    let locale = Locale::parse(&input.lang).unwrap_or(Locale::En);
    let generated = input
        .meta
        .as_ref()
        .map(|m| m.generated_at.trim().to_string())
        .unwrap_or_default();
    let creation = opts.creation_date.clone().unwrap_or(generated);
    let mut rep = Report::new(opts.clone(), locale);
    let passport = map::build(input, &mut rep, creation, locale);
    Ok((passport, rep.issues))
}

pub fn analyze_json(
    json: &str,
    opts: &Options,
) -> Result<(SkillsPassport, Vec<Issue>), Vec<ExportError>> {
    let input = input::parse(json).map_err(|e| vec![e])?;
    analyze(&input, opts)
}

pub fn export(input: &CvInput, opts: &Options) -> Result<Export, ExportError> {
    if opts.format == crate::options::Format::Candidate {
        return crate::candidate::export(input, opts);
    }
    match analyze(input, opts) {
        Ok((passport, warnings)) => Ok(Export {
            xml: serialize(&passport.to_node(), opts.pretty),
            warnings,
        }),
        Err(mut errs) => Err(errs.remove(0)),
    }
}

pub fn export_json(json: &str, opts: &Options) -> Result<Export, ExportError> {
    let input = input::parse(json)?;
    export(&input, opts)
}
