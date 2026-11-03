//! Name splitting.

/// Split on the last space: "Jean de La Fontaine" gives FirstName "Jean de La", Surname "Fontaine".
pub fn split_name(name: &str) -> (Option<String>, Option<String>) {
    let n = name.split_whitespace().collect::<Vec<_>>().join(" ");
    match n.rsplit_once(' ') {
        Some((a, b)) => (Some(a.to_string()), Some(b.to_string())),
        None if n.is_empty() => (None, None),
        None => (Some(n), None),
    }
}
