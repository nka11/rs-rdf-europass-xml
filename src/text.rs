//! Sanitising, clamping and small string checks (DESIGN section 6).

/// Result of sanitising a string.
pub struct Sanitized {
    pub text: String,
    /// Number of characters removed because they are illegal in XML 1.0.
    pub removed_illegal: usize,
}

fn is_illegal(c: char) -> bool {
    let u = c as u32;
    (u < 0x20 && c != '\n' && c != '\t' && c != '\r')
        || (0x7F..=0x9F).contains(&u)
        || u == 0xFFFE
        || u == 0xFFFF
}

/// Invisible characters that only hurt matching and copy/paste. U+200C (ZWNJ) and U+200D (ZWJ) are
/// kept: they carry meaning in emoji sequences and in Arabic, Persian and Indic shaping.
fn is_zero_width(c: char) -> bool {
    matches!(c as u32, 0x200B | 0x2060 | 0xFEFF)
}

fn clean_chars(s: &str) -> Sanitized {
    let norm = s.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(norm.len());
    let mut removed = 0usize;
    for c in norm.chars() {
        match c {
            '\u{2028}' | '\u{2029}' => out.push('\n'),
            '\u{00A0}' | '\t' => out.push(' '),
            c if is_zero_width(c) => {}
            c if is_illegal(c) => removed += 1,
            c => out.push(c),
        }
    }
    Sanitized {
        text: out,
        removed_illegal: removed,
    }
}

/// Multi-line sanitise: keeps line breaks, trims line ends, at most one blank line in a row,
/// no leading or trailing blank lines.
pub fn sanitize_multi(s: &str) -> Sanitized {
    let c = clean_chars(s);
    let mut lines: Vec<&str> = Vec::new();
    let mut prev_blank = true; // drops leading blank lines
    for l in c.text.split('\n') {
        let t = l.trim_end();
        let blank = t.is_empty();
        if blank && prev_blank {
            continue;
        }
        lines.push(t);
        prev_blank = blank;
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    Sanitized {
        text: lines.join("\n"),
        removed_illegal: c.removed_illegal,
    }
}

/// Single-line sanitise: every whitespace run becomes one space, trimmed.
pub fn sanitize_line(s: &str) -> Sanitized {
    let c = clean_chars(s);
    let text = c.text.split_whitespace().collect::<Vec<_>>().join(" ");
    Sanitized {
        text,
        removed_illegal: c.removed_illegal,
    }
}

/// Convenience used by tests and other modules: multi-line sanitise, text only.
pub fn sanitize(s: &str) -> String {
    sanitize_multi(s).text
}

/// Clamp to `max` Unicode scalar values. With `ellipsis` the result ends in U+2026 (counted in `max`)
/// and is cut at a whitespace boundary in the last 20% when possible. Returns the new text and the
/// original length when something was cut.
pub fn clamp(s: &str, max: usize, ellipsis: bool) -> (String, Option<usize>) {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return (s.to_string(), None);
    }
    let orig = chars.len();
    let keep = if ellipsis { max.saturating_sub(1) } else { max };
    let head: Vec<char> = chars.iter().take(keep).copied().collect();
    let min_cut = keep - keep / 5;
    let mut cut = head.len();
    if let Some(p) = head.iter().rposition(|c| c.is_whitespace()) {
        if p >= min_cut {
            cut = p;
        }
    }
    let mut out: String = head.iter().take(cut).collect();
    out = out.trim_end().to_string();
    if ellipsis {
        out.push('\u{2026}');
    }
    (out, Some(orig))
}

/// Cut a comma separated list at the last ", " boundary within `max` chars; no ellipsis.
pub fn clamp_list(s: &str, max: usize) -> (String, Option<usize>) {
    let n = s.chars().count();
    if n <= max {
        return (s.to_string(), None);
    }
    let head: String = s.chars().take(max + 1).collect();
    // the cut must keep only whole items: find the last ", " that starts inside the limit
    let mut best: Option<usize> = None;
    for (i, _) in head.match_indices(", ") {
        if head.get(..i).is_some_and(|h| h.chars().count() <= max) {
            best = Some(i);
        }
    }
    let out = match best {
        Some(i) => head.get(..i).unwrap_or("").to_string(),
        None => s.chars().take(max).collect(),
    };
    (out, Some(n))
}

