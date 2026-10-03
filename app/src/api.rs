use base64::{engine::general_purpose::STANDARD as B64, Engine};
use kintree_core::date::{GenDate, Locale};
use kintree_core::duplicates;
use kintree_core::facts::Facts;
use kintree_core::gedcom::{self, Charset, Dialect, ExportOptions, LivingPolicy, Version};
use kintree_core::kinship_terms::{describe_all, Lang};
use kintree_core::layout::{self, Direction, Options as LayoutOptions};
use kintree_core::name::{suggest_child_surname, NamingCulture, PersonName, Sex as NSex};
use kintree_core::places;
use kintree_core::quality::{self, Rules};
use kintree_core::relationship::{
    ahnentafel, birth_keys, blood_relations, number_descendants, relationship, DescendantNumbering,
    Graph, Kind, Kinship,
};
use kintree_core::stats;
use kintree_core::store::{new_id, Row, Store, StoreError, Tx};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl ApiError {
    fn new(code: &str, message: impl Into<String>) -> ApiError {
        ApiError {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        ApiError::new("store", e.to_string())
    }
}
impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        ApiError::new("bad_args", e.to_string())
    }
}
type Res = Result<Value, ApiError>;

#[derive(Default)]
pub struct Session {
    store: Option<Store>,
    path: Option<PathBuf>,
}

impl Session {
    pub fn new() -> Session {
        Session::default()
    }
    fn store(&self) -> Result<&Store, ApiError> {
        self.store
            .as_ref()
            .ok_or_else(|| ApiError::new("no_project", "no project is open"))
    }
    fn store_mut(&mut self) -> Result<&mut Store, ApiError> {
        self.store
            .as_mut()
            .ok_or_else(|| ApiError::new("no_project", "no project is open"))
    }
}

fn s(args: &Value, k: &str) -> Option<String> {
    args.get(k).and_then(|v| v.as_str()).map(String::from)
}
fn req(args: &Value, k: &str) -> Result<String, ApiError> {
    s(args, k).ok_or_else(|| ApiError::new("bad_args", format!("missing `{}`", k)))
}
fn u(args: &Value, k: &str, d: u64) -> u64 {
    args.get(k).and_then(|v| v.as_u64()).unwrap_or(d)
}
fn b(args: &Value, k: &str, d: bool) -> bool {
    args.get(k).and_then(|v| v.as_bool()).unwrap_or(d)
}
fn row(pairs: &[(&str, Value)]) -> Row {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v.clone());
    }
    m
}
fn rs(r: &Row, k: &str) -> String {
    r.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

fn pref(store: &Store, key: &str) -> Option<String> {
    store
        .conn()
        .query_row(
            "SELECT value FROM setting WHERE id = ?1",
            [format!("pref:{}", key)],
            |r| r.get(0),
        )
        .ok()
}
fn locale(store: &Store) -> Locale {
    if pref(store, "lang").as_deref() == Some("tr") {
        Locale::Tr
    } else {
        Locale::En
    }
}
fn current_year() -> i32 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (1970.0 + secs as f64 / 31_556_952.0) as i32
}

// ---------- shared JSON builders ----------

fn date_info(json: Option<&str>, loc: Locale) -> (Option<String>, Option<i32>) {
    let Some(d) = json.and_then(|j| serde_json::from_str::<GenDate>(j).ok()) else {
        return (None, None);
    };
    (Some(d.format(loc)), d.gregorian_year())
}

