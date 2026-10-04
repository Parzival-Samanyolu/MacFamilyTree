//! Advanced person search: combinable criteria and saved searches.

use crate::facts::Facts;
use crate::model::living_ids;
use crate::name::{cologne, fold, soundex};
use crate::store::{Result, Store};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Criteria {
    pub given: String,
    pub surname: String,
    /// Match surnames that sound alike (Soundex or Cologne phonetics) instead of containing the text.
    pub phonetic: bool,
    /// "M", "F", "U" or empty for any.
    pub sex: String,
    pub born_from: Option<i32>,
    pub born_to: Option<i32>,
    pub died_from: Option<i32>,
    pub died_to: Option<i32>,
    /// Text found in the place of any event of the person.
    pub place: String,
    /// Only people having an event of this kind (GEDCOM tag), e.g. "OCCU".
    pub event_kind: String,
    /// Text found in the value / description of the person's events (e.g. an occupation).
    pub event_text: String,
    /// "living", "deceased" or empty.
    pub status: String,
    pub has_media: Option<bool>,
    pub has_sources: Option<bool>,
    pub has_parents: Option<bool>,
    pub has_children: Option<bool>,
    pub bookmarked: Option<bool>,
}

fn year(j: Option<i64>) -> Option<i32> {
    j.map(|k| crate::date::jdn_to_gregorian(k).0)
}

fn contains(hay: &str, needle: &str) -> bool {
    fold(hay).contains(&fold(needle))
}