/// Make a value usable as `xsd:anyURI` for a website: http, https or ftp only, no whitespace or
/// control characters (a plain space, `"`, `<` and `>` are percent-encoded), only RFC 3986 characters afterwards; non-ASCII characters are percent-encoded
/// (RFC 3987 to 3986 mapping) and malformed `%` escapes are rejected. Returns `None` when unusable.
pub fn clean_uri(s: &str) -> Option<String> {
    if s.is_empty() || s.chars().count() > 2000 {
        return None;
    }
    let lower = s.to_ascii_lowercase();
    let rest = ["http://", "https://", "ftp://"]
        .iter()
        .find_map(|p| lower.strip_prefix(p))?;
    if rest.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(s.len());
    let bytes: Vec<char> = s.chars().collect();
    let mut i = 0;
    // the authority (host) cannot hold encoded characters: a space there is a broken URL, not a path detail
    let mut in_authority = true;
    while let Some(&c) = bytes.get(i) {
        if i >= s.len() - rest.len() && matches!(c, '/' | '?' | '#') {
            in_authority = false;
        }
        if in_authority && i >= s.len() - rest.len() && matches!(c, ' ' | '"' | '<' | '>') {
            return None;
        }
        if c.is_control() || (c.is_whitespace() && c != ' ') {
            return None;
        }
        if matches!(c, ' ' | '"' | '<' | '>') {
            // not allowed in a URI but unambiguous: percent-encode instead of dropping the site
            out.push_str(&format!("%{:02X}", c as u8));
        } else if c.is_ascii() {
            let ok = c.is_ascii_alphanumeric() || "-._~:/?#@!$&'()*+,;=".contains(c);
            if c == '%' {
                let h = |k: usize| bytes.get(i + k).is_some_and(|x| x.is_ascii_hexdigit());
                if !(h(1) && h(2)) {
                    return None;
                }
                out.push('%');
            } else if ok {
                out.push(c);
            } else {
                return None;
            }
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        i += 1;
    }
    authority_ok(&out).then_some(out)
}

/// Structural check libxml2 applies to `xsd:anyURI`: one `#` at most, a non-empty host, at most one
/// `@` in the authority and an all-digit, non-empty port when a `:` follows the host.
fn authority_ok(uri: &str) -> bool {
    let Some((_, rest)) = uri.split_once("://") else {
        return false;
    };
    let (before_frag, frag) = match rest.split_once('#') {
        Some((a, f)) => (a, Some(f)),
        None => (rest, None),
    };
    if frag.is_some_and(|f| f.contains('#')) {
        return false;
    }
    let auth = before_frag.split(['/', '?']).next().unwrap_or("");
    if auth.matches('@').count() > 1 {
        return false;
    }
    let hostport = auth.rsplit_once('@').map_or(auth, |x| x.1);
    let (host, port) = match hostport.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (hostport, None),
    };
    !host.is_empty()
        && !host.contains(':')
        && port.map_or(true, |p| {
            !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())
        })
}

/// True when [`clean_uri`] accepts the value unchanged.
pub fn valid_uri(s: &str) -> bool {
    clean_uri(s).as_deref() == Some(s)
}

/// Email check: the XSD pattern `[^@]+@[^\.]+\..+` plus no whitespace.
pub fn valid_email(s: &str) -> bool {
    if s.is_empty() || s.chars().count() > 254 || s.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    let Some(at) = s.find('@') else { return false };
    if at == 0 {
        return false;
    }
    let rest = s.get(at + 1..).unwrap_or("");
    let Some(dot) = rest.find('.') else {
        return false;
    };
    dot > 0 && rest.get(dot + 1..).is_some_and(|t| !t.is_empty())
}

