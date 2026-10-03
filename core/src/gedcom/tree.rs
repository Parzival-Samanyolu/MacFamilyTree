//! Generic GEDCOM line tree: tolerant parsing (CONC/CONT folding, malformed-line recovery) and serialisation.

use super::{Issue, Severity};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Node {
    pub xref: Option<String>,
    pub tag: String,
    pub value: String,
    pub children: Vec<Node>,
    /// 1-based source line (0 when synthesised).
    pub line: usize,
}

impl Node {
    pub fn new(tag: &str, value: &str) -> Node {
        Node {
            tag: tag.into(),
            value: value.into(),
            ..Default::default()
        }
    }
    pub fn child(&self, tag: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.tag == tag)
    }
    pub fn children_of<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.tag == tag)
    }
    pub fn child_value(&self, tag: &str) -> Option<&str> {
        self.child(tag).map(|c| c.value.as_str())
    }
    pub fn push(&mut self, n: Node) -> &mut Node {
        self.children.push(n);
        self.children.last_mut().unwrap()
    }
    pub fn with(mut self, n: Node) -> Node {
        self.children.push(n);
        self
    }
    pub fn is_pointer(&self) -> bool {
        is_pointer(&self.value)
    }
}

pub fn is_pointer(v: &str) -> bool {
    v.len() > 2 && v.starts_with('@') && v.ends_with('@') && !v.starts_with("@#")
}

struct Line {
    level: usize,
    xref: Option<String>,
    tag: String,
    value: String,
}

fn parse_line(raw: &str) -> Option<Line> {
    let raw = raw.trim_start_matches(['\u{FEFF}', ' ', '\t']);
    let digits: String = raw.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.len() > 2 {
        return None;
    }
    let level: usize = digits.parse().ok()?;
    let mut rest = raw[digits.len()..].trim_start_matches([' ', '\t']);
    let mut xref = None;
    if rest.starts_with('@') {
        let end = rest[1..].find('@')? + 2;
        let candidate = &rest[..end];
        // A level-N line `@X@ TAG` has an xref; a value pointer never precedes the tag.
        xref = Some(candidate.to_string());
        rest = rest[end..].trim_start_matches([' ', '\t']);
    }
    let tag_end = rest.find([' ', '\t']).unwrap_or(rest.len());
    let tag = rest[..tag_end].to_string();
    if tag.is_empty() {
        return None;
    }
    // Exactly one delimiter space is skipped; the rest of the line is the value, verbatim.
    let value = if tag_end < rest.len() {
        rest[tag_end + 1..].to_string()
    } else {
        String::new()
    };
    Some(Line {
        level,
        xref,
        tag,
        value,
    })
}

/// Parse GEDCOM text into level-0 records. Never fails: problems are reported as issues.
pub fn parse(text: &str, issues: &mut Vec<Issue>) -> Vec<Node> {
    // Stack of (level, path-index) built via an arena of flat nodes then folded.
    struct Flat {
        level: usize,
        node: Node,
    }
    let mut flat: Vec<Flat> = Vec::new();
    let mut prev_level = 0usize;
    for (i, raw) in text.split('\n').enumerate() {
        let raw = raw.trim_end_matches('\r');
        let lineno = i + 1;
        if raw.trim().is_empty() {
            continue;
        }
        let Some(l) = parse_line(raw) else {
            match flat.last_mut() {
                Some(last) => {
                    issues.push(Issue::new(
                        Severity::Warning,
                        lineno,
                        format!(
                            "malformed line treated as continuation: {:?}",
                            truncate(raw)
                        ),
                    ));
                    last.node.value.push('\n');
                    last.node.value.push_str(raw);
                }
                None => issues.push(Issue::new(
                    Severity::Error,
                    lineno,
                    format!(
                        "ignored malformed line before first record: {:?}",
                        truncate(raw)
                    ),
                )),
            }
            continue;
        };
        let mut level = l.level;
        if level > prev_level + 1 {
            issues.push(Issue::new(
                Severity::Warning,
                lineno,
                format!("level jumps from {} to {}; clamped", prev_level, level),
            ));
            level = prev_level + 1;
        }
        if level == 0 && flat.is_empty() && l.tag != "HEAD" {
            issues.push(Issue::new(
                Severity::Warning,
                lineno,
                "file does not start with a HEAD record".into(),
            ));
        }
        if matches!(l.tag.as_str(), "CONC" | "CONT") && level > 0 {
            if let Some(parent) = flat.iter_mut().rev().find(|f| f.level < level) {
                if l.tag == "CONT" {
                    parent.node.value.push('\n');
                }
                parent.node.value.push_str(&l.value);
                prev_level = level;
                continue;
            }
        }
        flat.push(Flat {
            level,
            node: Node {
                xref: l.xref,
                tag: l.tag,
                value: l.value,
                children: vec![],
                line: lineno,
            },
        });
        prev_level = level;
    }
    // Fold flat list into a forest.
    fn fold(
        flat: &mut std::vec::IntoIter<Flat>,
        peeked: &mut Option<Flat>,
        level: usize,
    ) -> Vec<Node> {
        let mut out = Vec::new();
        loop {
            let next = peeked.take().or_else(|| flat.next());
            let Some(f) = next else { break };
            if f.level < level {
                *peeked = Some(f);
                break;
            }
            let mut node = f.node;
            node.children = fold(flat, peeked, level + 1);
            out.push(node);
        }
        out
    }
    let mut it = flat.into_iter();
    let mut peeked = None;
    fold(&mut it, &mut peeked, 0)
}

