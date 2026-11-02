//! EmploymentExperience.xsd / Experience.xsd / Organisation.xsd.
use super::label::Period;
use crate::xml::node::{opt_text, Node};

#[derive(Debug, Clone, Default)]
pub struct Employer {
    pub name: Option<String>,
    pub website: Option<String>,
}

impl Employer {
    // Organisation.xsd EmployerType: Name?, ContactInfo?, Sector? ; ContactInfo: Address?, Website? (single)
    pub fn to_node(&self) -> Option<Node> {
        let site = self.website.as_ref().and_then(|w| {
            Node::parent(
                "ContactInfo",
                vec![Node::parent(
                    "Website",
                    vec![Some(Node::text("Contact", w.clone()))],
                )],
            )
        });
        Node::parent("Employer", vec![opt_text("Name", &self.name), site])
    }
}

#[derive(Debug, Clone, Default)]
pub struct WorkExperience {
    pub period: Option<Period>,
    pub position: Option<String>,
    pub activities: Option<String>,
    pub employer: Option<Employer>,
}

impl WorkExperience {
    // Experience.xsd ExperienceType: Period?, Description?, Documentation?, then Position?, Activities?, Employer?
    pub fn to_node(&self) -> Option<Node> {
        Node::parent(
            "WorkExperience",
            vec![
                self.period.as_ref().and_then(|p| p.to_node()),
                self.position.as_ref().and_then(|p| {
                    Node::parent("Position", vec![Some(Node::text("Label", p.clone()))])
                }),
                opt_text("Activities", &self.activities),
                self.employer.as_ref().and_then(|e| e.to_node()),
            ],
        )
    }
}