/// Summary of one person (for lists, tree cards, search results).
fn summary(store: &Store, id: &str) -> Result<Value, ApiError> {
    let conn = store.conn();
    let loc = locale(store);
    let p = conn
        .query_row(
            "SELECT sex, bookmarked, color, is_private, living_override FROM person WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                ))
            },
        )
        .map_err(|_| ApiError::new("not_found", format!("person {}", id)))?;
    let (given, surname): (String, String) = conn
        .query_row("SELECT COALESCE(given,''), COALESCE(surname,'') FROM person_name WHERE person_id = ?1 ORDER BY sort_order, rowid LIMIT 1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap_or_default();
    let ev = |kinds: &[&str]| -> (Option<String>, Option<i32>, Option<String>) {
        for k in kinds {
            let r: Option<(Option<String>, Option<String>)> = conn
                .query_row(
                    "SELECT date_json, place_id FROM event WHERE owner_type='person' AND owner_id=?1 AND kind=?2 ORDER BY (date_json IS NULL), rowid LIMIT 1",
                    rusqlite::params![id, k],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
            if let Some((dj, pl)) = r {
                let (t, y) = date_info(dj.as_deref(), loc);
                if t.is_some() || pl.is_some() {
                    return (t, y, pl);
                }
            }
        }
        (None, None, None)
    };
    let (bt, by, _) = ev(&["BIRT", "CHR", "BAPM"]);
    let (dt, dy, _) = ev(&["DEAT", "BURI", "CREM"]);
    let has_death: bool = conn
        .query_row("SELECT 1 FROM event WHERE owner_type='person' AND owner_id=?1 AND kind IN ('DEAT','BURI','CREM') LIMIT 1", [id], |_| Ok(()))
        .is_ok();
    let living = match p.4 {
        Some(v) => v != 0,
        None => !has_death && by.map(|y| y > current_year() - 110).unwrap_or(true),
    };
    let life = match (by, dy) {
        (Some(b), Some(d)) => format!("{}–{}", b, d),
        (Some(b), None) if living => format!("{}–", b),
        (Some(b), None) => format!("{}", b),
        (None, Some(d)) => format!("–{}", d),
        _ => String::new(),
    };
    Ok(json!({
        "id": id, "given": given, "surname": surname,
        "name": format!("{} {}", given, surname).trim(),
        "sex": p.0, "bookmarked": p.1 != 0, "color": p.2, "private": p.3 != 0,
        "birth_text": bt, "birth_year": by, "death_text": dt, "death_year": dy,
        "living": living, "life": life
    }))
}

fn citations_of(store: &Store, ttype: &str, tid: &str) -> Result<Vec<Value>, ApiError> {
    let mut out = vec![];
    for c in store.rows_where("citation", "target_id", tid)? {
        if rs(&c, "target_type") != ttype {
            continue;
        }
        let title = store
            .rows_where("source", "id", &rs(&c, "source_id"))?
            .first()
            .map(|r| rs(r, "title"))
            .unwrap_or_default();
        out.push(json!({"id": c["id"], "source_id": c["source_id"], "source_title": title, "page": c["page"], "quality": c["quality"]}));
    }
    Ok(out)
}

fn notes_of(store: &Store, ttype: &str, tid: &str) -> Result<Vec<Value>, ApiError> {
    let mut out = vec![];
    for l in store.rows_where("note_link", "target_id", tid)? {
        if rs(&l, "target_type") != ttype {
            continue;
        }
        if let Some(n) = store.rows_where("note", "id", &rs(&l, "note_id"))?.first() {
            out.push(
                json!({"id": n["id"], "link_id": l["id"], "title": n["title"], "body": n["body"]}),
            );
        }
    }
    Ok(out)
}

fn event_json(
    store: &Store,
    e: &Row,
    places: &HashMap<String, String>,
    birth_jdn: Option<i64>,
) -> Result<Value, ApiError> {
    let loc = locale(store);
    let date: Option<GenDate> = e["date_json"]
        .as_str()
        .and_then(|j| serde_json::from_str(j).ok());
    let age = match (birth_jdn, date.as_ref().and_then(|d| d.sort_key())) {
        (Some(b), Some(d))
            if rs(e, "owner_type") == "person" && rs(e, "kind") != "BIRT" && d >= b =>
        {
            Some(((d - b) as f64 / 365.2425).floor() as i64)
        }
        _ => None,
    };
    let id = rs(e, "id");
    Ok(json!({
        "id": id, "owner_type": e["owner_type"], "owner_id": e["owner_id"], "kind": e["kind"], "custom_kind": e["custom_kind"],
        "value": e["value"], "date_text": date.as_ref().map(|d| d.format(loc)), "date_gedcom": date.as_ref().map(|d| d.to_gedcom()),
        "date_sort": e["date_sort"], "place_id": e["place_id"],
        "place_text": e["place_id"].as_str().and_then(|p| places.get(p)),
        "cause": e["cause"], "agency": e["agency"], "age": age,
        "citations": citations_of(store, "event", &id)?, "notes": notes_of(store, "event", &id)?
    }))
}

fn person_events(
    store: &Store,
    owner_type: &str,
    owner_id: &str,
    places: &HashMap<String, String>,
    birth: Option<i64>,
) -> Result<Vec<Value>, ApiError> {
    let mut evs: Vec<Row> = store
        .rows_where("event", "owner_id", owner_id)?
        .into_iter()
        .filter(|e| rs(e, "owner_type") == owner_type)
        .collect();
    // chronological with manual order as tie-breaker; undated events keep their stored order after dated ones
    evs.sort_by_key(|e| {
        (
            e["date_sort"].as_i64().is_none(),
            e["date_sort"].as_i64().unwrap_or(0),
            e["sort_order"].as_i64().unwrap_or(0),
        )
    });
    evs.iter()
        .map(|e| event_json(store, e, places, birth))
        .collect()
}

fn person_detail(store: &Store, id: &str) -> Res {
    let places = places::full_names(store)?;
    let p = store
        .rows_where("person", "id", id)?
        .into_iter()
        .next()
        .ok_or_else(|| ApiError::new("not_found", format!("person {}", id)))?;
    let facts_birth = store
        .rows_where("event", "owner_id", id)?
        .into_iter()
        .filter(|e| {
            rs(e, "owner_type") == "person"
                && matches!(rs(e, "kind").as_str(), "BIRT" | "CHR" | "BAPM")
                && !e["date_sort"].is_null()
        })
        .map(|e| e["date_sort"].as_i64().unwrap_or(0))
        .min();
    let names: Vec<Value> = store
        .rows_where("person_name", "person_id", id)?
        .into_iter()
        .map(Value::Object)
        .collect();
    let events = person_events(store, "person", id, &places, facts_birth)?;
    let mut partner_families = vec![];
    let mut child_families = vec![];
    for f in store.rows("family")? {
        let fid = rs(&f, "id");
        let partners: Vec<String> = ["partner1", "partner2"]
            .iter()
            .filter_map(|k| f[*k].as_str().map(String::from))
            .collect();
        let kids: Vec<Row> = store.rows_where("family_child", "family_id", &fid)?;
        if partners.iter().any(|p| p == id) {
            let others: Vec<Value> = partners
                .iter()
                .filter(|p| *p != id)
                .map(|p| summary(store, p))
                .collect::<Result<_, _>>()?;
            let mut kids_sorted = kids.clone();
            kids_sorted.sort_by_key(|k| k["sort_order"].as_i64().unwrap_or(0));
            let children: Vec<Value> = kids_sorted
                .iter()
                .map(|k| -> Result<Value, ApiError> {
                    let mut v = summary(store, &rs(k, "person_id"))?;
                    v["link_id"] = k["id"].clone();
                    v["rel_type"] = k["rel_type"].clone();
                    Ok(v)
                })
                .collect::<Result<_, _>>()?;
            partner_families.push(json!({
                "id": fid, "rel_type": f["rel_type"], "partners": others, "children": children,
                "events": person_events(store, "family", &fid, &places, None)?,
                "notes": notes_of(store, "family", &fid)?, "citations": citations_of(store, "family", &fid)?
            }));
        }
        if kids.iter().any(|k| rs(k, "person_id") == id) {
            let parents: Vec<Value> = partners
                .iter()
                .map(|p| summary(store, p))
                .collect::<Result<_, _>>()?;
            let siblings: Vec<Value> = kids
                .iter()
                .filter(|k| rs(k, "person_id") != id)
                .map(|k| summary(store, &rs(k, "person_id")))
                .collect::<Result<_, _>>()?;
            let link = kids.iter().find(|k| rs(k, "person_id") == id).unwrap();
            child_families.push(json!({"id": fid, "link_id": link["id"], "rel_type": link["rel_type"], "parents": parents, "siblings": siblings}));
        }
    }
    let assoc: Vec<Value> = store
        .rows_where("association", "person_id", id)?
        .into_iter()
        .map(|a| -> Result<Value, ApiError> { Ok(json!({"id": a["id"], "role": a["role"], "notes": a["notes"], "other": summary(store, &rs(&a, "other_id"))?})) })
        .collect::<Result<_, _>>()?;
    let mut tasks = vec![];
    for l in store.rows_where("task_link", "target_id", id)? {
        if let Some(t) = store
            .rows_where("task", "id", &rs(&l, "task_id"))?
            .into_iter()
            .next()
        {
            tasks.push(Value::Object(t));
        }
    }
    let mut media = vec![];
    for l in store.rows_where("media_link", "target_id", id)? {
        if rs(&l, "target_type") == "person" {
            if let Some(m) = store
                .rows_where("media", "id", &rs(&l, "media_id"))?
                .into_iter()
                .next()
            {
                media.push(json!({"id": m["id"], "path": m["path"], "caption": m["caption"], "kind": m["kind"], "link_id": l["id"]}));
            }
        }
    }
    let is_living = summary(store, id)?["living"].clone();
    Ok(json!({
        "person": {"id": id, "sex": p["sex"], "is_private": p["is_private"].as_i64() == Some(1), "bookmarked": p["bookmarked"].as_i64() == Some(1),
                   "color": p["color"], "ref_no": p["ref_no"], "living_override": p["living_override"], "living": is_living, "created": p["created"], "modified": p["modified"]},
        "summary": summary(store, id)?,
        "names": names, "events": events,
        "partner_families": partner_families, "child_families": child_families,
        "notes": notes_of(store, "person", id)?, "citations": citations_of(store, "person", id)?,
        "associations": assoc, "tasks": tasks, "media": media,
    }))
}

// ---------- list / search ----------

fn list_persons(store: &Store, args: &Value) -> Res {
    let offset = u(args, "offset", 0) as usize;
    let limit = u(args, "limit", 50).min(500) as usize;
    let query = s(args, "query").unwrap_or_default();
    let sort = s(args, "sort").unwrap_or_else(|| "name".into());
    let only_bookmarked = b(args, "bookmarked", false);
    let conn = store.conn();
    let ids: Vec<String>;
    let total: i64;
    if !query.trim().is_empty() {
        let all = store.search_persons(&query, 2000)?;
        total = all.len() as i64;
        ids = all.into_iter().skip(offset).take(limit).collect();
    } else {
        let where_ = if only_bookmarked {
            "WHERE p.bookmarked = 1"
        } else {
            ""
        };
        total = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM person p {}", where_),
                [],
                |r| r.get(0),
            )
            .map_err(StoreError::from)?;
        let order = match sort.as_str() {
            "birth" => "(SELECT MIN(date_sort) FROM event e WHERE e.owner_type='person' AND e.owner_id=p.id AND e.kind='BIRT') IS NULL, (SELECT MIN(date_sort) FROM event e WHERE e.owner_type='person' AND e.owner_id=p.id AND e.kind='BIRT'), p.rowid",
            "recent" => "p.modified DESC, p.rowid",
            "added" => "p.rowid",
            _ => "(SELECT lower(COALESCE(surname,'')) || ' ' || lower(COALESCE(given,'')) FROM person_name n WHERE n.person_id=p.id ORDER BY n.sort_order, n.rowid LIMIT 1), p.rowid",
        };
        let sql = format!(
            "SELECT p.id FROM person p {} ORDER BY {} LIMIT ?1 OFFSET ?2",
            where_, order
        );
        let mut st = conn.prepare(&sql).map_err(StoreError::from)?;
        ids = st
            .query_map([limit as i64, offset as i64], |r| r.get::<_, String>(0))
            .map_err(StoreError::from)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)?;
    }
    let items: Vec<Value> = ids
        .iter()
        .map(|i| summary(store, i))
        .collect::<Result<_, _>>()?;
    Ok(json!({"total": total, "offset": offset, "items": items}))
}

