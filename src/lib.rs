//! CV JSON to the Europass Candidate XML (the format the europass.europa.eu editor imports).
//!
//! The wasm-facing functions are thin string-in, string-out wrappers; all logic is in
//! [`export::export`], which is testable natively.
pub mod candidate;
pub mod date;
pub mod error;
pub mod export;
pub mod i18n;
pub mod input;
pub mod map;
pub mod options;
pub mod text;
pub mod xml;

use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct ReportOut<'a> {
    xml: &'a str,
    warnings: &'a [error::Warning],
}

fn js_err(e: error::ExportError) -> JsError {
    JsError::new(&e.to_string())
}

/// Main entry. Returns the XML text; warnings are discarded.
#[wasm_bindgen(js_name = exportEuropassXml)]
pub fn export_europass_xml(
    input_json: &str,
    options_json: Option<String>,
) -> Result<String, JsError> {
    let opts = options::Options::from_json(options_json.as_deref()).map_err(js_err)?;
    export::export_json(input_json, &opts)
        .map(|e| e.xml)
        .map_err(js_err)
}

/// Same, returning `{"xml": "...", "warnings": [...]}`.
#[wasm_bindgen(js_name = exportEuropassXmlReport)]
pub fn export_europass_xml_report(
    input_json: &str,
    options_json: Option<String>,
) -> Result<String, JsError> {
    let opts = options::Options::from_json(options_json.as_deref()).map_err(js_err)?;
    let e = export::export_json(input_json, &opts).map_err(js_err)?;
    serde_json::to_string(&ReportOut {
        xml: &e.xml,
        warnings: &e.warnings,
    })
    .map_err(|e| JsError::new(&format!("[internal] {e}")))
}

/// Validates the INPUT only. Never throws.
#[wasm_bindgen(js_name = validateCvInput)]
pub fn validate_cv_input(input_json: &str) -> String {
    serde_json::to_string(&input::validate_json(input_json))
        .unwrap_or_else(|_| "{\"ok\":false,\"errors\":[],\"warnings\":[]}".to_string())
}

/// Static info for UI and tests.
#[wasm_bindgen(js_name = europassXmlInfo)]
pub fn europass_xml_info() -> String {
    format!(
        "{{\"crate\":\"{}\",\"format\":\"candidate\",\"schemaVersion\":1}}",
        env!("CARGO_PKG_VERSION")
    )
}
