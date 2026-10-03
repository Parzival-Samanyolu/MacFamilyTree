//! Plausibility checker: configurable rules with severity, jump-to ids, ignore list and auto-fix suggestions.

use crate::date::Qualifier;
use crate::facts::{years_between, Facts};
use crate::relationship::Sex;
use crate::store::{Result, Store, Tx};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Fix {
    /// Remove an exact duplicate event.
    DeleteEvent { event_id: String },
    /// Clear an event date that contradicts its owner's other dates.
    ClearDate { event_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    /// Stable id (rule + entities) used for the ignore list.
    pub id: String,
    pub rule: String,
    pub severity: Severity,
    pub entity_type: String,
    pub entity_id: String,
    pub related: Vec<String>,
    pub message: String,
    pub fix: Option<Fix>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rules {
    pub enabled: HashSet<String>,
    pub min_mother_age: f64,
    pub max_mother_age: f64,
    pub min_father_age: f64,
    pub max_father_age: f64,
    pub max_lifespan: f64,
    pub min_marriage_age: f64,
    /// Days after a father's death a child may still be born (gestation).
    pub posthumous_days: i64,
}

pub const ALL_RULES: &[&str] = &[
    "death_before_birth",
    "lifespan",
    "child_before_parent",
    "parent_too_young",
    "parent_too_old",
    "born_after_parent_death",
    "marriage_too_young",
    "marriage_before_birth",
    "marriage_after_death",
    "event_after_death",
    "event_before_birth",
    "ancestor_loop",
    "missing_sex",
    "orphan_person",
    "empty_family",
    "dangling_reference",
    "invalid_date",
    "duplicate_event",
    "multiple_births",
    "overlapping_residence",
    "sibling_spacing",
];

impl Default for Rules {
    fn default() -> Self {
        Rules {
            enabled: ALL_RULES.iter().map(|s| s.to_string()).collect(),
            min_mother_age: 12.0,
            max_mother_age: 55.0,
            min_father_age: 13.0,
            max_father_age: 80.0,
            max_lifespan: 110.0,
            min_marriage_age: 14.0,
            posthumous_days: 280,
        }
    }
}

type EventSig = (String, Option<String>, Option<String>, Option<String>);

struct Out<'a> {
    rules: &'a Rules,
    v: Vec<Finding>,
}

impl Out<'_> {
    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        rule: &str,
        sev: Severity,
        et: &str,
        eid: &str,
        related: &[&str],
        msg: String,
        fix: Option<Fix>,
    ) {
        if !self.rules.enabled.contains(rule) {
            return;
        }
        let mut id = format!("{}:{}", rule, eid);
        for r in related {
            id.push(':');
            id.push_str(r);
        }
        self.v.push(Finding {
            id,
            rule: rule.into(),
            severity: sev,
            entity_type: et.into(),
            entity_id: eid.into(),
            related: related.iter().map(|s| s.to_string()).collect(),
            message: msg,
            fix,
        });
    }
}

const ALLOWED_AFTER_DEATH: &[&str] = &["DEAT", "BURI", "CREM", "PROB", "WILL", "EVEN", "CENS"];

