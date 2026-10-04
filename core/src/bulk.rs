//! Bulk find & replace over selected text fields, previewed first and applied as one undoable change.

use crate::store::{Result, Store, StoreError, Tx};
use serde::Serialize;
use serde_json::Value;

/// Fields that may be edited in bulk: (table, column).
pub const FIELDS: &[(&str, &str)] = &[
    ("person_name", "given"),
    ("person_name", "surname"),
    ("person_name", "nickname"),
    ("place", "name"),
    ("event", "description"),
    ("event", "value"),
    ("event", "cause"),
    ("note", "body"),
    ("source", "title"),
    ("source", "author"),
    ("citation", "page"),
];

#[derive(Debug, Clone)]
pub struct Spec {
    /// `table.column`, one of [`FIELDS`].
    pub field: String,
    pub find: String,
    pub replace: String,
    pub case_sensitive: bool,
    /// Only values equal to `find` match, and they are replaced entirely.
    pub whole_field: bool,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Change {
    pub table: String,
    pub id: String,
    pub before: String,
    pub after: String,
}

fn field(spec: &Spec) -> Result<(&'static str, &'static str)> {
    FIELDS
        .iter()
        .find(|(t, c)| format!("{t}.{c}") == spec.field)
        .copied()
        .ok_or_else(|| {
            StoreError::Other(format!("field `{}` cannot be edited in bulk", spec.field))
        })
}

fn same(a: char, b: char, cs: bool) -> bool {
    a == b || (!cs && a.to_lowercase().eq(b.to_lowercase()))
}

/// Replace every non-overlapping occurrence of `find`; `None` if there is none.
pub fn replace_in(text: &str, spec: &Spec) -> Option<String> {
    if spec.find.is_empty() {
        return None;
    }
    let cs = spec.case_sensitive;
    if spec.whole_field {
        let eq = text.chars().count() == spec.find.chars().count()
            && text
                .chars()
                .zip(spec.find.chars())
                .all(|(a, b)| same(a, b, cs));
        return (eq && text != spec.replace).then(|| spec.replace.clone());
    }
    let hay: Vec<char> = text.chars().collect();
    let needle: Vec<char> = spec.find.chars().collect();
    let mut out = String::with_capacity(text.len());
    let (mut i, mut hit) = (0, false);
    while i < hay.len() {
        if i + needle.len() <= hay.len()
            && hay[i..i + needle.len()]
                .iter()
                .zip(&needle)
                .all(|(a, b)| same(*a, *b, cs))
        {
            out.push_str(&spec.replace);
            i += needle.len();
            hit = true;
        } else {
            out.push(hay[i]);
            i += 1;
        }
    }
    (hit && out != text).then_some(out)
}

pub fn preview(store: &Store, spec: &Spec) -> Result<Vec<Change>> {
    let (t, c) = field(spec)?;
    let mut out = vec![];
    for r in store.rows(t)? {
        if let Some(before) = r[c].as_str() {
            if let Some(after) = replace_in(before, spec) {
                out.push(Change {
                    table: t.into(),
                    id: r["id"].as_str().unwrap_or("").into(),
                    before: before.into(),
                    after,
                });
            }
        }
    }
    out.sort_by(|a, b| a.before.cmp(&b.before).then(a.id.cmp(&b.id)));
    Ok(out)
}

/// Apply inside a transaction; returns the number of changed records.
pub fn apply(tx: &mut Tx, spec: &Spec) -> Result<usize> {
    let (t, c) = field(spec)?;
    let mut n = 0;
    for id in tx.all_ids(t)? {
        let Some(mut r) = tx.get(t, &id)? else {
            continue;
        };
        if let Some(after) = r[c].as_str().and_then(|b| replace_in(b, spec)) {
            // a place may not become empty, a name field stays a string
            if t == "place" && after.trim().is_empty() {
                continue;
            }
            r.insert(c.into(), Value::from(after));
            tx.put_row(t, r)?;
            n += 1;
        }
    }
    Ok(n)
}
