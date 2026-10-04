//! CSV (persons) and JSON export, and CSV person import.

use crate::date::GenDate;
use crate::facts::Facts;
use crate::model::EventRec;
use crate::name::PersonName;
use crate::places;
use crate::store::{new_id, Result, Store, Tx};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

const COLUMNS: &[&str] = &[
    "id",
    "given",
    "surname",
    "sex",
    "birth_date",
    "birth_place",
    "death_date",
    "death_place",
    "occupation",
    "father_id",
    "mother_id",
    "partner_ids",
];

fn q(s: &str) -> String {
    // quote when needed; neutralise spreadsheet formula injection
    let mut v = s.to_string();
    if v.starts_with(['=', '+', '-', '@'])
        && !v.chars().skip(1).all(|c| c.is_ascii_digit() || c == '.')
    {
        v.insert(0, '\'');
    }
    if v.contains([',', '"', '\n', '\r', ';']) || v.starts_with(' ') || v.ends_with(' ') {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v
    }
}

/// Persons as RFC 4180 CSV (UTF-8, CRLF). Dates are written in GEDCOM form so they re-import losslessly.
pub fn persons_csv(store: &Store) -> Result<String> {
    let f = Facts::load(store)?;
    let mut out = COLUMNS.join(",") + "\r\n";
    for id in &f.order {
        let p = &f.persons[id];
        let ev = |kinds: &[&str]| -> (String, String) {
            for k in kinds {
                if let Some(e) = f.person_events(id).find(|e| e.kind == *k) {
                    return (
                        e.date.as_ref().map(|d| d.to_gedcom()).unwrap_or_default(),
                        f.place_name(e).cloned().unwrap_or_default(),
                    );
                }
            }
            (String::new(), String::new())
        };
        let (bd, bp) = ev(&["BIRT", "CHR"]);
        let (dd, dp) = ev(&["DEAT", "BURI"]);
        let occ = f
            .person_events(id)
            .find(|e| e.kind == "OCCU")
            .and_then(|e| e.value.clone())
            .unwrap_or_default();
        let ps = &f.graph.people[id];
        let parent = |sex: crate::relationship::Sex| {
            ps.parents
                .iter()
                .find(|(p, _, _)| f.graph.sex(p) == sex)
                .map(|x| x.0.clone())
                .unwrap_or_default()
        };
        let mut partners: Vec<String> = ps.spouses.clone();
        partners.dedup();
        let sex = match p.sex {
            crate::relationship::Sex::Male => "M",
            crate::relationship::Sex::Female => "F",
            _ => "",
        };
        let row = [
            id.clone(),
            p.given.clone(),
            p.surname.clone(),
            sex.to_string(),
            bd,
            bp,
            dd,
            dp,
            occ,
            parent(crate::relationship::Sex::Male),
            parent(crate::relationship::Sex::Female),
            partners.join(" "),
        ];
        out.push_str(&row.iter().map(|c| q(c)).collect::<Vec<_>>().join(","));
        out.push_str("\r\n");
    }
    Ok(out)
}