// ---------- mutations ----------

fn put(tx: &mut Tx, table: &str, id: &str, pairs: &[(&str, Value)]) -> Result<(), ApiError> {
    let mut r = row(pairs);
    r.insert("id".into(), id.into());
    tx.put_row(table, r)?;
    Ok(())
}

fn nullable(v: Option<String>) -> Value {
    match v {
        Some(s) if !s.trim().is_empty() => Value::from(s.trim().to_string()),
        _ => Value::Null,
    }
}

fn put_event(tx: &mut Tx, args: &Value) -> Result<String, ApiError> {
    let id = s(args, "id").unwrap_or_else(new_id);
    let existing = tx.get("event", &id)?;
    let mut r = Row::new();
    r.insert("id".into(), id.clone().into());
    if existing.is_none() {
        r.insert("owner_type".into(), req(args, "owner_type")?.into());
        r.insert("owner_id".into(), req(args, "owner_id")?.into());
        let n = tx
            .ids_where("event", "owner_id", &req(args, "owner_id")?)?
            .len() as i64;
        r.insert("sort_order".into(), n.into());
    }
    if let Some(k) = s(args, "kind") {
        r.insert("kind".into(), k.into());
    } else if existing.is_none() {
        return Err(ApiError::new("bad_args", "missing `kind`"));
    }
    for (src, col) in [
        ("custom_kind", "custom_kind"),
        ("value", "value"),
        ("cause", "cause"),
        ("agency", "agency"),
    ] {
        if args.get(src).is_some() {
            r.insert(col.into(), nullable(s(args, src)));
        }
    }
    if args.get("date_text").is_some() {
        match s(args, "date_text").and_then(|t| GenDate::parse_lenient(&t)) {
            Some(d) => {
                r.insert("date_json".into(), serde_json::to_string(&d)?.into());
                r.insert(
                    "date_sort".into(),
                    d.sort_key().map(Value::from).unwrap_or(Value::Null),
                );
                r.insert(
                    "date_sort_end".into(),
                    d.range().map(|x| Value::from(x.1)).unwrap_or(Value::Null),
                );
            }
            None => {
                for c in ["date_json", "date_sort", "date_sort_end"] {
                    r.insert(c.into(), Value::Null);
                }
            }
        }
    }
    if args.get("place_text").is_some() {
        let pid = match s(args, "place_text") {
            Some(t) if !t.trim().is_empty() => places::find_or_create(tx, &t)?,
            _ => None,
        };
        r.insert(
            "place_id".into(),
            pid.map(Value::from).unwrap_or(Value::Null),
        );
    }
    tx.put_row("event", r)?;
    Ok(id)
}

fn delete_event(tx: &mut Tx, id: &str) -> Result<(), ApiError> {
    for t in [
        "citation",
        "note_link",
        "media_link",
        "tag_link",
        "task_link",
    ] {
        for r in tx.ids_where(t, "target_id", id)? {
            tx.delete(t, &r)?;
        }
    }
    for r in tx.ids_where("raw_tag", "owner_id", id)? {
        tx.delete("raw_tag", &r)?;
    }
    tx.delete("event", id)?;
    Ok(())
}

fn delete_family(tx: &mut Tx, id: &str) -> Result<(), ApiError> {
    for c in tx.ids_where("family_child", "family_id", id)? {
        tx.delete("family_child", &c)?;
    }
    for e in tx.ids_where("event", "owner_id", id)? {
        if tx
            .get("event", &e)?
            .map(|r| r["owner_type"] == "family")
            .unwrap_or(false)
        {
            delete_event(tx, &e)?;
        }
    }
    for t in [
        "citation",
        "note_link",
        "media_link",
        "tag_link",
        "task_link",
    ] {
        for r in tx.ids_where(t, "target_id", id)? {
            tx.delete(t, &r)?;
        }
    }
    tx.delete("family", id)?;
    Ok(())
}