pub fn check(f: &Facts, rules: &Rules) -> Vec<Finding> {
    let mut o = Out { rules, v: vec![] };
    let name = |id: &str| f.display_name(id);

    for id in &f.order {
        let birth = f.birth(id);
        let death = f.death(id);
        if let (Some(b), Some(d)) = (birth, death) {
            if d.end.unwrap() < b.start.unwrap() {
                o.add(
                    "death_before_birth",
                    Severity::Error,
                    "person",
                    id,
                    &[],
                    format!("{} died before being born", name(id)),
                    None,
                );
            } else {
                let min_life = years_between(b.end.unwrap(), d.start.unwrap());
                if min_life > rules.max_lifespan {
                    o.add(
                        "lifespan",
                        Severity::Warning,
                        "person",
                        id,
                        &[],
                        format!("{} would have lived {:.0} years", name(id), min_life),
                        None,
                    );
                }
            }
        }
        if let Some(d) = death {
            for e in f.person_events(id) {
                if !ALLOWED_AFTER_DEATH.contains(&e.kind.as_str()) {
                    if let Some(s) = e.start {
                        if s as f64 > d.end.unwrap() as f64 + 366.0 {
                            o.add(
                                "event_after_death",
                                Severity::Warning,
                                "event",
                                &e.id,
                                &[id],
                                format!("{}: {} is dated after death", name(id), e.kind),
                                None,
                            );
                        }
                    }
                }
            }
        }
        if let Some(b) = birth {
            for e in f.person_events(id) {
                if e.id != b.id && !matches!(e.kind.as_str(), "BIRT" | "CHR" | "BAPM") {
                    if let Some(en) = e.end {
                        if en < b.start.unwrap() {
                            o.add(
                                "event_before_birth",
                                Severity::Warning,
                                "event",
                                &e.id,
                                &[id],
                                format!("{}: {} is dated before birth", name(id), e.kind),
                                None,
                            );
                        }
                    }
                }
            }
        }
        // Dates and duplicate events
        let evs: Vec<_> = f.person_events(id).collect();
        for e in &evs {
            if let Some(d) = &e.date {
                if d.a.is_none() && d.phrase.is_some() && d.verbatim {
                    o.add(
                        "invalid_date",
                        Severity::Info,
                        "event",
                        &e.id,
                        &[id],
                        format!(
                            "{}: date {:?} is not understood",
                            name(id),
                            d.phrase.as_deref().unwrap_or("")
                        ),
                        None,
                    );
                }
                if matches!(d.qualifier, Qualifier::Between | Qualifier::FromTo) {
                    if let (Some((a, _)), Some(b)) = (
                        d.a.map(|p| p.span(d.calendar)),
                        d.b.map(|p| p.span(d.calendar)),
                    ) {
                        if a > b.1 {
                            o.add(
                                "invalid_date",
                                Severity::Error,
                                "event",
                                &e.id,
                                &[id],
                                format!("{}: date range ends before it starts", name(id)),
                                Some(Fix::ClearDate {
                                    event_id: e.id.clone(),
                                }),
                            );
                        }
                    }
                }
            }
        }
        let mut seen: HashMap<EventSig, &str> = HashMap::new();
        for e in &evs {
            if e.custom.is_none() && e.kind != "EVEN" {
                let key = (
                    e.kind.clone(),
                    e.date.as_ref().map(|d| d.to_gedcom()),
                    e.place.clone(),
                    e.value.clone(),
                );
                if let Some(first) = seen.get(&key) {
                    o.add(
                        "duplicate_event",
                        Severity::Warning,
                        "event",
                        &e.id,
                        &[first, id],
                        format!("{}: duplicate {} event", name(id), e.kind),
                        Some(Fix::DeleteEvent {
                            event_id: e.id.clone(),
                        }),
                    );
                } else {
                    seen.insert(key, &e.id);
                }
            }
        }
        for kind in ["BIRT", "DEAT"] {
            let ds: Vec<_> = evs
                .iter()
                .filter(|e| e.kind == kind && e.start.is_some())
                .collect();
            if ds.len() > 1
                && ds
                    .windows(2)
                    .any(|w| w[0].start != w[1].start || w[0].end != w[1].end)
            {
                o.add(
                    "multiple_births",
                    Severity::Warning,
                    "person",
                    id,
                    &[],
                    format!("{} has {} conflicting {} dates", name(id), ds.len(), kind),
                    None,
                );
            }
        }
        let resi: Vec<_> = evs
            .iter()
            .filter(|e| e.kind == "RESI" && e.start.is_some() && e.end.is_some())
            .collect();
        for (i, a) in resi.iter().enumerate() {
            for b in resi.iter().skip(i + 1) {
                let both_ranges = |e: &&&crate::facts::EventFact| {
                    e.date
                        .as_ref()
                        .map(|d| matches!(d.qualifier, Qualifier::FromTo | Qualifier::Between))
                        .unwrap_or(false)
                };
                if both_ranges(a)
                    && both_ranges(b)
                    && a.place != b.place
                    && a.start.unwrap() < b.end.unwrap()
                    && b.start.unwrap() < a.end.unwrap()
                {
                    o.add(
                        "overlapping_residence",
                        Severity::Info,
                        "person",
                        id,
                        &[&a.id, &b.id],
                        format!("{} has overlapping residences", name(id)),
                        None,
                    );
                }
            }
        }
        // Graph shape
        let p = &f.graph.people[id];
        if p.parents.is_empty() && p.children.is_empty() && p.spouses.is_empty() {
            o.add(
                "orphan_person",
                Severity::Info,
                "person",
                id,
                &[],
                format!("{} is not connected to anyone", name(id)),
                None,
            );
        }
    }

    // Parent/child rules
    for fam in &f.families {
        for child in &fam.children {
            let cb = f.birth(child);
            for parent in &fam.partners {
                let sex = f.persons.get(parent).map(|p| p.sex).unwrap_or(Sex::Unknown);
                let (pb, pd) = (f.birth(parent), f.death(parent));
                if let (Some(c), Some(p)) = (cb, pb) {
                    if c.end.unwrap() < p.start.unwrap() {
                        o.add(
                            "child_before_parent",
                            Severity::Error,
                            "person",
                            child,
                            &[parent],
                            format!(
                                "{} was born before their parent {}",
                                name(child),
                                name(parent)
                            ),
                            None,
                        );
                    } else {
                        let (min_age_limit, max_age_limit, who) = if sex == Sex::Female {
                            (rules.min_mother_age, rules.max_mother_age, "mother")
                        } else {
                            (rules.min_father_age, rules.max_father_age, "father")
                        };
                        if years_between(p.start.unwrap(), c.end.unwrap()) < min_age_limit {
                            o.add(
                                "parent_too_young",
                                Severity::Warning,
                                "person",
                                child,
                                &[parent],
                                format!(
                                    "{} would have been under {:.0} when their {} {} was born",
                                    name(parent),
                                    min_age_limit,
                                    who,
                                    name(child)
                                ),
                                None,
                            );
                        }
                        if years_between(p.end.unwrap(), c.start.unwrap()) > max_age_limit {
                            o.add(
                                "parent_too_old",
                                Severity::Warning,
                                "person",
                                child,
                                &[parent],
                                format!(
                                    "{} would have been over {:.0} when {} was born",
                                    name(parent),
                                    max_age_limit,
                                    name(child)
                                ),
                                None,
                            );
                        }
                    }
                }
                if let (Some(c), Some(d)) = (cb, pd) {
                    let grace = if sex == Sex::Female {
                        0
                    } else {
                        rules.posthumous_days
                    };
                    if c.start.unwrap() > d.end.unwrap() + grace {
                        o.add(
                            "born_after_parent_death",
                            Severity::Error,
                            "person",
                            child,
                            &[parent],
                            format!(
                                "{} was born after the death of {}",
                                name(child),
                                name(parent)
                            ),
                            None,
                        );
                    }
                }
            }
        }
        // Siblings born impossibly close together (same mother, different days, <240 days)
        if let Some(mother) = fam.partners.iter().find(|p| {
            f.persons
                .get(*p)
                .map(|x| x.sex == Sex::Female)
                .unwrap_or(false)
        }) {
            let mut dated: Vec<(&String, i64)> = fam
                .children
                .iter()
                .filter_map(|c| {
                    f.birth(c)
                        .filter(|b| b.start == b.end)
                        .map(|b| (c, b.start.unwrap()))
                })
                .collect();
            dated.sort_by_key(|x| x.1);
            for w in dated.windows(2) {
                let gap = w[1].1 - w[0].1;
                if gap > 1 && gap < 240 {
                    o.add(
                        "sibling_spacing",
                        Severity::Warning,
                        "person",
                        w[1].0,
                        &[w[0].0, mother],
                        format!(
                            "{} and {} were born only {} days apart",
                            name(w[0].0),
                            name(w[1].0),
                            gap
                        ),
                        None,
                    );
                }
            }
        }
        // Marriage rules
        for e in fam
            .events
            .iter()
            .map(|&i| &f.events[i])
            .filter(|e| e.kind == "MARR" && e.start.is_some())
        {
            for p in &fam.partners {
                if let Some(b) = f.birth(p) {
                    if e.end.unwrap() < b.start.unwrap() {
                        o.add(
                            "marriage_before_birth",
                            Severity::Error,
                            "event",
                            &e.id,
                            &[p],
                            format!("{} married before being born", name(p)),
                            None,
                        );
                    } else if years_between(b.start.unwrap(), e.end.unwrap())
                        < rules.min_marriage_age
                    {
                        o.add(
                            "marriage_too_young",
                            Severity::Warning,
                            "event",
                            &e.id,
                            &[p],
                            format!(
                                "{} would have been under {:.0} at marriage",
                                name(p),
                                rules.min_marriage_age
                            ),
                            None,
                        );
                    }
                }
                if let Some(d) = f.death(p) {
                    if e.start.unwrap() > d.end.unwrap() {
                        o.add(
                            "marriage_after_death",
                            Severity::Error,
                            "event",
                            &e.id,
                            &[p],
                            format!("{} married after dying", name(p)),
                            None,
                        );
                    }
                }
            }
        }
        if fam.partners.is_empty() && fam.children.is_empty() {
            o.add(
                "empty_family",
                Severity::Warning,
                "family",
                &fam.id,
                &[],
                "Family has no members".into(),
                None,
            );
        }
        for p in &fam.partners {
            if f.persons
                .get(p)
                .map(|x| x.sex == Sex::Unknown)
                .unwrap_or(false)
            {
                o.add(
                    "missing_sex",
                    Severity::Info,
                    "person",
                    p,
                    &[&fam.id],
                    format!("{} has no recorded sex but is a partner", name(p)),
                    None,
                );
            }
        }
    }
    for id in f.graph.find_cycles() {
        o.add(
            "ancestor_loop",
            Severity::Error,
            "person",
            &id,
            &[],
            format!("{} is their own ancestor", name(&id)),
            None,
        );
    }
    o.v.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then(a.rule.cmp(&b.rule))
            .then(a.id.cmp(&b.id))
    });
    o.v
}

