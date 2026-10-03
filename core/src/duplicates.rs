//! Duplicate-person detection (blocked, scored) and full-rewiring merge.

use crate::facts::Facts;
use crate::name::{cologne, fold, similarity, soundex};
use crate::relationship::Sex;
use crate::store::{Result, Row, Store, Tx};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Candidate {
    pub a: String,
    pub b: String,
    pub score: f64,
    pub reasons: Vec<String>,
}

fn year_of(f: &Facts, id: &str) -> Option<i32> {
    f.birth(id)
        .and_then(|e| e.start)
        .map(|k| crate::date::jdn_to_gregorian(k).0)
}

fn names_of(f: &Facts, id: &str) -> Vec<String> {
    f.graph.people[id]
        .parents
        .iter()
        .map(|(p, _, _)| fold(&f.display_name(p)))
        .collect()
}

fn spouse_names(f: &Facts, id: &str) -> Vec<String> {
    f.graph.people[id]
        .spouses
        .iter()
        .map(|p| fold(&f.display_name(p)))
        .collect()
}

/// Score a pair in 0..1; returns None when they are clearly different people.
pub fn score_pair(f: &Facts, a: &str, b: &str) -> Option<Candidate> {
    let (pa, pb) = (&f.persons[a], &f.persons[b]);
    let mut reasons = vec![];
    if pa.sex != Sex::Unknown && pb.sex != Sex::Unknown && pa.sex != pb.sex {
        return None;
    }
    let given = similarity(&pa.given, &pb.given);
    let sur = similarity(&pa.surname, &pb.surname);
    let phon = soundex(&pa.surname) == soundex(&pb.surname)
        || cologne(&pa.surname) == cologne(&pb.surname);
    if sur < 0.6 && !phon {
        return None;
    }
    let mut name = 0.55 * given + 0.45 * sur.max(if phon { 0.85 } else { 0.0 });
    if !pa.given.is_empty() && fold(&pa.given) == fold(&pb.given) {
        name = name.max(0.9 * 0.55 + 0.45 * sur);
        reasons.push("same given name".into());
    }
    if sur > 0.99 {
        reasons.push("same surname".into());
    } else if phon {
        reasons.push("similar-sounding surname".into());
    }
    let mut score = name * 0.6;
    // Birth year
    match (year_of(f, a), year_of(f, b)) {
        (Some(x), Some(y)) => {
            let d = (x - y).abs();
            if d == 0 {
                score += 0.25;
                reasons.push("same birth year".into());
            } else if d <= 2 {
                score += 0.15;
                reasons.push("birth years within 2".into());
            } else if d > 5 {
                score -= 0.3;
                reasons.push("birth years differ".into());
            }
        }
        _ => score += 0.05,
    }
    // Birth place
    let place = |id: &str| f.birth(id).and_then(|e| f.place_name(e)).map(|p| fold(p));
    if let (Some(x), Some(y)) = (place(a), place(b)) {
        if x == y {
            score += 0.08;
            reasons.push("same birth place".into());
        }
    }
    // Death
    let dy = |id: &str| {
        f.death(id)
            .and_then(|e| e.start)
            .map(|k| crate::date::jdn_to_gregorian(k).0)
    };
    if let (Some(x), Some(y)) = (dy(a), dy(b)) {
        if (x - y).abs() <= 1 {
            score += 0.08;
            reasons.push("same death year".into());
        } else if (x - y).abs() > 5 {
            score -= 0.2;
        }
    }
    // Parents / spouses
    let (pna, pnb) = (names_of(f, a), names_of(f, b));
    if !pna.is_empty()
        && pna
            .iter()
            .any(|n| pnb.iter().any(|m| similarity(n, m) > 0.85))
    {
        score += 0.12;
        reasons.push("shared parent name".into());
    }
    let (sna, snb) = (spouse_names(f, a), spouse_names(f, b));
    if !sna.is_empty()
        && sna
            .iter()
            .any(|n| snb.iter().any(|m| similarity(n, m) > 0.85))
    {
        score += 0.1;
        reasons.push("same spouse name".into());
    }
    // Never suggest merging direct relatives
    if f.graph.is_ancestor_of(a, b)
        || f.graph.is_ancestor_of(b, a)
        || f.graph.people[a].spouses.iter().any(|s| s == b)
    {
        return None;
    }
    Some(Candidate {
        a: a.into(),
        b: b.into(),
        score: score.clamp(0.0, 1.0),
        reasons,
    })
}