fn truncate(s: &str) -> String {
    s.chars().take(40).collect()
}

pub struct WriteOpts {
    /// Split lines longer than this with CONC (0 = never; GEDCOM 7).
    pub max_len: usize,
}

pub fn write(records: &[Node], opts: &WriteOpts) -> String {
    let mut out = String::new();
    for r in records {
        write_node(r, 0, opts, &mut out);
    }
    out
}

fn push_line(out: &mut String, level: usize, xref: Option<&str>, tag: &str, value: &str) {
    out.push_str(&level.to_string());
    if let Some(x) = xref {
        out.push(' ');
        out.push_str(x);
    }
    out.push(' ');
    out.push_str(tag);
    if !value.is_empty() {
        out.push(' ');
        out.push_str(value);
    }
    out.push('\n');
}

fn write_node(n: &Node, level: usize, opts: &WriteOpts, out: &mut String) {
    let mut segs = n.value.split('\n');
    let first = segs.next().unwrap_or("");
    write_value(
        out,
        level,
        n.xref.as_deref(),
        &n.tag,
        first,
        opts,
        level + 1,
    );
    for seg in segs {
        write_value(out, level + 1, None, "CONT", seg, opts, level + 1);
    }
    for c in &n.children {
        write_node(c, level + 1, opts, out);
    }
}

/// Emit one logical value, splitting with CONC at char boundaries when too long.
fn write_value(
    out: &mut String,
    level: usize,
    xref: Option<&str>,
    tag: &str,
    value: &str,
    opts: &WriteOpts,
    conc_level: usize,
) {
    if opts.max_len == 0 || value.chars().count() <= opts.max_len {
        push_line(out, level, xref, tag, value);
        return;
    }
    let chars: Vec<char> = value.chars().collect();
    let mut chunks = chars.chunks(opts.max_len);
    let first: String = chunks.next().unwrap().iter().collect();
    push_line(out, level, xref, tag, &first);
    for c in chunks {
        let s: String = c.iter().collect();
        // CONC value must preserve leading spaces, so the delimiter is always exactly one space.
        out.push_str(&conc_level.to_string());
        out.push_str(" CONC ");
        out.push_str(&s);
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> (Vec<Node>, Vec<Issue>) {
        let mut i = vec![];
        let r = parse(s, &mut i);
        (r, i)
    }

    #[test]
    fn nests_and_folds_continuations() {
        let (r, i) = p("0 HEAD\n1 GEDC\n2 VERS 5.5.1\n0 @N1@ NOTE Line one\n1 CONT line two\n1 CONC  continued\n0 TRLR\n");
        assert!(i.is_empty());
        assert_eq!(r.len(), 3);
        assert_eq!(
            r[0].child("GEDC").unwrap().child_value("VERS"),
            Some("5.5.1")
        );
        assert_eq!(r[1].xref.as_deref(), Some("@N1@"));
        assert_eq!(r[1].value, "Line one\nline two continued");
    }

    #[test]
    fn recovers_from_malformed() {
        let (r, i) = p("0 HEAD\n1 NOTE a\nstray line\n3 DEEP x\n0 TRLR");
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].child("NOTE").unwrap().value, "a\nstray line");
        assert!(i.iter().any(|x| x.message.contains("malformed")));
        assert!(i.iter().any(|x| x.message.contains("level jumps")));
    }

    #[test]
    fn write_splits_and_roundtrips_long_lines() {
        let long = "é".repeat(600);
        let mut n = Node::new("NOTE", &format!("{}\nsecond", long));
        n.xref = Some("@N1@".into());
        let text = write(&[n.clone()], &WriteOpts { max_len: 248 });
        assert!(text.lines().all(|l| l.chars().count() <= 260));
        let (r, i) = p(&text);
        assert!(i.iter().all(|x| x.message.contains("HEAD")));
        assert_eq!(r[0].value, n.value);
    }

    #[test]
    fn preserves_leading_space_in_value() {
        let (r, _) = p("0 HEAD\n1 NOTE   indented\n");
        assert_eq!(r[0].child("NOTE").unwrap().value, "  indented");
    }

    #[test]
    fn pointer_detection() {
        assert!(is_pointer("@I1@"));
        assert!(!is_pointer("@#DJULIAN@ 1 JAN 1700"));
        assert!(!is_pointer("@@"));
    }
}