fn culture(store: &Store) -> NamingCulture {
    match pref(store, "naming_culture").as_deref() {
        Some("patronymic") => NamingCulture::Patronymic,
        Some("spanish") => NamingCulture::SpanishDouble,
        Some("turkish") => NamingCulture::Turkish,
        Some("slavic") => NamingCulture::SlavicGendered,
        _ => NamingCulture::Patrilineal,
    }
}

fn first_name(tx: &Tx, pid: &str) -> Result<PersonName, ApiError> {
    let ids = tx.ids_where("person_name", "person_id", pid)?;
    let Some(id) = ids.first() else {
        return Ok(PersonName::default());
    };
    let r = tx.get("person_name", id)?.unwrap_or_default();
    Ok(PersonName {
        given: rs(&r, "given"),
        surname: rs(&r, "surname"),
        surname_prefix: rs(&r, "surname_prefix"),
        ..Default::default()
    })
}

fn family_partners(tx: &Tx, fid: &str) -> Result<Vec<String>, ApiError> {
    let f = tx.get("family", fid)?.unwrap_or_default();
    Ok(["partner1", "partner2"]
        .iter()
        .filter_map(|k| f[*k].as_str().map(String::from))
        .collect())
}

/// Add a relative with culture-aware surname defaults. Returns (new person id, family id).
fn add_relative(
    tx: &mut Tx,
    cult: NamingCulture,
    args: &Value,
) -> Result<(String, String), ApiError> {
    let anchor = req(args, "person_id")?;
    let kind = req(args, "kind")?;
    let given = s(args, "given").unwrap_or_default();
    let sex_s = s(args, "sex").unwrap_or_else(|| match kind.as_str() {
        "father" => "M".into(),
        "mother" => "F".into(),
        _ => "U".into(),
    });
    let sex = match sex_s.as_str() {
        "M" => NSex::Male,
        "F" => NSex::Female,
        _ => NSex::Unknown,
    };
    let anchor_name = first_name(tx, &anchor)?;
    // families the anchor belongs to
    let fam_as_child = || -> Result<Option<String>, ApiError> {
        match tx.ids_where("family_child", "person_id", &anchor)?.first() {
            Some(l) => Ok(tx.get("family_child", l)?.map(|r| rs(&r, "family_id"))),
            None => Ok(None),
        }
    };
    let fam_as_partner = |tx: &Tx| -> Result<Vec<String>, ApiError> {
        let mut v = tx.ids_where("family", "partner1", &anchor)?;
        v.extend(tx.ids_where("family", "partner2", &anchor)?);
        Ok(v)
    };
    let parents_names =
        |tx: &Tx, fid: &str| -> Result<(Option<PersonName>, Option<PersonName>), ApiError> {
            let mut father = None;
            let mut mother = None;
            for p in family_partners(tx, fid)? {
                let sx = tx
                    .get("person", &p)?
                    .map(|r| rs(&r, "sex"))
                    .unwrap_or_default();
                let n = first_name(tx, &p)?;
                if sx == "F" {
                    mother = Some(n);
                } else if father.is_none() {
                    father = Some(n);
                } else {
                    mother = Some(n);
                }
            }
            Ok((father, mother))
        };
    let (surname, fam_id, new_id_) = match kind.as_str() {
        "father" | "mother" => {
            let fid = match fam_as_child()? {
                Some(f) => f,
                None => {
                    let f = tx.create_family(None, None, "married")?;
                    tx.add_child(&f, &anchor, "")?;
                    f
                }
            };
            let partners = family_partners(tx, &fid)?;
            if partners.len() >= 2 {
                return Err(ApiError::new(
                    "conflict",
                    "this person's parent family already has two parents",
                ));
            }
            let surname = s(args, "surname").unwrap_or_else(|| {
                if kind == "father" {
                    anchor_name.surname.clone()
                } else {
                    String::new()
                }
            });
            let pid = tx.create_person(&PersonName::new(&given, &surname), &sex_s)?;
            put(
                tx,
                "family",
                &fid,
                &[(
                    if partners.is_empty() {
                        "partner1"
                    } else {
                        "partner2"
                    },
                    pid.clone().into(),
                )],
            )?;
            (surname, fid, pid)
        }
        "partner" => {
            let surname = s(args, "surname").unwrap_or_default();
            let pid = tx.create_person(&PersonName::new(&given, &surname), &sex_s)?;
            let fid = tx.create_family(
                Some(&anchor),
                Some(&pid),
                &s(args, "rel_type").unwrap_or_else(|| "married".into()),
            )?;
            (surname, fid, pid)
        }
        "child" => {
            let fid = match s(args, "family_id") {
                Some(f) => f,
                None => {
                    let fams = fam_as_partner(tx)?;
                    match fams.len() {
                        0 => tx.create_family(Some(&anchor), None, "unknown")?,
                        1 => fams[0].clone(),
                        _ => {
                            return Err(ApiError::new(
                                "choose_family",
                                "person has several partnerships; pass family_id",
                            ))
                        }
                    }
                }
            };
            let (father, mother) = parents_names(tx, &fid)?;
            let surname = s(args, "surname").unwrap_or_else(|| {
                suggest_child_surname(cult, father.as_ref(), mother.as_ref(), sex)
            });
            let pid = tx.create_person(&PersonName::new(&given, &surname), &sex_s)?;
            tx.add_child(&fid, &pid, &s(args, "rel_type").unwrap_or_default())?;
            (surname, fid, pid)
        }
        "sibling" => {
            let fid = match fam_as_child()? {
                Some(f) => f,
                None => {
                    let f = tx.create_family(None, None, "unknown")?;
                    tx.add_child(&f, &anchor, "")?;
                    f
                }
            };
            let (father, mother) = parents_names(tx, &fid)?;
            let default = if father.is_none() && mother.is_none() {
                anchor_name.surname.clone()
            } else {
                suggest_child_surname(cult, father.as_ref(), mother.as_ref(), sex)
            };
            let surname = s(args, "surname").unwrap_or(default);
            let pid = tx.create_person(&PersonName::new(&given, &surname), &sex_s)?;
            tx.add_child(&fid, &pid, "")?;
            (surname, fid, pid)
        }
        other => {
            return Err(ApiError::new(
                "bad_args",
                format!("unknown relative kind {}", other),
            ))
        }
    };
    let _ = surname;
    Ok((new_id_, fam_id))
}

// ---------- charts ----------

