//! The only place where XML text is produced and escaped (DESIGN section 6.2).
use super::node::{Kids, Node};

pub fn escape_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            c => o.push(c),
        }
    }
    o
}

pub fn escape_attr(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\n' => o.push_str("&#10;"),
            '\t' => o.push_str("&#9;"),
            '\r' => o.push_str("&#13;"),
            c => o.push(c),
        }
    }
    o
}

fn open_tag(n: &Node, out: &mut String, selfclose: bool) {
    out.push('<');
    out.push_str(n.name);
    for (k, v) in &n.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(&escape_attr(v));
        out.push('"');
    }
    out.push_str(if selfclose { "/>" } else { ">" });
}

fn write_node(n: &Node, out: &mut String, depth: usize, pretty: bool) {
    let pad = |out: &mut String, d: usize| {
        if pretty {
            for _ in 0..d {
                out.push_str("  ");
            }
        }
    };
    pad(out, depth);
    match &n.kids {
        Kids::Empty => open_tag(n, out, true),
        Kids::Text(t) => {
            open_tag(n, out, false);
            out.push_str(&escape_text(t));
            out.push_str("</");
            out.push_str(n.name);
            out.push('>');
        }
        Kids::Elems(ks) => {
            open_tag(n, out, false);
            if pretty {
                out.push('\n');
            }
            for k in ks {
                write_node(k, out, depth + 1, pretty);
                if pretty {
                    out.push('\n');
                }
            }
            pad(out, depth);
            out.push_str("</");
            out.push_str(n.name);
            out.push('>');
        }
    }
}

/// Serialise with the XML declaration and exactly one trailing newline.
pub fn serialize(root: &Node, pretty: bool) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    write_node(root, &mut out, 0, pretty);
    out.push('\n');
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn escaping() {
        assert_eq!(
            escape_text("a&b<c>d\"e'f ]]> g"),
            "a&amp;b&lt;c&gt;d\"e'f ]]&gt; g"
        );
        assert_eq!(
            escape_attr("a\"b\n\t\r&<>"),
            "a&quot;b&#10;&#9;&#13;&amp;&lt;&gt;"
        );
    }

    #[test]
    fn empty_and_nested() {
        let n = Node::parent_always("LearnerInfo", vec![]);
        assert_eq!(
            serialize(&n, true),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<LearnerInfo/>\n"
        );
        let n = Node::parent(
            "A",
            vec![
                Some(Node::text("B", "x&y")),
                Some(Node::empty("C", vec![("year", "2020".into())])),
            ],
        )
        .unwrap();
        assert_eq!(
            serialize(&n, true),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<A>\n  <B>x&amp;y</B>\n  <C year=\"2020\"/>\n</A>\n"
        );
        assert_eq!(
            serialize(&n, false),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<A><B>x&amp;y</B><C year=\"2020\"/></A>\n"
        );
    }

    #[test]
    fn pruning() {
        assert!(Node::parent("A", vec![None, None]).is_none());
    }
}
