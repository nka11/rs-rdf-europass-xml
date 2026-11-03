//! Pure core: `export()` is testable natively without wasm.
use crate::error::{ExportError, Warning};
use crate::input::{self, CvInput};
use crate::options::Options;

#[derive(Debug, Clone)]
pub struct Export {
    pub xml: String,
    pub warnings: Vec<Warning>,
}

/// Hard-error check plus mapping to the Candidate XML.
pub fn export(input: &CvInput, opts: &Options) -> Result<Export, ExportError> {
    crate::candidate::export(input, opts)
}

pub fn export_json(json: &str, opts: &Options) -> Result<Export, ExportError> {
    let input = input::parse(json)?;
    export(&input, opts)
}