fn layout_opts(args: &Value) -> LayoutOptions {
    let mut o = LayoutOptions {
        ancestors: u(args, "ancestors", 4) as usize,
        descendants: u(args, "descendants", 3) as usize,
        show_spouses: b(args, "show_spouses", true),
        direction: if s(args, "direction").as_deref() == Some("lr") {
            Direction::LeftRight
        } else {
            Direction::TopDown
        },
        ..Default::default()
    };
    if let Some(w) = args.get("card_w").and_then(|v| v.as_f64()) {
        o.card_w = w;
    }
    if let Some(h) = args.get("card_h").and_then(|v| v.as_f64()) {
        o.card_h = h;
    }
    if let Some(c) = args.get("collapsed").and_then(|v| v.as_array()) {
        o.collapsed = c
            .iter()
            .filter_map(|x| x.as_str().map(String::from))
            .collect();
    }
    o
}

fn tree_layout(store: &Store, args: &Value) -> Res {
    let root = req(args, "root")?;
    let g = Graph::load(store)?;
    if !g.people.contains_key(&root) {
        return Err(ApiError::new("not_found", format!("person {}", root)));
    }
    let o = layout_opts(args);
    let births = birth_keys(store)?;
    let mode = s(args, "mode").unwrap_or_else(|| "hourglass".into());
    let l = match mode.as_str() {
        "ancestors" => layout::layout_ancestors(&g, &root, &o),
        "descendants" => layout::layout_descendants(&g, &births, &root, &o),
        _ => layout::layout_hourglass(&g, &births, &root, &o),
    };
    let mut people: Map<String, Value> = Map::new();
    for n in &l.nodes {
        if !people.contains_key(&n.person_id) {
            people.insert(n.person_id.clone(), summary(store, &n.person_id)?);
        }
    }
    Ok(json!({"layout": l, "people": people}))
}

fn ahnentafel_json(store: &Store, args: &Value) -> Res {
    let root = req(args, "root")?;
    let g = Graph::load(store)?;
    let gens = u(args, "generations", 5) as usize;
    let rows: Vec<Value> = ahnentafel(&g, &root, gens)
        .into_iter()
        .map(|(n, id)| -> Result<Value, ApiError> {
            Ok(json!({"n": n, "person": summary(store, &id)?}))
        })
        .collect::<Result<_, _>>()?;
    Ok(json!({"root": root, "items": rows}))
}

fn relationship_json(store: &Store, args: &Value) -> Res {
    let (a, bb) = (req(args, "a")?, req(args, "b")?);
    let lang = Lang::from_code(&s(args, "lang").unwrap_or_else(|| "en".into()));
    let g = Graph::load(store)?;
    for id in [&a, &bb] {
        if !g.people.contains_key(id) {
            return Err(ApiError::new("not_found", format!("person {}", id)));
        }
    }
    let rels = relationship(&g, &a, &bb);
    let names = describe_all(&rels, lang);
    let mut k = Kinship::new(&g);
    let facts = Facts::load(store)?;
    let blood = blood_relations(&g, &a, &bb);
    let routes: Vec<Value> = blood
        .iter()
        .map(|bl| json!({"up": bl.up, "down": bl.down, "half": bl.half(), "ancestors": bl.ancestors.iter().map(|x| json!({"id": x, "name": facts.display_name(x)})).collect::<Vec<_>>()}))
        .collect();
    let kind = match rels.first().map(|r| &r.kind) {
        Some(Kind::Blood(_)) => "blood",
        Some(Kind::Spouse) => "spouse",
        Some(Kind::Same) => "same",
        Some(Kind::Unrelated) | None => "none",
        Some(_) => "affinity",
    };
    Ok(
        json!({"descriptions": names, "kind": kind, "routes": routes, "relatedness": k.relatedness(&a, &bb), "inbreeding_a": k.inbreeding(&a), "inbreeding_b": k.inbreeding(&bb)}),
    )
}

// ---------- generic records ----------

const REC_TABLES: &[&str] = &[
    "source",
    "repository",
    "note",
    "task",
    "tag",
    "story",
    "journal",
    "place",
    "media",
    "citation",
    "note_link",
    "task_link",
    "tag_link",
    "media_link",
    "association",
];

fn rec_table(args: &Value) -> Result<String, ApiError> {
    let t = req(args, "table")?;
    if REC_TABLES.contains(&t.as_str()) {
        Ok(t)
    } else {
        Err(ApiError::new(
            "bad_args",
            format!("table `{}` not available", t),
        ))
    }
}

fn export_options(args: &Value) -> ExportOptions {
    ExportOptions {
        version: if s(args, "version").as_deref() == Some("7.0") {
            Version::V70
        } else {
            Version::V551
        },
        charset: match s(args, "charset").as_deref() {
            Some("utf16") => Charset::Utf16Le,
            Some("latin1") => Charset::Latin1,
            Some("ascii") => Charset::Ascii,
            _ => Charset::Utf8,
        },
        living: match s(args, "living").as_deref() {
            Some("mask") => LivingPolicy::Mask,
            Some("exclude") => LivingPolicy::Exclude,
            _ => LivingPolicy::Include,
        },
        living_years: u(args, "living_years", 110) as i32,
        current_year: current_year(),
        dialect: match s(args, "dialect").as_deref() {
            Some("ancestry") => Dialect::Ancestry,
            Some("ftm") => Dialect::FamilyTreeMaker,
            Some("rootsmagic") => Dialect::RootsMagic,
            Some("legacy") => Dialect::Legacy,
            Some("gramps") => Dialect::Gramps,
            _ => Dialect::Standard,
        },
        include_media: b(args, "include_media", true),
    }
}

fn status(sess: &Session) -> Value {
    match &sess.store {
        None => json!({"open": false}),
        Some(st) => json!({
            "open": true, "path": sess.path.as_ref().map(|p| p.display().to_string()),
            "persons": st.count("person").unwrap_or(0), "families": st.count("family").unwrap_or(0),
            "can_undo": st.can_undo(), "can_redo": st.can_redo(), "undo_label": st.undo_label()
        }),
    }
}

fn import_bytes(sess: &mut Session, bytes: &[u8]) -> Res {
    let st = sess.store_mut()?;
    let rep = gedcom::import(st, bytes)?;
    let issues: Vec<Value> = rep
        .issues
        .iter()
        .map(|i| json!({"severity": match i.severity { gedcom::Severity::Info => "info", gedcom::Severity::Warning => "warning", gedcom::Severity::Error => "error" }, "line": i.line, "message": i.message}))
        .collect();
    Ok(
        json!({"version": rep.version, "charset": format!("{:?}", rep.charset), "persons": rep.persons, "families": rep.families, "events": rep.events,
              "places": rep.places, "sources": rep.sources, "repositories": rep.repositories, "notes": rep.notes, "media": rep.media,
              "preserved_structures": rep.raw_structures, "issues": issues}),
    )
}