/// De-duplicate case-insensitively, first occurrence wins.
pub fn dedup_ci(items: Vec<String>) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for i in items {
        let k = i.to_lowercase();
        if !seen.contains(&k) {
            seen.push(k);
            out.push(i);
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn removes_illegal_chars() {
        let s = sanitize_line("a\u{0}b\u{1F}c\u{7F}d\u{FFFE}e\u{85}f");
        assert_eq!(s.text, "abcdef");
        assert_eq!(s.removed_illegal, 5);
    }

    #[test]
    fn newlines_nbsp_zero_width_tab() {
        assert_eq!(sanitize("a\r\nb\rc"), "a\nb\nc");
        assert_eq!(sanitize_line("a\u{A0}b\u{200B}c\td").text, "a bc d");
        assert_eq!(sanitize("a\u{2028}b"), "a\nb");
        assert_eq!(sanitize("\n\n a  \n\n\n\nb \n\n"), " a\n\nb");
    }

    #[test]
    fn zwj_and_zwnj_are_kept() {
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        assert_eq!(sanitize_line(family).text, family);
        assert_eq!(
            sanitize_line("\u{0645}\u{200C}\u{0647}").text,
            "\u{0645}\u{200C}\u{0647}"
        );
        assert_eq!(sanitize_line("a\u{200B}b\u{2060}c\u{FEFF}d").text, "abcd");
    }

    #[test]
    fn blank_runs_collapse() {
        assert_eq!(sanitize("\n\nx\n\n\n\ny\n\n"), "x\n\ny");
    }

    #[test]
    fn idempotent() {
        for s in [
            "a\r\n\r\n\r\n b  ",
            "\u{200B}x\u{A0}\ty\n\n\n z",
            "  ",
            "é\u{301}\u{0}",
        ] {
            assert_eq!(sanitize(&sanitize(s)), sanitize(s));
            assert_eq!(
                sanitize_line(&sanitize_line(s).text).text,
                sanitize_line(s).text
            );
        }
    }

    #[test]
    fn clamp_on_scalar_boundary() {
        let s = "é\u{301}😀".repeat(10);
        let (o, orig) = clamp(&s, 7, true);
        assert_eq!(o.chars().count(), 7);
        assert!(o.ends_with('\u{2026}'));
        assert_eq!(orig, Some(30));
        let (o, orig) = clamp("short", 10, true);
        assert_eq!((o.as_str(), orig), ("short", None));
    }

    #[test]
    fn clamp_prefers_word_boundary() {
        let (o, _) = clamp("aaaa bbbb cccc dddd", 15, true);
        assert_eq!(o, "aaaa bbbb cccc\u{2026}");
    }

    #[test]
    fn clamp_list_cuts_at_items() {
        let (o, orig) = clamp_list("alpha, beta, gamma, delta", 15);
        assert_eq!(o, "alpha, beta");
        assert_eq!(orig, Some(25));
        assert_eq!(clamp_list("alpha, beta", 50).0, "alpha, beta");
        assert_eq!(clamp_list("abcdefghij", 5).0, "abcde");
    }

    #[test]
    fn email_and_uri() {
        assert!(valid_email("a@b.co"));
        assert!(!valid_email("a@b"));
        assert!(!valid_email("a b@c.d"));
        assert!(!valid_email("@b.c"));
        assert!(!valid_email("a@.c"));
        assert!(!valid_email("a@b."));
        assert!(valid_uri("https://example.org/cv"));
        assert!(valid_uri("HTTP://x.org/a?b=c%20d"));
        assert!(!valid_uri("javascript:alert(1)"));
        assert!(!valid_uri("file:///etc/passwd"));
        assert!(!valid_uri("https://a b.c"));
        assert!(!valid_uri("https://"));
        assert!(!valid_uri("https://Z[x"));
        assert!(!valid_uri("https://https://x"));
        assert!(!valid_uri("https://a:/x"));
        assert!(!valid_uri("https://a#b#c"));
        assert!(!valid_uri("https://a@b@c/"));
        assert!(valid_uri("https://u:p@host:8080/p?q=1#f"));
        assert!(!valid_uri("https://a/%zz"));
        assert_eq!(
            clean_uri("https://exemple.org/café").as_deref(),
            Some("https://exemple.org/caf%C3%A9")
        );
        assert_eq!(clean_uri("https://a/%41").as_deref(), Some("https://a/%41"));
        assert_eq!(
            clean_uri("https://example.org/a b").as_deref(),
            Some("https://example.org/a%20b")
        );
        assert_eq!(
            clean_uri("https://example.org/q?x=\"y\"").as_deref(),
            Some("https://example.org/q?x=%22y%22")
        );
        assert_eq!(
            clean_uri("https://example.org/a?b=1&c=<2>#f").as_deref(),
            Some("https://example.org/a?b=1&c=%3C2%3E#f")
        );
        assert_eq!(clean_uri("https://example.org/a\tb"), None);
    }
}
