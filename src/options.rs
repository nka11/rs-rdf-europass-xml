//! Export options (DESIGN section 2.2). Invalid values are errors, unknown keys are ignored.
use crate::error::ExportError;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatePrecision {
    Year,
    Month,
    Day,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SummaryPlacement {
    Headline,
    Drop,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    pub date_precision: DatePrecision,
    pub include_esco: bool,
    pub summary_placement: SummaryPlacement,
    pub include_projects: bool,
    pub include_skills: bool,
    pub max_experiences: Option<usize>,
    pub max_activities_chars: usize,
    pub pretty: bool,
    pub schema_location_hint: bool,
    pub creation_date: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            date_precision: DatePrecision::Month,
            include_esco: false,
            summary_placement: SummaryPlacement::Headline,
            include_projects: true,
            include_skills: true,
            max_experiences: None,
            max_activities_chars: 6000,
            pretty: true,
            schema_location_hint: false,
            creation_date: None,
        }
    }
}

impl Options {
    /// Parse the options JSON. `None`, "" and "null" give the defaults.
    pub fn from_json(s: Option<&str>) -> Result<Options, ExportError> {
        let s = match s {
            Some(s) if !s.trim().is_empty() => s,
            _ => return Ok(Options::default()),
        };
        let v: serde_json::Value =
            serde_json::from_str(s).map_err(|e| ExportError::BadOptions { msg: e.to_string() })?;
        if v.is_null() {
            return Ok(Options::default());
        }
        if !v.is_object() {
            return Err(ExportError::BadOptions {
                msg: "options must be a JSON object".into(),
            });
        }
        if let Some(f) = v.get("format") {
            if f.as_str() != Some("candidate") {
                return Err(ExportError::BadOptions {
                    msg: format!("format {f}: only \"candidate\" is supported (the legacy SkillsPassport XML was removed)"),
                });
            }
        }
        let o: Options = serde_json::from_value(v)
            .map_err(|e| ExportError::BadOptions { msg: e.to_string() })?;
        if o.max_activities_chars < 20 {
            return Err(ExportError::BadOptions {
                msg: "maxActivitiesChars must be at least 20".into(),
            });
        }
        if let Some(d) = &o.creation_date {
            if !crate::date::valid_datetime(d) {
                return Err(ExportError::BadOptions {
                    msg: format!("creationDate is not an xsd:dateTime: {d}"),
                });
            }
        }
        Ok(o)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_object_options_are_rejected() {
        assert!(Options::from_json(Some("[]")).is_err());
        assert!(Options::from_json(Some("5")).is_err());
        assert!(Options::from_json(Some("null")).is_ok());
        assert!(Options::from_json(Some("{}")).is_ok());
        assert!(Options::from_json(Some(r#"{"format":"candidate"}"#)).is_ok());
        assert!(Options::from_json(Some(r#"{"format":"legacy"}"#)).is_err());
    }
}
