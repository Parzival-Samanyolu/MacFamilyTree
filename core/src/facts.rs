//! Denormalised in-memory view of a project (persons, events, families, places) for analysis modules.

use crate::date::GenDate;
use crate::relationship::{Graph, Sex};
use crate::store::{Result, Store};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct EventFact {
    pub id: String,
    pub owner_type: String,
    pub owner_id: String,
    pub kind: String,
    pub custom: Option<String>,
    pub value: Option<String>,
    /// Earliest / latest Julian Day Number the date can mean.
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub place: Option<String>,
    pub date: Option<GenDate>,
}

#[derive(Debug, Clone)]
pub struct PersonFacts {
    pub id: String,
    pub sex: Sex,
    pub given: String,
    pub surname: String,
    pub events: Vec<usize>,
    pub is_private: bool,
}

#[derive(Debug, Clone)]
pub struct FamilyFacts {
    pub id: String,
    pub partners: Vec<String>,
    pub children: Vec<String>,
    pub events: Vec<usize>,
}

#[derive(Debug)]
pub struct Facts {
    pub persons: HashMap<String, PersonFacts>,
    pub order: Vec<String>,
    pub families: Vec<FamilyFacts>,
    pub events: Vec<EventFact>,
    pub places: HashMap<String, String>,
    pub graph: Graph,
}

pub const DAYS_PER_YEAR: f64 = 365.2425;

impl Facts {
    pub fn load(store: &Store) -> Result<Facts> {
        let graph = Graph::load(store)?;
        let mut places_raw: HashMap<String, (String, Option<String>)> = HashMap::new();
        for p in store.rows("place")? {
            places_raw.insert(
                p["id"].as_str().unwrap_or("").into(),
                (
                    p["name"].as_str().unwrap_or("").into(),
                    p["parent_id"].as_str().map(String::from),
                ),
            );
        }
        let mut places = HashMap::new();
        for id in places_raw.keys() {
            let mut parts = vec![];
            let mut cur = Some(id.clone());
            let mut guard = 0;
            while let Some(c) = cur {
                let Some((n, parent)) = places_raw.get(&c) else {
                    break;
                };
                parts.push(n.clone());
                cur = parent.clone();
                guard += 1;
                if guard > 64 {
                    break;
                }
            }
            places.insert(id.clone(), parts.join(", "));
        }

        let mut names: HashMap<String, (String, String)> = HashMap::new();
        for n in store.rows("person_name")? {
            names
                .entry(n["person_id"].as_str().unwrap_or("").into())
                .or_insert((
                    n["given"].as_str().unwrap_or("").into(),
                    n["surname"].as_str().unwrap_or("").into(),
                ));
        }
        let mut persons = HashMap::new();
        let mut order = vec![];
        for p in store.rows("person")? {
            let id: String = p["id"].as_str().unwrap_or("").into();
            let (given, surname) = names.get(&id).cloned().unwrap_or_default();
            order.push(id.clone());
            persons.insert(
                id.clone(),
                PersonFacts {
                    id,
                    sex: Sex::from_gedcom(p["sex"].as_str().unwrap_or("")),
                    given,
                    surname,
                    events: vec![],
                    is_private: p["is_private"].as_i64() == Some(1),
                },
            );
        }

        let mut events = Vec::new();
        let mut fam_events: HashMap<String, Vec<usize>> = HashMap::new();
        for e in store.rows("event")? {
            let date: Option<GenDate> = e["date_json"]
                .as_str()
                .and_then(|j| serde_json::from_str(j).ok());
            let (start, end) = date
                .as_ref()
                .and_then(|d| d.range())
                .map(|(a, b)| (Some(a), Some(b)))
                .unwrap_or((None, None));
            let ef = EventFact {
                id: e["id"].as_str().unwrap_or("").into(),
                owner_type: e["owner_type"].as_str().unwrap_or("").into(),
                owner_id: e["owner_id"].as_str().unwrap_or("").into(),
                kind: e["kind"].as_str().unwrap_or("").into(),
                custom: e["custom_kind"].as_str().map(String::from),
                value: e["value"].as_str().map(String::from),
                start,
                end,
                place: e["place_id"].as_str().map(String::from),
                date,
            };
            let idx = events.len();
            match ef.owner_type.as_str() {
                "person" => {
                    if let Some(p) = persons.get_mut(&ef.owner_id) {
                        p.events.push(idx);
                    }
                }
                "family" => fam_events.entry(ef.owner_id.clone()).or_default().push(idx),
                _ => {}
            }
            events.push(ef);
        }

        let mut kids: HashMap<String, Vec<String>> = HashMap::new();
        for c in store.rows("family_child")? {
            kids.entry(c["family_id"].as_str().unwrap_or("").into())
                .or_default()
                .push(c["person_id"].as_str().unwrap_or("").into());
        }
        let mut families = vec![];
        for f in store.rows("family")? {
            let id: String = f["id"].as_str().unwrap_or("").into();
            families.push(FamilyFacts {
                partners: ["partner1", "partner2"]
                    .iter()
                    .filter_map(|k| f[*k].as_str().map(String::from))
                    .collect(),
                children: kids.remove(&id).unwrap_or_default(),
                events: fam_events.remove(&id).unwrap_or_default(),
                id,
            });
        }
        Ok(Facts {
            persons,
            order,
            families,
            events,
            places,
            graph,
        })
    }

    pub fn person_events<'a>(&'a self, id: &str) -> impl Iterator<Item = &'a EventFact> {
        self.persons
            .get(id)
            .into_iter()
            .flat_map(|p| p.events.iter().map(|&i| &self.events[i]))
    }

    /// First event of the given kinds (in priority order) that has a usable date.
    pub fn first_dated(&self, id: &str, kinds: &[&str]) -> Option<&EventFact> {
        for k in kinds {
            if let Some(e) = self
                .person_events(id)
                .find(|e| e.kind == *k && e.start.is_some())
            {
                return Some(e);
            }
        }
        None
    }

    pub fn birth(&self, id: &str) -> Option<&EventFact> {
        self.first_dated(id, &["BIRT", "CHR", "BAPM"])
    }
    pub fn death(&self, id: &str) -> Option<&EventFact> {
        self.first_dated(id, &["DEAT", "BURI", "CREM"])
    }
    pub fn has_death_event(&self, id: &str) -> bool {
        self.person_events(id)
            .any(|e| matches!(e.kind.as_str(), "DEAT" | "BURI" | "CREM"))
    }
    pub fn display_name(&self, id: &str) -> String {
        self.persons
            .get(id)
            .map(|p| format!("{} {}", p.given, p.surname).trim().to_string())
            .unwrap_or_default()
    }
    pub fn place_name(&self, event: &EventFact) -> Option<&String> {
        event.place.as_ref().and_then(|p| self.places.get(p))
    }
}

pub fn years_between(from_jdn: i64, to_jdn: i64) -> f64 {
    (to_jdn - from_jdn) as f64 / DAYS_PER_YEAR
}
