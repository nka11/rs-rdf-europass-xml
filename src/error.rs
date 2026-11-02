//! Hard errors and the shared `Issue` shape (DESIGN section 9).
use serde::Serialize;
use std::fmt;

/// Error or warning shape shared by `validateCvInput` and the export report.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Issue {
    pub code: String,
    pub path: String,
    pub message: String,
}

impl Issue {
    pub fn new(code: &str, path: &str, message: impl Into<String>) -> Self {
        Issue {
            code: code.to_string(),
            path: path.to_string(),
            message: message.into(),
        }
    }
}

/// Alias used for warnings.
pub type Warning = Issue;

/// Errors that make a valid document impossible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError {
    JsonSyntax {
        line: usize,
        col: usize,
        msg: String,
    },
    InputTooLarge {
        bytes: usize,
    },
    UnsupportedSchemaVersion {
        got: String,
    },
    InvalidLang {
        got: String,
    },
    MissingField {
        path: &'static str,
    },
    InvalidGeneratedAt {
        got: String,
    },
    BadOptions {
        msg: String,
    },
}

/// Truncate a value for messages (60 chars, single line).
pub fn short(v: &str) -> String {
    let one: String = v
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if one.chars().count() > 60 {
        let mut s: String = one.chars().take(59).collect();
        s.push('\u{2026}');
        s
    } else {
        one
    }
}

impl ExportError {
    pub fn code(&self) -> &'static str {
        match self {
            ExportError::JsonSyntax { .. } => "json_syntax",
            ExportError::InputTooLarge { .. } => "input_too_large",
            ExportError::UnsupportedSchemaVersion { .. } => "unsupported_schema_version",
            ExportError::InvalidLang { .. } => "invalid_lang",
            ExportError::MissingField { .. } => "missing_field",
            ExportError::InvalidGeneratedAt { .. } => "invalid_generated_at",
            ExportError::BadOptions { .. } => "bad_options",
        }
    }

    pub fn path(&self) -> String {
        match self {
            ExportError::JsonSyntax { .. } => "$".to_string(),
            ExportError::InputTooLarge { .. } => "$".to_string(),
            ExportError::UnsupportedSchemaVersion { .. } => "schemaVersion".to_string(),
            ExportError::InvalidLang { .. } => "lang".to_string(),
            ExportError::MissingField { path } => (*path).to_string(),
            ExportError::InvalidGeneratedAt { .. } => "meta.generatedAt".to_string(),
            ExportError::BadOptions { .. } => "options".to_string(),
        }
    }

    pub fn message(&self) -> String {
        match self {
            ExportError::JsonSyntax { line, col, msg } => {
                format!("invalid JSON at line {line} column {col}: {}", short(msg))
            }
            ExportError::InputTooLarge { bytes } => {
                format!("input is {bytes} bytes, the limit is 4194304")
            }
            ExportError::UnsupportedSchemaVersion { got } => {
                format!("schemaVersion must be the integer 1, got {}", short(got))
            }
            ExportError::InvalidLang { got } => {
                format!("lang must be \"en\" or \"fr\", got \"{}\"", short(got))
            }
            ExportError::MissingField { path } => format!("required field {path} is missing"),
            ExportError::InvalidGeneratedAt { got } => {
                format!("meta.generatedAt must be an xsd:dateTime such as 2026-11-02T10:00:00Z, got \"{}\"", short(got))
            }
            ExportError::BadOptions { msg } => format!("bad options: {}", short(msg)),
        }
    }

    pub fn to_issue(&self) -> Issue {
        Issue::new(self.code(), &self.path(), self.message())
    }
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code(), self.message())
    }
}

impl std::error::Error for ExportError {}