pub fn run(
    store: &Store,
    c: &Criteria,
    living_years: i32,
    current_year: i32,
) -> Result<Vec<String>> {
    let f = Facts::load(store)?;
    let names: HashMap<String, Vec<(String, String)>> = {
        let mut m: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for n in store.rows("person_name")? {
            m.entry(n["person_id"].as_str().unwrap_or("").into())
                .or_default()
                .push((
                    n["given"].as_str().unwrap_or("").into(),
                    n["surname"].as_str().unwrap_or("").into(),
                ));
        }
        m
    };
    let rows: HashMap<String, crate::store::Row> = store
        .rows("person")?
        .into_iter()
        .filter_map(|r| r["id"].as_str().map(|i| (i.to_string(), r.clone())))
        .collect();
    let set = |t: &str, col: &str, types: &[&str]| -> Result<HashSet<String>> {
        Ok(store
            .rows(t)?
            .into_iter()
            .filter(|r| {
                types.is_empty() || types.contains(&r["target_type"].as_str().unwrap_or(""))
            })
            .filter_map(|r| r[col].as_str().map(String::from))
            .collect())
    };
    let with_media = set("media_link", "target_id", &["person"])?;
    let with_sources = {
        let mut s = set("citation", "target_id", &["person"])?;
        // a citation on one of the person's events also counts
        for e in &f.events {
            if e.owner_type == "person" {
                s.extend(
                    store
                        .rows_where("citation", "target_id", &e.id)?
                        .into_iter()
                        .map(|_| e.owner_id.clone()),
                );
            }
        }
        s
    };
    let living = if c.status.is_empty() {
        HashSet::new()
    } else {
        living_ids(store, living_years, current_year)?
    };
    let mut has_parents: HashSet<&str> = HashSet::new();
    let mut has_children: HashSet<&str> = HashSet::new();
    for fam in &f.families {
        for ch in &fam.children {
            has_parents.insert(ch);
            for p in &fam.partners {
                has_children.insert(p);
            }
        }
    }
    let sx = (soundex(&c.surname), cologne(&c.surname));
    let mut out = vec![];
    for id in &f.order {
        let nm = names.get(id).cloned().unwrap_or_default();
        if !c.given.trim().is_empty() && !nm.iter().any(|(g, _)| contains(g, &c.given)) {
            continue;
        }
        if !c.surname.trim().is_empty() {
            let ok = nm.iter().any(|(_, s)| {
                if c.phonetic {
                    !sx.0.is_empty() && (soundex(s) == sx.0 || cologne(s) == sx.1)
                } else {
                    contains(s, &c.surname)
                }
            });
            if !ok {
                continue;
            }
        }
        let row = &rows[id];
        if !c.sex.is_empty() {
            let sex = row["sex"].as_str().unwrap_or("U");
            if sex != c.sex && !(c.sex == "U" && sex.is_empty()) {
                continue;
            }
        }
        let by = year(
            f.first_dated(id, &["BIRT", "CHR", "BAPM"])
                .and_then(|e| e.start),
        );
        let dy = year(
            f.first_dated(id, &["DEAT", "BURI", "CREM"])
                .and_then(|e| e.start),
        );
        let in_range = |v: Option<i32>, lo: Option<i32>, hi: Option<i32>| {
            (lo.is_none() && hi.is_none())
                || v.map(|y| {
                    lo.map(|l| y >= l).unwrap_or(true) && hi.map(|h| y <= h).unwrap_or(true)
                })
                .unwrap_or(false)
        };
        if !in_range(by, c.born_from, c.born_to) || !in_range(dy, c.died_from, c.died_to) {
            continue;
        }
        if !c.place.trim().is_empty()
            && !f.person_events(id).any(|e| {
                f.place_name(e)
                    .map(|p| contains(p, &c.place))
                    .unwrap_or(false)
            })
        {
            continue;
        }
        if !c.event_kind.is_empty() && !f.person_events(id).any(|e| e.kind == c.event_kind) {
            continue;
        }
        if !c.event_text.trim().is_empty()
            && !f.person_events(id).any(|e| {
                (c.event_kind.is_empty() || e.kind == c.event_kind)
                    && (e
                        .value
                        .as_deref()
                        .map(|v| contains(v, &c.event_text))
                        .unwrap_or(false)
                        || e.custom
                            .as_deref()
                            .map(|v| contains(v, &c.event_text))
                            .unwrap_or(false))
            })
        {
            continue;
        }
        match c.status.as_str() {
            "living" if !living.contains(id) => continue,
            "deceased" if living.contains(id) => continue,
            _ => {}
        }
        let flag = |want: Option<bool>, have: bool| want.map(|w| w == have).unwrap_or(true);
        if !flag(c.has_media, with_media.contains(id))
            || !flag(c.has_sources, with_sources.contains(id))
            || !flag(c.has_parents, has_parents.contains(id.as_str()))
            || !flag(c.has_children, has_children.contains(id.as_str()))
            || !flag(c.bookmarked, row["bookmarked"].as_i64() == Some(1))
        {
            continue;
        }
        out.push(id.clone());
    }
    out.sort_by_key(|id| {
        let p = &f.persons[id];
        (fold(&p.surname), fold(&p.given), id.clone())
    });
    Ok(out)
}

// ---------- saved searches ----------

const KEY: &str = "saved_searches";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Saved {
    pub name: String,
    pub criteria: Criteria,
}

pub fn saved(store: &Store) -> Result<Vec<Saved>> {
    let json: Option<String> = store
        .conn()
        .query_row("SELECT value FROM setting WHERE id = ?1", [KEY], |r| {
            r.get(0)
        })
        .ok();
    Ok(json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default())
}

fn write(store: &mut Store, list: &[Saved], label: &str) -> Result<()> {
    let json = serde_json::to_string(list)?;
    store.transact(label, |tx| {
        let mut r = serde_json::Map::new();
        r.insert("id".into(), json!(KEY));
        r.insert("value".into(), json!(json));
        tx.put_row("setting", r)?;
        Ok(())
    })
}

/// Save (or replace) a search under `name`.
pub fn save(store: &mut Store, name: &str, c: &Criteria) -> Result<()> {
    let mut list = saved(store)?;
    list.retain(|s| s.name != name);
    list.push(Saved {
        name: name.to_string(),
        criteria: c.clone(),
    });
    list.sort_by_key(|s| fold(&s.name));
    write(store, &list, "Save search")
}

pub fn delete(store: &mut Store, name: &str) -> Result<()> {
    let mut list = saved(store)?;
    list.retain(|s| s.name != name);
    write(store, &list, "Delete saved search")
}