/// Database-level integrity: records pointing at things that no longer exist.
pub fn check_references(store: &Store, rules: &Rules) -> Result<Vec<Finding>> {
    let mut o = Out { rules, v: vec![] };
    let ids = |t: &str| -> Result<HashSet<String>> {
        Ok(store
            .rows(t)?
            .into_iter()
            .filter_map(|r| r["id"].as_str().map(String::from))
            .collect())
    };
    let persons = ids("person")?;
    let families = ids("family")?;
    let sources = ids("source")?;
    let media = ids("media")?;
    let notes = ids("note")?;
    let events = ids("event")?;
    let target_exists = |tt: &str, id: &str| match tt {
        "person" => persons.contains(id),
        "family" => families.contains(id),
        "event" => events.contains(id),
        "source" => sources.contains(id),
        "media" => media.contains(id),
        "note" => notes.contains(id),
        _ => true,
    };
    for e in store.rows("event")? {
        let (ot, oid) = (
            e["owner_type"].as_str().unwrap_or(""),
            e["owner_id"].as_str().unwrap_or(""),
        );
        if !target_exists(ot, oid) {
            o.add(
                "dangling_reference",
                Severity::Warning,
                "event",
                e["id"].as_str().unwrap_or(""),
                &[],
                format!("Event belongs to a missing {}", ot),
                Some(Fix::DeleteEvent {
                    event_id: e["id"].as_str().unwrap_or("").into(),
                }),
            );
        }
    }
    for c in store.rows("family_child")? {
        if !persons.contains(c["person_id"].as_str().unwrap_or(""))
            || !families.contains(c["family_id"].as_str().unwrap_or(""))
        {
            o.add(
                "dangling_reference",
                Severity::Warning,
                "family",
                c["family_id"].as_str().unwrap_or(""),
                &[c["id"].as_str().unwrap_or("")],
                "Child link points to a missing person or family".into(),
                None,
            );
        }
    }
    for c in store.rows("citation")? {
        if !sources.contains(c["source_id"].as_str().unwrap_or(""))
            || !target_exists(
                c["target_type"].as_str().unwrap_or(""),
                c["target_id"].as_str().unwrap_or(""),
            )
        {
            o.add(
                "dangling_reference",
                Severity::Warning,
                "citation",
                c["id"].as_str().unwrap_or(""),
                &[],
                "Citation points to a missing source or target".into(),
                None,
            );
        }
    }
    for (table, col, set) in [
        ("media_link", "media_id", &media),
        ("note_link", "note_id", &notes),
    ] {
        for l in store.rows(table)? {
            if !set.contains(l[col].as_str().unwrap_or(""))
                || !target_exists(
                    l["target_type"].as_str().unwrap_or(""),
                    l["target_id"].as_str().unwrap_or(""),
                )
            {
                o.add(
                    "dangling_reference",
                    Severity::Warning,
                    table,
                    l["id"].as_str().unwrap_or(""),
                    &[],
                    format!("{} points to a missing record", table),
                    None,
                );
            }
        }
    }
    Ok(o.v)
}

