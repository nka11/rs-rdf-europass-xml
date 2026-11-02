//! Headline.xsd HeadlineType: `Type?, Description?`. Description/Code is never emitted (UUID pattern).
use super::label::Label;
use crate::xml::node::Node;

#[derive(Debug, Clone)]
pub struct Headline {
    pub type_: Label,
    pub description: String,
}

impl Headline {
    pub fn to_node(&self) -> Option<Node> {
        let desc = Node::parent(
            "Description",
            vec![Some(Node::text("Label", self.description.clone()))],
        );
        Node::parent("Headline", vec![self.type_.to_node("Type"), desc])
    }
}
