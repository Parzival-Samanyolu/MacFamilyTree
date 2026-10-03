//! Place engine: hierarchical find-or-create from free text, full names, merge, hierarchy cleanup.

use crate::name::fold;
use crate::store::{new_id, Result, Row, Store, Tx};
use serde_json::{Map, Value};
use std::collections::HashMap;

fn row(pairs: &[(&str, Value)]) -> Row {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v.clone());
    }
    m
}

/// Split "Konya, Konya Province, Turkey" into trimmed, non-empty parts, most specific first.
pub fn split_place(text: &str) -> Vec<String> {
    text.split(',')
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Find or create the place for `text` and return the id of the most specific part.
/// Matching ignores case and diacritics within the same parent, so "Istanbul" and "İstanbul" are one place.
pub fn find_or_create(tx: &mut Tx, text: &str) -> Result<Option<String>> {
    let parts = split_place(text);
    if parts.is_empty() {
        return Ok(None);
    }
    let mut parent: Option<String> = None;
    for name in parts.iter().rev() {
        let mut found = None;
        let siblings: Vec<String> = match &parent {
            Some(p) => tx.ids_where("place", "parent_id", p)?,
            None => root_places(tx)?,
        };
        for id in siblings {
            if let Some(r) = tx.get("place", &id)? {
                if fold(r["name"].as_str().unwrap_or("")) == fold(name) {
                    found = Some(id);
                    break;
                }
            }
        }
        let id = match found {
            Some(id) => id,
            None => {
                let id = new_id();
                tx.put_row(
                    "place",
                    row(&[
                        ("id", id.clone().into()),
                        ("name", name.clone().into()),
                        (
                            "parent_id",
                            parent.clone().map(Value::from).unwrap_or(Value::Null),
                        ),
                    ]),
                )?;
                id
            }
        };
        parent = Some(id);
    }
    Ok(parent)
}

fn root_places(tx: &Tx) -> Result<Vec<String>> {
    // top-level places have a NULL parent
    let mut out = vec![];
    for id in tx.all_ids("place")? {
        if tx
            .get("place", &id)?
            .map(|r| r["parent_id"].is_null())
            .unwrap_or(false)
        {
            out.push(id);
        }
    }
    Ok(out)
}

/// Full name from a map of all places (cheap when many lookups are needed).
pub fn full_names(store: &Store) -> Result<HashMap<String, String>> {
    let rows = store.rows("place")?;
    let by_id: HashMap<&str, &Row> = rows
        .iter()
        .filter_map(|r| r["id"].as_str().map(|i| (i, r)))
        .collect();
    let mut out = HashMap::new();
    for r in &rows {
        let id = r["id"].as_str().unwrap_or("");
        let mut parts = vec![];
        let mut cur: Option<&Row> = Some(r);
        let mut guard = 0;
        while let Some(c) = cur {
            parts.push(c["name"].as_str().unwrap_or("").to_string());
            cur = c["parent_id"].as_str().and_then(|p| by_id.get(p).copied());
            guard += 1;
            if guard > 64 {
                break;
            }
        }
        out.insert(id.to_string(), parts.join(", "));
    }
    Ok(out)
}

/// Merge place `remove` into `keep`: events, media and child places move; matching child places are merged recursively.
pub fn merge(tx: &mut Tx, keep: &str, remove: &str) -> Result<()> {
    if keep == remove {
        return Ok(());
    }
    for col_table in [("event", "place_id"), ("media", "place_id")] {
        for id in tx.ids_where(col_table.0, col_table.1, remove)? {
            tx.put_row(
                col_table.0,
                row(&[("id", id.into()), (col_table.1, keep.into())]),
            )?;
        }
    }
    let keep_children: Vec<(String, String)> = tx
        .ids_where("place", "parent_id", keep)?
        .into_iter()
        .filter_map(|i| {
            tx.get("place", &i)
                .ok()
                .flatten()
                .map(|r| (i, fold(r["name"].as_str().unwrap_or(""))))
        })
        .collect();
    for child in tx.ids_where("place", "parent_id", remove)? {
        let name = fold(
            tx.get("place", &child)?
                .map(|r| r["name"].as_str().unwrap_or("").to_string())
                .unwrap_or_default()
                .as_str(),
        );
        match keep_children.iter().find(|(_, n)| *n == name) {
            Some((same, _)) => merge(tx, same, &child)?,
            None => {
                tx.put_row(
                    "place",
                    row(&[("id", child.into()), ("parent_id", keep.into())]),
                )?;
            }
        }
    }
    // keep coordinates if the survivor has none
    if let (Some(k), Some(r)) = (tx.get("place", keep)?, tx.get("place", remove)?) {
        if k["lat"].is_null() && !r["lat"].is_null() {
            tx.put_row(
                "place",
                row(&[
                    ("id", keep.into()),
                    ("lat", r["lat"].clone()),
                    ("lon", r["lon"].clone()),
                ]),
            )?;
        }
    }
    tx.delete("place", remove)?;
    Ok(())
}

/// Places that look like the same place (same folded name under the same parent).
pub fn find_duplicate_places(store: &Store) -> Result<Vec<(String, String)>> {
    let mut seen: HashMap<(String, String), String> = HashMap::new();
    let mut out = vec![];
    for r in store.rows("place")? {
        let key = (
            r["parent_id"].as_str().unwrap_or("").to_string(),
            fold(r["name"].as_str().unwrap_or("")),
        );
        let id = r["id"].as_str().unwrap_or("").to_string();
        if let Some(first) = seen.get(&key) {
            out.push((first.clone(), id));
        } else {
            seen.insert(key, id);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_and_creates_hierarchy_once() {
        assert_eq!(
            split_place(" Konya ,  Konya  Province, ,Turkey "),
            ["Konya", "Konya Province", "Turkey"]
        );
        let mut s = Store::open_memory().unwrap();
        let (a, b, c) = s
            .transact("p", |tx| {
                let a = find_or_create(tx, "Konya, Konya Province, Turkey")?.unwrap();
                let b = find_or_create(tx, "konya, konya province, TURKEY")?.unwrap();
                let c = find_or_create(tx, "Ankara, Turkey")?.unwrap();
                Ok((a, b, c))
            })
            .unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(s.count("place").unwrap(), 4, "Turkey shared");
        let names = full_names(&s).unwrap();
        assert_eq!(names[&a], "Konya, Konya Province, Turkey");
        assert_eq!(names[&c], "Ankara, Turkey");
    }

    #[test]
    fn diacritics_do_not_split_places_and_merge_moves_events() {
        let mut s = Store::open_memory().unwrap();
        let (i1, i2) = s
            .transact("p", |tx| {
                let a = find_or_create(tx, "İstanbul, Türkiye")?.unwrap();
                let b = find_or_create(tx, "Istanbul, Turkiye")?.unwrap();
                Ok((a, b))
            })
            .unwrap();
        assert_eq!(i1, i2, "folded match");
        // Different spellings that do not fold together stay separate until merged by the user.
        let (x, y, ev) = s
            .transact("q", |tx| {
                let x = find_or_create(tx, "Constantinople, Turkiye")?.unwrap();
                let y = find_or_create(tx, "Istanbul, Turkiye")?.unwrap();
                let p = tx.create_person(&crate::name::PersonName::new("A", "B"), "M")?;
                let ev = tx.add_event("person", &p, "BIRT", None, Some(&x))?;
                Ok((x, y, ev))
            })
            .unwrap();
        s.transact("merge", |tx| merge(tx, &y, &x)).unwrap();
        let moved = s
            .rows("event")
            .unwrap()
            .into_iter()
            .find(|e| e["id"].as_str() == Some(&ev))
            .unwrap();
        assert_eq!(moved["place_id"].as_str(), Some(y.as_str()));
        assert!(s
            .rows("place")
            .unwrap()
            .iter()
            .all(|p| p["id"].as_str() != Some(&x)));
        s.undo().unwrap();
        assert!(s
            .rows("place")
            .unwrap()
            .iter()
            .any(|p| p["id"].as_str() == Some(&x)));
    }
}
