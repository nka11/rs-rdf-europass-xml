//! XSD `LabelType`: `Code?, Label?` (CommonTypes.xsd).
use crate::date::DateAttrs;
use crate::xml::node::Node;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Label {
    pub code: Option<String>,
    pub label: Option<String>,
}

impl Label {
    pub fn new(code: Option<String>, label: Option<String>) -> Label {
        Label { code, label }
    }

    pub fn is_empty(&self) -> bool {
        self.code.is_none() && self.label.is_none()
    }

    /// `<name><Code/><Label/></name>`, pruned when both parts are missing.
    pub fn to_node(&self, name: &'static str) -> Option<Node> {
        Node::parent(
            name,
            vec![
                self.code.as_ref().map(|c| Node::text("Code", c.clone())),
                self.label.as_ref().map(|l| Node::text("Label", l.clone())),
            ],
        )
    }
}

/// XSD `DateType`: attributes only, `year` required (CommonTypes.xsd).
pub fn date_node(name: &'static str, d: &DateAttrs) -> Node {
    let mut attrs = vec![("year", d.year.clone())];
    if let Some(m) = &d.month {
        attrs.push(("month", m.clone()));
    }
    if let Some(day) = &d.day {
        attrs.push(("day", day.clone()));
    }
    Node::empty(name, attrs)
}

/// XSD `PeriodType`: `From?, To?, Current?` (Duration and DurationEquivalent are never emitted).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Period {
    pub from: Option<DateAttrs>,
    pub to: Option<DateAttrs>,
    pub current: bool,
}

impl Period {
    pub fn to_node(&self) -> Option<Node> {
        Node::parent(
            "Period",
            vec![
                self.from.as_ref().map(|d| date_node("From", d)),
                self.to.as_ref().map(|d| date_node("To", d)),
                self.current.then(|| Node::text("Current", "true")),
            ],
        )
    }
}
