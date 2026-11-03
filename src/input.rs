//! Input structs (DESIGN section 3) and the hard-error pre-check.
use crate::date::valid_datetime;
use crate::error::{ExportError, Issue};
use crate::i18n::Locale;
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

/// `null` is the same as absent for every field (lists, strings, booleans, options).
fn nd<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

pub const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CvInput {
    #[serde(default, deserialize_with = "nd")]
    pub schema_version: Option<Value>,
    #[serde(default, deserialize_with = "nd")]
    pub lang: String,
    #[serde(default, deserialize_with = "nd")]
    pub person: Option<Person>,
    #[serde(default, deserialize_with = "nd")]
    pub languages: Vec<LanguageEntry>,
    #[serde(default, deserialize_with = "nd")]
    pub experiences: Vec<Experience>,
    #[serde(default, deserialize_with = "nd")]
    pub education: Vec<EducationEntry>,
    #[serde(default, deserialize_with = "nd")]
    pub skills: Vec<SkillEntry>,
    #[serde(default, deserialize_with = "nd")]
    pub projects: Vec<Project>,
    #[serde(default, deserialize_with = "nd")]
    pub meta: Option<Meta>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub headline: Option<String>,
    pub summary: Option<String>,
    pub email: Option<String>,
    #[serde(default, deserialize_with = "nd")]
    pub websites: Vec<Website>,
    pub address: Option<AddressIn>,
    #[serde(default, deserialize_with = "nd")]
    pub phones: Vec<Phone>,
    pub birth_date: Option<String>,
    pub gender: Option<String>,
    #[serde(default, deserialize_with = "nd")]
    pub nationalities: Vec<Nationality>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Website {
    #[serde(default, deserialize_with = "nd")]
    pub url: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddressIn {
    pub line1: Option<String>,
    pub line2: Option<String>,
    pub postal_code: Option<String>,
    pub municipality: Option<String>,
    pub country_code: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Phone {
    #[serde(default, deserialize_with = "nd")]
    pub number: String,
    #[serde(rename = "use")]
    pub use_: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Nationality {
    #[serde(default, deserialize_with = "nd")]
    pub code: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Cef {
    pub listening: Option<String>,
    pub reading: Option<String>,
    pub spoken_interaction: Option<String>,
    pub spoken_production: Option<String>,
    pub writing: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LanguageEntry {
    #[serde(default, deserialize_with = "nd")]
    pub label: String,
    pub level: Option<String>,
    pub code: Option<String>,
    #[serde(default, deserialize_with = "nd")]
    pub mother_tongue: bool,
    pub cef: Option<Cef>,
}

/// A skill reference: a plain string or `{ "label": ... }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SkillRef {
    Plain(String),
    Obj {
        #[serde(default, deserialize_with = "nd")]
        label: String,
    },
}

impl SkillRef {
    pub fn label(&self) -> &str {
        match self {
            SkillRef::Plain(s) => s,
            SkillRef::Obj { label } => label,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Experience {
    #[serde(default, deserialize_with = "nd")]
    pub id: String,
    pub role: Option<String>,
    pub organization: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub current: Option<bool>,
    pub url: Option<String>,
    pub description: Option<String>,
    #[serde(default, deserialize_with = "nd")]
    pub bullets: Vec<String>,
    pub esco_occupation: Option<String>,
    #[serde(default, deserialize_with = "nd")]
    pub skills: Vec<SkillRef>,
    #[serde(default, deserialize_with = "nd")]
    pub positions: Vec<PositionEntry>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PositionEntry {
    pub title: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub description: Option<String>,
    #[serde(default, deserialize_with = "nd")]
    pub bullets: Vec<String>,
    #[serde(default, deserialize_with = "nd")]
    pub projects: Vec<Project>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    #[serde(default, deserialize_with = "nd")]
    pub name: String,
    pub role: Option<String>,
    pub description: Option<String>,
    #[serde(default, deserialize_with = "nd")]
    pub bullets: Vec<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub url: Option<String>,
    #[serde(default, deserialize_with = "nd")]
    pub skills: Vec<SkillRef>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EducationEntry {
    #[serde(default, deserialize_with = "nd")]
    pub title: String,
    pub school: Option<String>,
    pub city: Option<String>,
    pub level_label: Option<String>,
    pub eqf: Option<Number>,
    pub start: Option<String>,
    pub end: Option<String>,
    #[serde(default, deserialize_with = "nd")]
    pub projects: Vec<Project>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SkillEntry {
    #[serde(default, deserialize_with = "nd")]
    pub label: String,
    pub category: Option<String>,
    pub uses: Option<Number>,
    pub esco: Option<String>,
    pub esco_match: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    #[serde(default, deserialize_with = "nd")]
    pub generated_at: String,
    pub generator: Option<String>,
    pub variant: Option<String>,
    pub source_url: Option<String>,
    #[serde(default, deserialize_with = "nd")]
    pub omitted: Vec<String>,
    #[serde(default, deserialize_with = "nd")]
    pub untagged: Vec<String>,
}

/// serde's derive also accepts a JSON array where a struct is expected (as a positional sequence).
/// That would silently turn `"person": []` into an empty person, so structural positions are checked first.
fn shape_check(v: &Value) -> Result<(), String> {
    fn obj(v: Option<&Value>, path: &str) -> Result<(), String> {
        match v {
            None | Some(Value::Null) | Some(Value::Object(_)) => Ok(()),
            Some(_) => Err(format!("{path} must be a JSON object")),
        }
    }
    fn objs<'a>(v: Option<&'a Value>, path: &str) -> Result<Vec<&'a Value>, String> {
        match v {
            None | Some(Value::Null) => Ok(vec![]),
            Some(Value::Array(a)) => {
                for x in a {
                    obj(Some(x), &format!("{path}[]"))?;
                }
                Ok(a.iter().collect())
            }
            Some(_) => Err(format!("{path} must be a JSON array")),
        }
    }
    let Value::Object(root) = v else {
        return Err("the top level must be a JSON object".into());
    };
    obj(root.get("person"), "person")?;
    obj(root.get("meta"), "meta")?;
    if let Some(Value::Object(p)) = root.get("person") {
        obj(p.get("address"), "person.address")?;
        for k in ["websites", "phones", "nationalities"] {
            objs(p.get(k), &format!("person.{k}"))?;
        }
    }
    for l in objs(root.get("languages"), "languages")? {
        obj(l.get("cef"), "languages[].cef")?;
    }
    for e in objs(root.get("experiences"), "experiences")? {
        for pos in objs(e.get("positions"), "experiences[].positions")? {
            objs(pos.get("projects"), "experiences[].positions[].projects")?;
        }
    }
    for e in objs(root.get("education"), "education")? {
        objs(e.get("projects"), "education[].projects")?;
    }
    objs(root.get("skills"), "skills")?;
    objs(root.get("projects"), "projects")?;
    Ok(())
}

/// Parse the JSON text. Size is checked first, wrong container types are `json_syntax`.
pub fn parse(json: &str) -> Result<CvInput, ExportError> {
    if json.len() > MAX_INPUT_BYTES {
        return Err(ExportError::InputTooLarge { bytes: json.len() });
    }
    let value: Value = serde_json::from_str(json).map_err(|e| ExportError::JsonSyntax {
        line: e.line(),
        col: e.column(),
        msg: e.to_string(),
    })?;
    shape_check(&value).map_err(|msg| ExportError::JsonSyntax {
        line: 1,
        col: 1,
        msg,
    })?;
    serde_json::from_value::<CvInput>(value).map_err(|e| ExportError::JsonSyntax {
        line: e.line(),
        col: e.column(),
        msg: e.to_string(),
    })
}

/// All hard errors of a parsed input, in a fixed order. `export` fails with the first.
pub fn hard_errors(input: &CvInput) -> Vec<ExportError> {
    let mut v = Vec::new();
    match &input.schema_version {
        Some(Value::Number(n)) if n.as_i64() == Some(1) => {}
        Some(other) => v.push(ExportError::UnsupportedSchemaVersion {
            got: other.to_string(),
        }),
        None => v.push(ExportError::UnsupportedSchemaVersion {
            got: "missing".to_string(),
        }),
    }
    if Locale::parse(&input.lang).is_none() {
        v.push(ExportError::InvalidLang {
            got: input.lang.clone(),
        });
    }
    if input.person.is_none() {
        v.push(ExportError::MissingField { path: "person" });
    }
    match &input.meta {
        None => v.push(ExportError::MissingField { path: "meta" }),
        Some(m) if m.generated_at.trim().is_empty() => v.push(ExportError::MissingField {
            path: "meta.generatedAt",
        }),
        Some(m) if !valid_datetime(m.generated_at.trim()) => {
            v.push(ExportError::InvalidGeneratedAt {
                got: m.generated_at.clone(),
            })
        }
        Some(_) => {}
    }
    v
}

/// Result of `validateCvInput`.
#[derive(Debug, Clone, Serialize)]
pub struct Validation {
    pub ok: bool,
    pub errors: Vec<Issue>,
    pub warnings: Vec<Issue>,
}

/// Validate the input only: errors are the hard errors, warnings are what the mapping reports.
/// Runs the exporter itself so the two can never disagree.
pub fn validate_json(json: &str) -> Validation {
    let input = match parse(json) {
        Ok(i) => i,
        Err(e) => {
            return Validation {
                ok: false,
                errors: vec![e.to_issue()],
                warnings: vec![],
            }
        }
    };
    let errs = hard_errors(&input);
    if !errs.is_empty() {
        return Validation {
            ok: false,
            errors: errs.iter().map(|e| e.to_issue()).collect(),
            warnings: vec![],
        };
    }
    match crate::export::export(&input, &crate::options::Options::default()) {
        Ok(e) => Validation {
            ok: true,
            errors: vec![],
            warnings: e.warnings,
        },
        Err(e) => Validation {
            ok: false,
            errors: vec![e.to_issue()],
            warnings: vec![],
        },
    }
}
