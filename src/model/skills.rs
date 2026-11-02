//! EuropassLearnerInformation.xsd SkillSetType, LinguisticSkill.xsd, Skill.xsd, ComputerSkill.xsd.
use super::label::Label;
use crate::xml::node::Node;

#[derive(Debug, Clone, Default)]
pub struct Cefr {
    pub listening: Option<String>,
    pub reading: Option<String>,
    pub spoken_interaction: Option<String>,
    pub spoken_production: Option<String>,
    pub writing: Option<String>,
}

impl Cefr {
    pub fn is_empty(&self) -> bool {
        self.listening.is_none()
            && self.reading.is_none()
            && self.spoken_interaction.is_none()
            && self.spoken_production.is_none()
            && self.writing.is_none()
    }

    // LinguisticSkill.xsd ProficiencyLevel: Listening?, Reading?, SpokenInteraction?, SpokenProduction?, Writing?
    pub fn to_node(&self) -> Option<Node> {
        let t = |n: &'static str, v: &Option<String>| v.as_ref().map(|v| Node::text(n, v.clone()));
        Node::parent(
            "ProficiencyLevel",
            vec![
                t("Listening", &self.listening),
                t("Reading", &self.reading),
                t("SpokenInteraction", &self.spoken_interaction),
                t("SpokenProduction", &self.spoken_production),
                t("Writing", &self.writing),
            ],
        )
    }
}

#[derive(Debug, Clone)]
pub struct ForeignLanguage {
    pub description: Label,
    pub level: Option<Cefr>,
}

#[derive(Debug, Clone, Default)]
pub struct Linguistic {
    pub mother: Vec<Label>,
    pub foreign: Vec<ForeignLanguage>,
}

impl Linguistic {
    // MotherTongueList?, ForeignLanguageList?
    pub fn to_node(&self) -> Option<Node> {
        let mother = Node::parent(
            "MotherTongueList",
            self.mother
                .iter()
                .map(|m| Node::parent("MotherTongue", vec![m.to_node("Description")]))
                .collect(),
        );
        let foreign = Node::parent(
            "ForeignLanguageList",
            self.foreign
                .iter()
                .map(|f| {
                    // ForeignLanguage: Description?, ProficiencyLevel?, AcquiredDuring?, VerifiedBy?, Documentation?
                    Node::parent(
                        "ForeignLanguage",
                        vec![
                            f.description.to_node("Description"),
                            f.level.as_ref().and_then(|l| l.to_node()),
                        ],
                    )
                })
                .collect(),
        );
        Node::parent("Linguistic", vec![mother, foreign])
    }
}

fn generic(name: &'static str, text: &Option<String>) -> Option<Node> {
    text.as_ref()
        .and_then(|t| Node::parent(name, vec![Some(Node::text("Description", t.clone()))]))
}

#[derive(Debug, Clone, Default)]
pub struct SkillSet {
    pub linguistic: Option<Linguistic>,
    pub organisational: Option<String>,
    pub job_related: Option<String>,
    pub computer: Option<String>,
    pub other: Option<String>,
}

impl SkillSet {
    // SkillSetType: Linguistic, Communication, Organisational, JobRelated, Computer, Driving, Other
    // (Communication and Driving never emitted)
    pub fn to_node(&self) -> Option<Node> {
        Node::parent(
            "Skills",
            vec![
                self.linguistic.as_ref().and_then(|l| l.to_node()),
                generic("Organisational", &self.organisational),
                generic("JobRelated", &self.job_related),
                generic("Computer", &self.computer),
                generic("Other", &self.other),
            ],
        )
    }
}
