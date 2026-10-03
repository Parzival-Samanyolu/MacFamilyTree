//! Typed records and domain operations over the generic [`Store`](crate::store::Store).

use crate::date::GenDate;
use crate::name::PersonName;
use crate::store::{new_id, Result, Tx};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersonRec {
    pub id: String,
    pub sex: String,
    pub living_override: Option<bool>,
    pub is_private: bool,
    pub bookmarked: bool,
    pub ref_no: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NameRec {
    pub id: String,
    pub person_id: String,
    pub kind: String,
    pub prefix: String,
    pub given: String,
    pub nickname: String,
    pub surname_prefix: String,
    pub surname: String,
    pub suffix: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FamilyRec {
    pub id: String,
    pub partner1: Option<String>,
    pub partner2: Option<String>,
    pub rel_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChildRec {
    pub id: String,
    pub family_id: String,
    pub person_id: String,
    pub rel_type: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventRec {
    pub id: String,
    pub owner_type: String,
    pub owner_id: String,
    pub kind: String,
    pub custom_kind: Option<String>,
    pub value: Option<String>,
    pub date_json: Option<String>,
    pub date_sort: Option<i64>,
    pub date_sort_end: Option<i64>,
    pub place_id: Option<String>,
    pub description: Option<String>,
    pub sort_order: i64,
}

impl EventRec {
    pub fn date(&self) -> Option<GenDate> {
        self.date_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
    }
    pub fn set_date(&mut self, d: Option<&GenDate>) {
        self.date_json = d.map(|d| serde_json::to_string(d).unwrap());
        self.date_sort = d.and_then(|d| d.sort_key());
        self.date_sort_end = d.and_then(|d| d.range().map(|r| r.1));
    }
}

impl<'a> Tx<'a> {
    pub fn create_person(&mut self, name: &PersonName, sex: &str) -> Result<String> {
        let id = new_id();
        self.put(
            "person",
            &PersonRec {
                id: id.clone(),
                sex: sex.into(),
                ..Default::default()
            },
        )?;
        self.add_name(&id, name, 0)?;
        Ok(id)
    }

    pub fn add_name(&mut self, person_id: &str, n: &PersonName, sort_order: i64) -> Result<String> {
        let id = new_id();
        self.put(
            "person_name",
            &NameRec {
                id: id.clone(),
                person_id: person_id.into(),
                kind: format!("{:?}", n.kind),
                prefix: n.prefix.clone(),
                given: n.given.clone(),
                nickname: n.nickname.clone(),
                surname_prefix: n.surname_prefix.clone(),
                surname: n.surname.clone(),
                suffix: n.suffix.clone(),
                sort_order,
            },
        )?;
        Ok(id)
    }

    pub fn add_event(
        &mut self,
        owner_type: &str,
        owner_id: &str,
        kind: &str,
        date: Option<&GenDate>,
        place_id: Option<&str>,
    ) -> Result<String> {
        let mut e = EventRec {
            id: new_id(),
            owner_type: owner_type.into(),
            owner_id: owner_id.into(),
            kind: kind.into(),
            place_id: place_id.map(String::from),
            ..Default::default()
        };
        e.set_date(date);
        self.put("event", &e)
    }

    pub fn create_family(
        &mut self,
        p1: Option<&str>,
        p2: Option<&str>,
        rel_type: &str,
    ) -> Result<String> {
        let id = new_id();
        self.put(
            "family",
            &FamilyRec {
                id: id.clone(),
                partner1: p1.map(String::from),
                partner2: p2.map(String::from),
                rel_type: rel_type.into(),
            },
        )
    }

    pub fn add_child(
        &mut self,
        family_id: &str,
        person_id: &str,
        rel_type: &str,
    ) -> Result<String> {
        let order = self
            .ids_where("family_child", "family_id", family_id)?
            .len() as i64;
        self.put(
            "family_child",
            &ChildRec {
                id: new_id(),
                family_id: family_id.into(),
                person_id: person_id.into(),
                rel_type: rel_type.into(),
                sort_order: order,
            },
        )
    }

    /// Delete a person and everything that only makes sense with them (names, events, links, child links);
    /// families keep existing with the partner slot cleared. Fully undoable as one step.
    pub fn delete_person(&mut self, id: &str) -> Result<()> {
        for t in ["person_name"] {
            for r in self.ids_where(t, "person_id", id)? {
                self.delete(t, &r)?;
            }
        }
        for r in self.ids_where("event", "owner_id", id)? {
            if self
                .get("event", &r)?
                .map(|e| e["owner_type"] == "person")
                .unwrap_or(false)
            {
                self.delete("event", &r)?;
            }
        }
        for r in self.ids_where("family_child", "person_id", id)? {
            self.delete("family_child", &r)?;
        }
        for col in ["partner1", "partner2"] {
            for f in self.ids_where("family", col, id)? {
                let mut row = serde_json::Map::new();
                row.insert("id".into(), f.into());
                row.insert(col.into(), serde_json::Value::Null);
                self.put_row("family", row)?;
            }
        }
        for t in [
            "media_link",
            "note_link",
            "task_link",
            "tag_link",
            "citation",
        ] {
            for r in self.ids_where(t, "target_id", id)? {
                self.delete(t, &r)?;
            }
        }
        self.delete("person", id)?;
        Ok(())
    }
}

/// Persons considered living under the privacy rule.
///
/// A manual override wins. Otherwise a person with a death/burial/cremation event is not living; a person with a
/// known birth/christening year is living iff born within `years` of `current_year`; a person with neither is
/// treated as living (privacy-safe default).
pub fn living_ids(
    store: &crate::store::Store,
    years: i32,
    current_year: i32,
) -> Result<std::collections::HashSet<String>> {
    use std::collections::{HashMap, HashSet};
    let mut dead: HashSet<String> = HashSet::new();
    let mut birth_year: HashMap<String, i32> = HashMap::new();
    for e in store.rows("event")? {
        if e["owner_type"] != "person" {
            continue;
        }
        let owner = e["owner_id"].as_str().unwrap_or("").to_string();
        match e["kind"].as_str().unwrap_or("") {
            "DEAT" | "BURI" | "CREM" => {
                dead.insert(owner);
            }
            "BIRT" | "CHR" | "BAPM" => {
                if let Some(k) = e["date_sort"].as_i64() {
                    let y = crate::date::jdn_to_gregorian(k).0;
                    birth_year
                        .entry(owner)
                        .and_modify(|b| *b = (*b).min(y))
                        .or_insert(y);
                }
            }
            _ => {}
        }
    }
    let mut out = HashSet::new();
    for p in store.rows("person")? {
        let id = p["id"].as_str().unwrap_or("").to_string();
        let living = match p["living_override"].as_i64() {
            Some(v) => v != 0,
            None if dead.contains(&id) => false,
            None => birth_year
                .get(&id)
                .map(|y| *y > current_year - years)
                .unwrap_or(true),
        };
        if living {
            out.insert(id);
        }
    }
    Ok(out)
}
