//! Typed mirror of the Europass XSD. No knowledge of the input JSON here (DESIGN section 1, rule 1).
pub mod achievement;
pub mod education;
pub mod headline;
pub mod identification;
pub mod label;
pub mod skills;
pub mod work;

use crate::xml::node::{opt_text, Node};
use achievement::Achievement;
use education::Education;
use headline::Headline;
use identification::Identification;
use skills::SkillSet;
use work::WorkExperience;

pub const NS: &str = "http://europass.cedefop.europa.eu/Europass";
pub const SCHEMA_LOCATION: &str =
    "http://europass.cedefop.europa.eu/Europass http://europass.cedefop.europa.eu/xml/v3.4.1/EuropassSchema.xsd";

#[derive(Debug, Clone)]
pub struct DocumentInfo {
    pub creation_date: String,
    pub generator: String,
    pub comment: Option<String>,
}

impl DocumentInfo {
    // DocumentInformation.xsd: DocumentType, Bundle?, CreationDate, LastUpdateDate, XSDVersion, Generator,
    // Comment?, Copyright?, EuropassLogo?   (Bundle, Copyright, EuropassLogo never emitted)
    pub fn to_node(&self) -> Node {
        Node::parent_always(
            "DocumentInfo",
            vec![
                Some(Node::text("DocumentType", "ECV")),
                Some(Node::text("CreationDate", self.creation_date.clone())),
                Some(Node::text("LastUpdateDate", self.creation_date.clone())),
                Some(Node::text("XSDVersion", "V3.4")),
                Some(Node::text("Generator", self.generator.clone())),
                opt_text("Comment", &self.comment),
            ],
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct LearnerInfo {
    pub identification: Option<Identification>,
    pub headline: Option<Headline>,
    pub work: Vec<WorkExperience>,
    pub education: Vec<Education>,
    pub skills: Option<SkillSet>,
    pub achievements: Vec<Achievement>,
}

impl LearnerInfo {
    // EuropassLearnerInformation.xsd LearnerInfoType:
    // Identification, Headline, WorkExperienceList, EducationList, Skills, AchievementList, Documentation
    pub fn to_node(&self) -> Node {
        Node::parent_always(
            "LearnerInfo",
            vec![
                self.identification.as_ref().and_then(|i| i.to_node()),
                self.headline.as_ref().and_then(|h| h.to_node()),
                Node::parent(
                    "WorkExperienceList",
                    self.work.iter().map(|w| w.to_node()).collect(),
                ),
                Node::parent(
                    "EducationList",
                    self.education.iter().map(|e| e.to_node()).collect(),
                ),
                self.skills.as_ref().and_then(|s| s.to_node()),
                Node::parent(
                    "AchievementList",
                    self.achievements.iter().map(|a| a.to_node()).collect(),
                ),
            ],
        )
    }
}

#[derive(Debug, Clone)]
pub struct SkillsPassport {
    pub locale: String,
    pub schema_location_hint: bool,
    pub document_info: DocumentInfo,
    pub learner_info: LearnerInfo,
}

impl SkillsPassport {
    // EuropassSchema.xsd: DocumentInfo, PrintingPreferences?, LearnerInfo, AttachmentList?, CoverLetter?
    pub fn to_node(&self) -> Node {
        let mut attrs: Vec<(&'static str, String)> = vec![
            ("xmlns", NS.to_string()),
            (
                "xmlns:xsi",
                "http://www.w3.org/2001/XMLSchema-instance".to_string(),
            ),
        ];
        if self.schema_location_hint {
            attrs.push(("xsi:schemaLocation", SCHEMA_LOCATION.to_string()));
        }
        attrs.push(("locale", self.locale.clone()));
        Node::parent_always(
            "SkillsPassport",
            vec![
                Some(self.document_info.to_node()),
                Some(self.learner_info.to_node()),
            ],
        )
        .with_attrs(attrs)
    }
}