fn pair_key(a: &str, b: &str) -> String {
    if a <= b {
        format!("{}|{}", a, b)
    } else {
        format!("{}|{}", b, a)
    }
}

pub fn not_duplicates(store: &Store) -> Result<HashSet<String>> {
    let json: Option<String> = store
        .conn()
        .query_row(
            "SELECT value FROM setting WHERE id = 'not_duplicates'",
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

/// Remember that two persons are *not* duplicates.
pub fn mark_not_duplicate(store: &mut Store, a: &str, b: &str) -> Result<()> {
    let mut set = not_duplicates(store)?;
    set.insert(pair_key(a, b));
    let mut v: Vec<String> = set.into_iter().collect();
    v.sort();
    store.transact("Mark not duplicate", |tx| {
        let mut row = serde_json::Map::new();
        row.insert("id".into(), "not_duplicates".into());
        row.insert("value".into(), serde_json::to_string(&v)?.into());
        tx.put_row("setting", row)?;
        Ok(())
    })
}

/// Find likely duplicates. Candidates are blocked by (Soundex of surname, first letter of given name) and by
/// Cologne code so the cost stays near-linear on large trees.
pub fn find_duplicates(store: &Store, threshold: f64, limit: usize) -> Result<Vec<Candidate>> {
    let f = Facts::load(store)?;
    let skip = not_duplicates(store)?;
    let mut blocks: HashMap<String, Vec<&String>> = HashMap::new();
    for id in &f.order {
        let p = &f.persons[id];
        if p.surname.is_empty() && p.given.is_empty() {
            continue;
        }
        let first = fold(&p.given).chars().next().unwrap_or('_');
        blocks
            .entry(format!("s{}{}", soundex(&p.surname), first))
            .or_default()
            .push(id);
        blocks
            .entry(format!("c{}{}", cologne(&p.surname), first))
            .or_default()
            .push(id);
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = vec![];
    for ids in blocks.values() {
        if ids.len() > 400 {
            // pathological block (e.g. empty surnames): refine by birth year
            continue;
        }
        for (i, a) in ids.iter().enumerate() {
            for b in ids.iter().skip(i + 1) {
                let key = pair_key(a, b);
                if skip.contains(&key) || !seen.insert(key) {
                    continue;
                }
                if let Some(c) = score_pair(&f, a, b) {
                    if c.score >= threshold {
                        out.push(c);
                    }
                }
            }
        }
    }
    out.sort_by(|x, y| y.score.partial_cmp(&x.score).unwrap().then(x.a.cmp(&y.a)));
    out.truncate(limit);
    Ok(out)
}

fn set_cols(tx: &mut Tx, table: &str, id: &str, cols: &[(&str, Value)]) -> Result<()> {
    let mut row = serde_json::Map::new();
    row.insert("id".into(), id.into());
    for (k, v) in cols {
        row.insert(k.to_string(), v.clone());
    }
    tx.put_row(table, row)?;
    Ok(())
}

fn event_signature(tx: &Tx, id: &str) -> Result<String> {
    let r = tx.get("event", id)?.unwrap_or_default();
    Ok(format!(
        "{}|{}|{}|{}|{}",
        r["kind"], r["custom_kind"], r["date_json"], r["place_id"], r["value"]
    ))
}

/// Merge person `remove` into `keep`. Everything that referenced `remove` is rewired to `keep`; exact duplicate
/// names/events are collapsed (their citations, notes and media move to the surviving record). One undo step.
pub fn merge_persons(tx: &mut Tx, keep: &str, remove: &str) -> Result<()> {
    if keep == remove {
        return Ok(());
    }
    let (Some(k), Some(r)) = (tx.get("person", keep)?, tx.get("person", remove)?) else {
        return Err(crate::store::StoreError::Other(
            "merge: person not found".into(),
        ));
    };
    // scalar fields: fill gaps on `keep`
    let mut fill: Vec<(&str, Value)> = vec![];
    if k["sex"].as_str().unwrap_or("").is_empty() || k["sex"] == "U" {
        if let Some(s) = r["sex"].as_str().filter(|s| !s.is_empty() && *s != "U") {
            fill.push(("sex", s.into()));
        }
    }
    for c in ["ref_no", "color", "primary_media", "xref"] {
        if k[c].is_null() && !r[c].is_null() && c != "xref" {
            fill.push((c, r[c].clone()));
        }
    }
    for c in ["is_private", "bookmarked"] {
        if r[c].as_i64() == Some(1) && k[c].as_i64() != Some(1) {
            fill.push((c, 1.into()));
        }
    }
    if !fill.is_empty() {
        set_cols(tx, "person", keep, &fill)?;
    }

    // names: move, skipping exact duplicates
    let existing: Vec<Row> = tx
        .ids_where("person_name", "person_id", keep)?
        .into_iter()
        .filter_map(|i| tx.get("person_name", &i).ok().flatten())
        .collect();
    let sig = |n: &Row| {
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            n["kind"],
            n["prefix"],
            n["given"],
            n["surname_prefix"],
            n["surname"],
            n["suffix"],
            n["nickname"]
        )
    };
    let mut have: HashSet<String> = existing.iter().map(sig).collect();
    let mut next_order = existing.len() as i64;
    for nid in tx.ids_where("person_name", "person_id", remove)? {
        let n = tx.get("person_name", &nid)?.unwrap();
        if have.insert(sig(&n)) {
            set_cols(
                tx,
                "person_name",
                &nid,
                &[
                    ("person_id", keep.into()),
                    ("sort_order", next_order.into()),
                ],
            )?;
            next_order += 1;
        } else {
            reparent_links(tx, "person_name", &nid, None)?;
            tx.delete("person_name", &nid)?;
        }
    }

    // events
    let mut keep_events: HashMap<String, String> = HashMap::new();
    for eid in tx.ids_where("event", "owner_id", keep)? {
        if tx
            .get("event", &eid)?
            .map(|e| e["owner_type"] == "person")
            .unwrap_or(false)
        {
            keep_events.insert(event_signature(tx, &eid)?, eid);
        }
    }
    let mut order = keep_events.len() as i64 + 100;
    for eid in tx.ids_where("event", "owner_id", remove)? {
        if !tx
            .get("event", &eid)?
            .map(|e| e["owner_type"] == "person")
            .unwrap_or(false)
        {
            continue;
        }
        let s = event_signature(tx, &eid)?;
        match keep_events.get(&s) {
            Some(survivor) => {
                reparent_links(tx, "event", &eid, Some(survivor))?;
                tx.delete("event", &eid)?;
            }
            None => {
                set_cols(
                    tx,
                    "event",
                    &eid,
                    &[("owner_id", keep.into()), ("sort_order", order.into())],
                )?;
                order += 1;
            }
        }
    }

    // families
    for col in ["partner1", "partner2"] {
        for fid in tx.ids_where("family", col, remove)? {
            set_cols(tx, "family", &fid, &[(col, keep.into())])?;
        }
    }
    let mut kept_links: HashSet<String> = tx
        .ids_where("family_child", "person_id", keep)?
        .into_iter()
        .filter_map(|i| tx.get("family_child", &i).ok().flatten())
        .map(|r| r["family_id"].as_str().unwrap_or("").to_string())
        .collect();
    for cid in tx.ids_where("family_child", "person_id", remove)? {
        let fam = tx.get("family_child", &cid)?.unwrap()["family_id"]
            .as_str()
            .unwrap_or("")
            .to_string();
        if kept_links.insert(fam) {
            set_cols(tx, "family_child", &cid, &[("person_id", keep.into())])?;
        } else {
            tx.delete("family_child", &cid)?;
        }
    }
    // associations
    for (col, other) in [("person_id", "other_id"), ("other_id", "person_id")] {
        for aid in tx.ids_where("association", col, remove)? {
            let a = tx.get("association", &aid)?.unwrap();
            if a[other].as_str() == Some(keep) {
                tx.delete("association", &aid)?; // would become a self-association
            } else {
                set_cols(tx, "association", &aid, &[(col, keep.into())])?;
            }
        }
    }
    // generic links targeting the person
    reparent_links(tx, "person", remove, Some(keep))?;
    // preserved raw GEDCOM structures
    for rid in tx.ids_where("raw_tag", "owner_id", remove)? {
        if tx
            .get("raw_tag", &rid)?
            .map(|r| r["owner_type"] == "person")
            .unwrap_or(false)
        {
            set_cols(tx, "raw_tag", &rid, &[("owner_id", keep.into())])?;
        }
    }
    tx.delete("person", remove)?;
    Ok(())
}

/// Move (or, with `to == None`, drop) citations, notes, media, tags and tasks attached to a record.
fn reparent_links(tx: &mut Tx, target_type: &str, from: &str, to: Option<&str>) -> Result<()> {
    for t in [
        "citation",
        "note_link",
        "media_link",
        "tag_link",
        "task_link",
    ] {
        for lid in tx.ids_where(t, "target_id", from)? {
            if tx
                .get(t, &lid)?
                .map(|r| r["target_type"] == target_type)
                .unwrap_or(false)
            {
                match to {
                    Some(to) => set_cols(tx, t, &lid, &[("target_id", to.into())])?,
                    None => {
                        tx.delete(t, &lid)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Merge family `remove` into `keep` (same partners): children and events are moved, duplicate children collapsed.
pub fn merge_families(tx: &mut Tx, keep: &str, remove: &str) -> Result<()> {
    let have: HashSet<String> = tx
        .ids_where("family_child", "family_id", keep)?
        .into_iter()
        .filter_map(|i| tx.get("family_child", &i).ok().flatten())
        .map(|r| r["person_id"].as_str().unwrap_or("").to_string())
        .collect();
    let mut have = have;
    let mut order = have.len() as i64;
    for cid in tx.ids_where("family_child", "family_id", remove)? {
        let p = tx.get("family_child", &cid)?.unwrap()["person_id"]
            .as_str()
            .unwrap_or("")
            .to_string();
        if have.insert(p) {
            set_cols(
                tx,
                "family_child",
                &cid,
                &[("family_id", keep.into()), ("sort_order", order.into())],
            )?;
            order += 1;
        } else {
            tx.delete("family_child", &cid)?;
        }
    }
    let mut sigs: HashMap<String, String> = HashMap::new();
    for eid in tx.ids_where("event", "owner_id", keep)? {
        sigs.insert(event_signature(tx, &eid)?, eid);
    }
    for eid in tx.ids_where("event", "owner_id", remove)? {
        if !tx
            .get("event", &eid)?
            .map(|e| e["owner_type"] == "family")
            .unwrap_or(false)
        {
            continue;
        }
        match sigs.get(&event_signature(tx, &eid)?) {
            Some(s) => {
                reparent_links(tx, "event", &eid, Some(s))?;
                tx.delete("event", &eid)?;
            }
            None => set_cols(tx, "event", &eid, &[("owner_id", keep.into())])?,
        }
    }
    reparent_links(tx, "family", remove, Some(keep))?;
    tx.delete("family", remove)?;
    Ok(())
}