pub fn check_all(store: &Store, rules: &Rules) -> Result<Vec<Finding>> {
    let facts = Facts::load(store)?;
    let mut v = check(&facts, rules);
    v.extend(check_references(store, rules)?);
    let ignored = ignored_ids(store)?;
    v.retain(|f| !ignored.contains(&f.id));
    Ok(v)
}

pub fn ignored_ids(store: &Store) -> Result<HashSet<String>> {
    let json: Option<String> = store
        .conn()
        .query_row(
            "SELECT value FROM setting WHERE id = 'plausibility_ignore'",
            [],
            |r| r.get(0),
        )
        .ok();
    Ok(json
        .and_then(|j| serde_json::from_str::<Vec<String>>(&j).ok())
        .unwrap_or_default()
        .into_iter()
        .collect())
}

pub fn ignore(store: &mut Store, finding_id: &str) -> Result<()> {
    let mut set = ignored_ids(store)?;
    set.insert(finding_id.to_string());
    let mut v: Vec<String> = set.into_iter().collect();
    v.sort();
    store.transact("Ignore finding", |tx| {
        let mut row = serde_json::Map::new();
        row.insert("id".into(), "plausibility_ignore".into());
        row.insert("value".into(), serde_json::to_string(&v)?.into());
        tx.put_row("setting", row)?;
        Ok(())
    })
}

pub fn apply_fix(tx: &mut Tx, fix: &Fix) -> Result<()> {
    match fix {
        Fix::DeleteEvent { event_id } => {
            for t in [
                "citation",
                "note_link",
                "media_link",
                "tag_link",
                "task_link",
            ] {
                for r in tx.ids_where(t, "target_id", event_id)? {
                    tx.delete(t, &r)?;
                }
            }
            tx.delete("event", event_id)?;
        }
        Fix::ClearDate { event_id } => {
            let mut row = serde_json::Map::new();
            row.insert("id".into(), event_id.clone().into());
            for c in ["date_json", "date_sort", "date_sort_end"] {
                row.insert(c.into(), serde_json::Value::Null);
            }
            tx.put_row("event", row)?;
        }
    }
    Ok(())
}
