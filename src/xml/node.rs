//! Minimal element tree. Containers prune themselves when empty (DESIGN section 8).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kids {
    Empty,
    Text(String),
    Elems(Vec<Node>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub name: &'static str,
    pub attrs: Vec<(&'static str, String)>,
    pub kids: Kids,
}

impl Node {
    /// Element with text content.
    pub fn text(name: &'static str, text: impl Into<String>) -> Node {
        Node {
            name,
            attrs: Vec::new(),
            kids: Kids::Text(text.into()),
        }
    }

    /// Element with attributes and no content (e.g. `DateType`).
    pub fn empty(name: &'static str, attrs: Vec<(&'static str, String)>) -> Node {
        Node {
            name,
            attrs,
            kids: Kids::Empty,
        }
    }

    /// Container; `None` when no child survived.
    pub fn parent(name: &'static str, kids: Vec<Option<Node>>) -> Option<Node> {
        let kids: Vec<Node> = kids.into_iter().flatten().collect();
        if kids.is_empty() {
            None
        } else {
            Some(Node {
                name,
                attrs: Vec::new(),
                kids: Kids::Elems(kids),
            })
        }
    }

    /// Container that is kept even when empty (`LearnerInfo`).
    pub fn parent_always(name: &'static str, kids: Vec<Option<Node>>) -> Node {
        let kids: Vec<Node> = kids.into_iter().flatten().collect();
        if kids.is_empty() {
            Node {
                name,
                attrs: Vec::new(),
                kids: Kids::Empty,
            }
        } else {
            Node {
                name,
                attrs: Vec::new(),
                kids: Kids::Elems(kids),
            }
        }
    }

    pub fn with_attrs(mut self, attrs: Vec<(&'static str, String)>) -> Node {
        self.attrs = attrs;
        self
    }
}

/// `Some(<name>text</name>)` when the text is present.
pub fn opt_text(name: &'static str, text: &Option<String>) -> Option<Node> {
    text.as_ref().map(|t| Node::text(name, t.clone()))
}
