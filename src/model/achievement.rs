//! Achievement.xsd AchievementType: `Title?, Description?, Documentation?`.
use super::label::Label;
use crate::xml::node::{opt_text, Node};

#[derive(Debug, Clone)]
pub struct Achievement {
    pub title: Label,
    pub description: Option<String>,
}

impl Achievement {
    pub fn to_node(&self) -> Option<Node> {
        Node::parent(
            "Achievement",
            vec![
                self.title.to_node("Title"),
                opt_text("Description", &self.description),
            ],
        )
    }
}