fn sample_gedcom() -> String {
    kintree_core::synth::generate_gedcom(30, 2024)
}

/// The single entry point. `cmd` is `area.verb`; `args` is a JSON object.
pub fn dispatch(sess: &mut Session, cmd: &str, args: Value) -> Res {
    match cmd {
        // ---- project ----
        "project.status" => Ok(status(sess)),
        "project.new" => {
            sess.store = Some(Store::open_memory()?);
            sess.path = None;
            Ok(status(sess))
        }
        "project.create" | "project.open" => {
            let p = PathBuf::from(req(&args, "path")?);
            if cmd == "project.open" && !p.exists() {
                return Err(ApiError::new(
                    "not_found",
                    format!("{} does not exist", p.display()),
                ));
            }
            sess.store = Some(Store::open(&p)?);
            sess.path = Some(p);
            Ok(status(sess))
        }
        "project.close" => {
            sess.store = None;
            sess.path = None;
            Ok(status(sess))
        }
        "project.load_sample" => {
            let n = u(&args, "persons", 30) as usize;
            sess.store = Some(Store::open_memory()?);
            sess.path = None;
            let text = if n <= 30 {
                sample_gedcom()
            } else {
                kintree_core::synth::generate_gedcom(n, 2024)
            };
            import_bytes(sess, text.as_bytes())?;
            Ok(status(sess))
        }
        "gedcom.import" => {
            let bytes = B64
                .decode(req(&args, "data")?)
                .map_err(|e| ApiError::new("bad_args", e.to_string()))?;
            if sess.store.is_none() {
                sess.store = Some(Store::open_memory()?);
            }
            let r = import_bytes(sess, &bytes)?;
            Ok(json!({"report": r, "status": status(sess)}))
        }
        "gedcom.export" => {
            let st = sess.store()?;
            let o = export_options(&args);
            let bytes = gedcom::export(st, &o)?;
            Ok(json!({"data": B64.encode(&bytes), "size": bytes.len()}))
        }
        // ---- history ----
        "history.undo" => {
            let l = sess.store_mut()?.undo()?;
            Ok(json!({"label": l, "status": status(sess)}))
        }
        "history.redo" => {
            let l = sess.store_mut()?.redo()?;
            Ok(json!({"label": l, "status": status(sess)}))
        }
        "history.list" => {
            let st = sess.store()?;
            let mut q = st.conn().prepare("SELECT id, group_id, label, ts, undone FROM history ORDER BY id DESC LIMIT 100").map_err(StoreError::from)?;
            let v: Vec<Value> = q
                .query_map([], |r| Ok(json!({"id": r.get::<_, i64>(0)?, "group": r.get::<_, Option<i64>>(1)?, "label": r.get::<_, String>(2)?, "ts": r.get::<_, i64>(3)?, "undone": r.get::<_, i64>(4)? != 0})))
                .map_err(StoreError::from)?
                .collect::<Result<_, _>>()
                .map_err(StoreError::from)?;
            Ok(json!(v))
        }
        // ---- settings ----
        "settings.get" => {
            let st = sess.store()?;
            let mut m = Map::new();
            for r in st.rows("setting")? {
                if let Some(k) = rs(&r, "id").strip_prefix("pref:") {
                    m.insert(k.to_string(), r["value"].clone());
                }
            }
            Ok(Value::Object(m))
        }
        "settings.set" => {
            let key = req(&args, "key")?;
            let val = args.get("value").cloned().unwrap_or(Value::Null);
            let v = match val {
                Value::String(s) => s,
                other => other.to_string(),
            };
            sess.store_mut()?.transact("Change setting", |tx| {
                let mut r = Row::new();
                r.insert("id".into(), format!("pref:{}", key).into());
                r.insert("value".into(), v.into());
                tx.put_row("setting", r)?;
                Ok(())
            })?;
            Ok(json!({"ok": true}))
        }
        // ---- persons ----
        "person.list" => list_persons(sess.store()?, &args),
        "person.get" => person_detail(sess.store()?, &req(&args, "id")?),
        "person.summary" => summary(sess.store()?, &req(&args, "id")?),
        "search" => {
            let st = sess.store()?;
            let q = req(&args, "q")?;
            let ids = st.search_persons(&q, u(&args, "limit", 20) as usize)?;
            let items: Vec<Value> = ids
                .iter()
                .map(|i| summary(st, i))
                .collect::<Result<_, _>>()?;
            Ok(json!(items))
        }
        "person.create" => {
            let name = PersonName::new(
                &s(&args, "given").unwrap_or_default(),
                &s(&args, "surname").unwrap_or_default(),
            );
            let sex = s(&args, "sex").unwrap_or_else(|| "U".into());
            let id = sess
                .store_mut()?
                .transact("Add person", |tx| tx.create_person(&name, &sex))?;
            Ok(json!({"id": id}))
        }
        "person.update" => {
            let id = req(&args, "id")?;
            sess.store_mut()?.transact("Edit person", |tx| {
                let mut r = Row::new();
                r.insert("id".into(), id.clone().into());
                if let Some(v) = s(&args, "sex") {
                    r.insert("sex".into(), v.into());
                }
                for k in ["is_private", "bookmarked"] {
                    if let Some(v) = args.get(k).and_then(|v| v.as_bool()) {
                        r.insert(k.into(), (v as i64).into());
                    }
                }
                for k in ["color", "ref_no"] {
                    if args.get(k).is_some() {
                        r.insert(k.into(), nullable(s(&args, k)));
                    }
                }
                if args.get("living_override").is_some() {
                    r.insert(
                        "living_override".into(),
                        args["living_override"]
                            .as_bool()
                            .map(|b| Value::from(b as i64))
                            .unwrap_or(Value::Null),
                    );
                }
                tx.put_row("person", r)?;
                Ok(())
            })?;
            Ok(json!({"id": id}))
        }
        "person.delete" => {
            let id = req(&args, "id")?;
            sess.store_mut()?
                .transact("Delete person", |tx| tx.delete_person(&id))?;
            Ok(json!({"ok": true}))
        }
        "name.put" => {
            let pid = req(&args, "person_id")?;
            let nid = s(&args, "id");
            let out = sess.store_mut()?.transact(
                if nid.is_some() {
                    "Edit name"
                } else {
                    "Add name"
                },
                |tx| {
                    let id = nid.clone().unwrap_or_else(new_id);
                    let mut r = Row::new();
                    r.insert("id".into(), id.clone().into());
                    r.insert("person_id".into(), pid.clone().into());
                    for k in [
                        "kind",
                        "prefix",
                        "given",
                        "nickname",
                        "surname_prefix",
                        "surname",
                        "suffix",
                    ] {
                        if args.get(k).is_some() {
                            r.insert(k.into(), s(&args, k).unwrap_or_default().into());
                        }
                    }
                    if nid.is_none() {
                        r.insert(
                            "sort_order".into(),
                            (tx.ids_where("person_name", "person_id", &pid)?.len() as i64).into(),
                        );
                        r.entry("kind").or_insert("Birth".into());
                    }
                    tx.put_row("person_name", r)?;
                    Ok(id)
                },
            )?;
            Ok(json!({"id": out}))
        }
        "name.delete" => {
            let id = req(&args, "id")?;
            sess.store_mut()?.transact("Delete name", |tx| {
                tx.delete("person_name", &id)?;
                Ok(())
            })?;
            Ok(json!({"ok": true}))
        }
        "name.set_primary" => {
            let pid = req(&args, "person_id")?;
            let id = req(&args, "id")?;
            sess.store_mut()?.transact("Reorder names", |tx| {
                let mut order = 1;
                for n in tx.ids_where("person_name", "person_id", &pid)? {
                    let o = if n == id {
                        0
                    } else {
                        order += 1;
                        order
                    };
                    let mut r = Row::new();
                    r.insert("id".into(), n.into());
                    r.insert("sort_order".into(), o.into());
                    tx.put_row("person_name", r)?;
                }
                Ok(())
            })?;
            Ok(json!({"ok": true}))
        }
        // ---- events ----
        "event.put" => {
            let label = if s(&args, "id").is_some() {
                "Edit event"
            } else {
                "Add event"
            };
            let id = sess.store_mut()?.transact(label, |tx| {
                put_event(tx, &args).map_err(|e| StoreError::Other(e.message))
            })?;
            Ok(json!({"id": id}))
        }
        "event.delete" => {
            let id = req(&args, "id")?;
            sess.store_mut()?.transact("Delete event", |tx| {
                delete_event(tx, &id).map_err(|e| StoreError::Other(e.message))
            })?;
            Ok(json!({"ok": true}))
        }
        "event.reorder" => {
            let ids: Vec<String> = args["ids"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            sess.store_mut()?.transact("Reorder events", |tx| {
                for (i, id) in ids.iter().enumerate() {
                    let mut r = Row::new();
                    r.insert("id".into(), id.clone().into());
                    r.insert("sort_order".into(), (i as i64).into());
                    tx.put_row("event", r)?;
                }
                Ok(())
            })?;
            Ok(json!({"ok": true}))
        }
        // ---- families ----
        "family.create" => {
            let (p1, p2) = (s(&args, "partner1"), s(&args, "partner2"));
            let rel = s(&args, "rel_type").unwrap_or_else(|| "married".into());
            let id = sess.store_mut()?.transact("Add family", |tx| {
                tx.create_family(p1.as_deref(), p2.as_deref(), &rel)
            })?;
            Ok(json!({"id": id}))
        }
        "family.update" => {
            let id = req(&args, "id")?;
            sess.store_mut()?.transact("Edit family", |tx| {
                let mut r = Row::new();
                r.insert("id".into(), id.clone().into());
                for k in ["partner1", "partner2"] {
                    if args.get(k).is_some() {
                        r.insert(k.into(), nullable(s(&args, k)));
                    }
                }
                if let Some(v) = s(&args, "rel_type") {
                    r.insert("rel_type".into(), v.into());
                }
                tx.put_row("family", r)?;
                Ok(())
            })?;
            Ok(json!({"id": id}))
        }
        "family.delete" => {
            let id = req(&args, "id")?;
            sess.store_mut()?.transact("Delete family", |tx| {
                delete_family(tx, &id).map_err(|e| StoreError::Other(e.message))
            })?;
            Ok(json!({"ok": true}))
        }
        "family.add_child" => {
            let (fid, pid) = (req(&args, "family_id")?, req(&args, "person_id")?);
            let rel = s(&args, "rel_type").unwrap_or_default();
            let id = sess
                .store_mut()?
                .transact("Add child", |tx| tx.add_child(&fid, &pid, &rel))?;
            Ok(json!({"id": id}))
        }
        "family.set_child" => {
            let id = req(&args, "id")?;
            sess.store_mut()?.transact("Edit child link", |tx| {
                let mut r = Row::new();
                r.insert("id".into(), id.clone().into());
                if let Some(v) = s(&args, "rel_type") {
                    r.insert("rel_type".into(), v.into());
                }
                tx.put_row("family_child", r)?;
                Ok(())
            })?;
            Ok(json!({"ok": true}))
        }
        "family.remove_child" => {
            let id = req(&args, "id")?;
            sess.store_mut()?.transact("Remove child", |tx| {
                tx.delete("family_child", &id)?;
                Ok(())
            })?;
            Ok(json!({"ok": true}))
        }
        "family.reorder_children" => {
            let ids: Vec<String> = args["ids"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            sess.store_mut()?.transact("Reorder children", |tx| {
                for (i, id) in ids.iter().enumerate() {
                    let mut r = Row::new();
                    r.insert("id".into(), id.clone().into());
                    r.insert("sort_order".into(), (i as i64).into());
                    tx.put_row("family_child", r)?;
                }
                Ok(())
            })?;
            Ok(json!({"ok": true}))
        }
        "relative.add" => {
            let label = format!("Add {}", s(&args, "kind").unwrap_or_default());
            let cult = culture(sess.store()?);
            let (pid, fid) = sess.store_mut()?.transact(&label, |tx| {
                add_relative(tx, cult, &args)
                    .map_err(|e| StoreError::Other(format!("{}: {}", e.code, e.message)))
            })?;
            Ok(json!({"person_id": pid, "family_id": fid}))
        }
        // ---- charts ----
        "tree.layout" => tree_layout(sess.store()?, &args),
        "chart.ahnentafel" => ahnentafel_json(sess.store()?, &args),
        "chart.numbering" => {
            let st = sess.store()?;
            let root = req(&args, "root")?;
            let g = Graph::load(st)?;
            let sys = if s(&args, "system").as_deref() == Some("henry") {
                DescendantNumbering::Henry
            } else {
                DescendantNumbering::DAboville
            };
            let rows = number_descendants(
                &g,
                &birth_keys(st)?,
                &root,
                u(&args, "generations", 5) as usize,
                sys,
            );
            let items: Vec<Value> = rows
                .into_iter()
                .map(|(n, id)| -> Result<Value, ApiError> {
                    Ok(json!({"number": n, "person": summary(st, &id)?}))
                })
                .collect::<Result<_, _>>()?;
            Ok(json!(items))
        }
        "relationship.calc" => relationship_json(sess.store()?, &args),
        // ---- quality ----
        "quality.check" => {
            let st = sess.store()?;
            let f = quality::check_all(st, &Rules::default())?;
            let facts = Facts::load(st)?;
            let v: Vec<Value> = f
                .into_iter()
                .map(|x| {
                    let person = match x.entity_type.as_str() {
                        "person" => Some(x.entity_id.clone()),
                        "event" => facts
                            .events
                            .iter()
                            .find(|e| e.id == x.entity_id)
                            .filter(|e| e.owner_type == "person")
                            .map(|e| e.owner_id.clone()),
                        _ => None,
                    };
                    let mut j = serde_json::to_value(&x).unwrap();
                    j["person_id"] = person.map(Value::from).unwrap_or(Value::Null);
                    j
                })
                .collect();
            Ok(json!(v))
        }
        "quality.ignore" => {
            quality::ignore(sess.store_mut()?, &req(&args, "id")?)?;
            Ok(json!({"ok": true}))
        }
        "quality.fix" => {
            let fix: quality::Fix = serde_json::from_value(args["fix"].clone())?;
            sess.store_mut()?
                .transact("Apply fix", |tx| quality::apply_fix(tx, &fix))?;
            Ok(json!({"ok": true}))
        }
        "duplicates.find" => {
            let st = sess.store()?;
            let c = duplicates::find_duplicates(
                st,
                args.get("threshold")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.75),
                u(&args, "limit", 200) as usize,
            )?;
            let v: Vec<Value> = c.iter().map(|x| -> Result<Value, ApiError> { Ok(json!({"a": summary(st, &x.a)?, "b": summary(st, &x.b)?, "score": x.score, "reasons": x.reasons})) }).collect::<Result<_, _>>()?;
            Ok(json!(v))
        }
        "duplicates.merge" => {
            let (k, r) = (req(&args, "keep")?, req(&args, "remove")?);
            sess.store_mut()?
                .transact("Merge persons", |tx| duplicates::merge_persons(tx, &k, &r))?;
            Ok(json!({"ok": true}))
        }
        "duplicates.not_duplicate" => {
            duplicates::mark_not_duplicate(
                sess.store_mut()?,
                &req(&args, "a")?,
                &req(&args, "b")?,
            )?;
            Ok(json!({"ok": true}))
        }
        "stats.compute" => Ok(serde_json::to_value(stats::compute(sess.store()?)?)?),
        "stats.persons" => {
            // resolve a list of ids to summaries (drill-down)
            let st = sess.store()?;
            let ids: Vec<String> = args["ids"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let items: Vec<Value> = ids
                .iter()
                .take(500)
                .map(|i| summary(st, i))
                .collect::<Result<_, _>>()?;
            Ok(json!(items))
        }
        // ---- places ----
        "place.list" => {
            let st = sess.store()?;
            let names = places::full_names(st)?;
            let mut usage: HashMap<String, i64> = HashMap::new();
            for e in st.rows("event")? {
                if let Some(p) = e["place_id"].as_str() {
                    *usage.entry(p.to_string()).or_default() += 1;
                }
            }
            let mut v: Vec<Value> = st.rows("place")?.into_iter().map(|p| json!({"id": p["id"], "name": p["name"], "full_name": names.get(&rs(&p, "id")), "lat": p["lat"], "lon": p["lon"], "uses": usage.get(&rs(&p, "id")).copied().unwrap_or(0)})).collect();
            v.sort_by(|a, b| a["full_name"].as_str().cmp(&b["full_name"].as_str()));
            Ok(json!(v))
        }
        "place.merge" => {
            let (k, r) = (req(&args, "keep")?, req(&args, "remove")?);
            sess.store_mut()?
                .transact("Merge places", |tx| places::merge(tx, &k, &r))?;
            Ok(json!({"ok": true}))
        }
        "place.duplicates" => Ok(json!(places::find_duplicate_places(sess.store()?)?)),
        // ---- generic records ----
        "rec.list" => {
            let t = rec_table(&args)?;
            Ok(json!(sess.store()?.rows(&t)?))
        }
        "rec.get" => {
            let t = rec_table(&args)?;
            let id = req(&args, "id")?;
            sess.store()?
                .rows_where(&t, "id", &id)?
                .into_iter()
                .next()
                .map(Value::Object)
                .ok_or_else(|| ApiError::new("not_found", id))
        }
        "rec.put" => {
            let t = rec_table(&args)?;
            let mut r = args["row"]
                .as_object()
                .cloned()
                .ok_or_else(|| ApiError::new("bad_args", "missing `row`"))?;
            let id = r
                .get("id")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(new_id);
            r.insert("id".into(), id.clone().into());
            sess.store_mut()?.transact(&format!("Edit {}", t), |tx| {
                tx.put_row(&t, r)?;
                Ok(())
            })?;
            Ok(json!({"id": id}))
        }
        "rec.delete" => {
            let t = rec_table(&args)?;
            let id = req(&args, "id")?;
            sess.store_mut()?.transact(&format!("Delete {}", t), |tx| {
                for lt in [
                    "citation",
                    "note_link",
                    "media_link",
                    "tag_link",
                    "task_link",
                ] {
                    if lt != t {
                        for r in tx.ids_where(lt, "target_id", &id)? {
                            tx.delete(lt, &r)?;
                        }
                    }
                }
                tx.delete(&t, &id)?;
                Ok(())
            })?;
            Ok(json!({"ok": true}))
        }
        "meta.event_types" => Ok(json!({
            "person": ["BIRT","CHR","BAPM","BARM","BASM","BLES","CHRA","CONF","FCOM","ORDN","ADOP","DEAT","BURI","CREM","PROB","WILL","NATU","EMIG","IMMI","CENS","GRAD","RETI","RESI","OCCU","EDUC","NATI","RELI","TITL","PROP","CAST","DSCR","IDNO","SSN","NCHI","FACT","EVEN"],
            "family": ["MARR","ENGA","MARB","MARC","MARL","MARS","DIV","DIVF","ANUL","CENS","RESI","EVEN"],
            "attributes": ["OCCU","EDUC","NATI","RELI","TITL","PROP","CAST","DSCR","IDNO","SSN","NCHI","FACT","RESI"]
        })),
        _ => Err(ApiError::new("unknown_command", cmd)),
    }
}
