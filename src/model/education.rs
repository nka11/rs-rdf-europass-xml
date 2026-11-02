//! EducationalExperience.xsd EducationType.
use super::label::{Label, Period};
use crate::xml::node::{opt_text, Node};

#[derive(Debug, Clone, Default)]
pub struct Organisation {
    pub name: Option<String>,
    pub municipality: Option<String>,
}

impl Organisation {
    // Organisation.xsd OrganisationType: Name?, ContactInfo? (Address?, Website?)
    pub fn to_node(&self) -> Option<Node> {
        let addr = self.municipality.as_ref().and_then(|m| {
            Node::parent(
                "ContactInfo",
                vec![Node::parent(
                    "Address",
                    vec![Node::parent(
                        "Contact",
                        vec![Some(Node::text("Municipality", m.clone()))],
                    )],
                )],
            )
        });
        Node::parent("Organisation", vec![opt_text("Name", &self.name), addr])
    }
}

#[derive(Debug, Clone, Default)]
pub struct Education {
    pub period: Option<Period>,
    pub title: Option<String>,
    pub activities: Option<String>,
    pub organisation: Option<Organisation>,
    pub level: Option<Label>,
}

impl Education {
    // Period?, Description?, Documentation?, Title?, Activities?, Organisation?, Level?, Field? (Field never emitted)
    pub fn to_node(&self) -> Option<Node> {
        Node::parent(
            "Education",
            vec![
                self.period.as_ref().and_then(|p| p.to_node()),
                opt_text("Title", &self.title),
                opt_text("Activities", &self.activities),
                self.organisation.as_ref().and_then(|o| o.to_node()),
                self.level.as_ref().and_then(|l| l.to_node("Level")),
            ],
        )
    }
}