/// RFC 4180 parser. Accepts `,` or `;` delimiters (spreadsheets in many locales write `;`), quoted fields with
/// embedded delimiters/newlines, CRLF or LF, and a UTF-8 BOM.
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{FEFF}');
    let first_line = text.lines().next().unwrap_or("");
    let delim = if first_line.matches(';').count() > first_line.matches(',').count() {
        ';'
    } else {
        ','
    };
    let mut rows: Vec<Vec<String>> = vec![];
    let mut row: Vec<String> = vec![];
    let mut cell = String::new();
    let mut in_q = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_q {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cell.push('"');
                    chars.next();
                } else {
                    in_q = false;
                }
            } else {
                cell.push(c);
            }
        } else if c == '"' && cell.is_empty() {
            in_q = true;
        } else if c == delim {
            row.push(std::mem::take(&mut cell));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut cell));
            if !(row.len() == 1 && row[0].is_empty()) {
                rows.push(std::mem::take(&mut row));
            } else {
                row.clear();
            }
        } else {
            cell.push(c);
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

#[derive(Debug, Default, PartialEq)]
pub struct CsvReport {
    pub persons: usize,
    pub families: usize,
    pub warnings: Vec<String>,
}

fn header_key(h: &str) -> String {
    let k = crate::name::fold(h).replace([' ', '-'], "_");
    match k.as_str() {
        "first_name" | "firstname" | "given_name" | "ad" | "isim" => "given",
        "last_name" | "lastname" | "family_name" | "soyad" | "soyadi" => "surname",
        "gender" | "cinsiyet" => "sex",
        "birth" | "born" | "dogum" | "dogum_tarihi" | "birth_date" | "date_of_birth" => {
            "birth_date"
        }
        "birthplace" | "dogum_yeri" | "place_of_birth" | "birth_place" => "birth_place",
        "death" | "died" | "olum" | "olum_tarihi" | "death_date" | "date_of_death" => "death_date",
        "deathplace" | "olum_yeri" | "place_of_death" | "death_place" => "death_place",
        "job" | "meslek" | "occupation" | "profession" => "occupation",
        other => other,
    }
    .to_string()
}

/// Import persons from CSV. Recognised headers (case/diacritics-insensitive, EN/TR aliases): given, surname, sex,
/// birth_date, birth_place, death_date, death_place, occupation, and optionally id + father_id/mother_id/partner_ids
/// to rebuild families. Rows are never rejected; problems are reported as warnings.
pub fn import_persons_csv(tx: &mut Tx, text: &str) -> Result<CsvReport> {
    let rows = parse_csv(text);
    let mut rep = CsvReport::default();
    let Some(head) = rows.first() else {
        return Ok(rep);
    };
    let keys: Vec<String> = head.iter().map(|h| header_key(h)).collect();
    if !keys.iter().any(|k| k == "given" || k == "surname") {
        rep.warnings
            .push("no given/surname column found (expected a header row)".into());
        return Ok(rep);
    }
    let mut ids: HashMap<String, String> = HashMap::new();
    let mut links: Vec<(String, String, String, String)> = vec![]; // (person, father, mother, partners)
    for (n, r) in rows.iter().enumerate().skip(1) {
        let get = |k: &str| -> String {
            keys.iter()
                .position(|x| x == k)
                .and_then(|i| r.get(i))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };
        let (given, surname) = (get("given"), get("surname"));
        if given.is_empty() && surname.is_empty() {
            rep.warnings
                .push(format!("row {}: no name, skipped", n + 1));
            continue;
        }
        let sex = match get("sex").to_ascii_uppercase().as_str() {
            "M" | "MALE" | "E" | "ERKEK" => "M",
            "F" | "FEMALE" | "K" | "KADIN" | "KADİN" => "F",
            "" => "",
            _ => "U",
        };
        let pid = tx.create_person(&PersonName::new(&given, &surname), sex)?;
        rep.persons += 1;
        let src_id = get("id");
        if !src_id.is_empty() {
            ids.insert(src_id, pid.clone());
        }
        for (kind, dk, pk) in [
            ("BIRT", "birth_date", "birth_place"),
            ("DEAT", "death_date", "death_place"),
        ] {
            let (d, p) = (get(dk), get(pk));
            if d.is_empty() && p.is_empty() {
                continue;
            }
            let date = GenDate::parse_lenient(&d);
            if date.as_ref().map(|x| x.verbatim).unwrap_or(false) {
                rep.warnings
                    .push(format!("row {}: date {:?} kept as text", n + 1, d));
            }
            let place = if p.is_empty() {
                None
            } else {
                places::find_or_create(tx, &p)?
            };
            let mut e = EventRec {
                id: new_id(),
                owner_type: "person".into(),
                owner_id: pid.clone(),
                kind: kind.into(),
                place_id: place,
                ..Default::default()
            };
            e.set_date(date.as_ref());
            tx.put("event", &e)?;
        }
        let occ = get("occupation");
        if !occ.is_empty() {
            let e = EventRec {
                id: new_id(),
                owner_type: "person".into(),
                owner_id: pid.clone(),
                kind: "OCCU".into(),
                value: Some(occ),
                ..Default::default()
            };
            tx.put("event", &e)?;
        }
        links.push((pid, get("father_id"), get("mother_id"), get("partner_ids")));
    }
    // Families: the parents of a child form (or share) one family. Pair keys ignore order so the same couple is
    // never created twice, whichever way round it is listed. Partners without shared children get their own family.
    fn norm(a: &str, b: &str) -> (String, String) {
        if a <= b {
            (a.to_string(), b.to_string())
        } else {
            (b.to_string(), a.to_string())
        }
    }
    let mut fam_of: HashMap<(String, String), String> = HashMap::new();
    for (pid, f, m, _) in &links {
        for (id, label) in [(f, "father"), (m, "mother")] {
            if !id.is_empty() && !ids.contains_key(id) {
                rep.warnings
                    .push(format!("{} id {:?} not found", label, id));
            }
        }
        let (fp, mp) = (ids.get(f), ids.get(m));
        if fp.is_some() || mp.is_some() {
            let key = norm(
                fp.map(String::as_str).unwrap_or(""),
                mp.map(String::as_str).unwrap_or(""),
            );
            let fam = match fam_of.get(&key) {
                Some(x) => x.clone(),
                None => {
                    let x = tx.create_family(
                        fp.map(String::as_str),
                        mp.map(String::as_str),
                        "married",
                    )?;
                    rep.families += 1;
                    fam_of.insert(key, x.clone());
                    x
                }
            };
            tx.add_child(&fam, pid, "")?;
        }
    }
    for (pid, _, _, partners) in &links {
        for sp in partners.split_whitespace() {
            if let Some(other) = ids.get(sp) {
                let key = norm(pid, other);
                if let std::collections::hash_map::Entry::Vacant(slot) = fam_of.entry(key.clone()) {
                    let x = tx.create_family(Some(&key.0), Some(&key.1), "married")?;
                    rep.families += 1;
                    slot.insert(x);
                }
            }
        }
    }
    Ok(rep)
}

/// Whole project as JSON (every table except internal history and the search index).
pub fn project_json(store: &Store) -> Result<Value> {
    let mut m = Map::new();
    for t in crate::store::TABLES {
        let rows: Vec<Value> = store
            .rows(t)?
            .into_iter()
            .map(|mut r| {
                r.remove("_rowid");
                Value::Object(r)
            })
            .collect();
        m.insert(t.to_string(), Value::Array(rows));
    }
    Ok(json!({"format": "kintree-json", "version": 1, "tables": m}))
}
