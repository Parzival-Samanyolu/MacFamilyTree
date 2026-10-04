//! Timelines, life-span charts, month calendar and iCalendar export.

use crate::date::{days_in_month, jdn_to_gregorian, to_jdn, Calendar, Locale};
use crate::facts::Facts;
use crate::model::living_ids;
use crate::store::{Result, Store};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    /// Julian Day Number used for ordering.
    pub sort: i64,
    pub date_text: String,
    pub kind: String,
    pub person_id: Option<String>,
    pub family_id: Option<String>,
    pub text: String,
    /// True for entries from the historical-events overlay.
    pub history: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Lifespan {
    pub person_id: String,
    pub name: String,
    pub start_year: i32,
    pub end_year: i32,
    pub estimated_end: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryEvent {
    pub year: i32,
    pub text: String,
}

pub enum Scope<'a> {
    Person(&'a str),
    /// A person's family: partners, children and parents.
    Family(&'a str),
    Surname(&'a str),
    All,
}

/// A small built-in overlay; users replace it by storing their own list under the `history_events` setting.
pub fn default_history() -> Vec<HistoryEvent> {
    [
        (1453, "Fall of Constantinople"),
        (1517, "Start of the Reformation"),
        (1789, "French Revolution begins"),
        (1815, "Battle of Waterloo"),
        (1861, "American Civil War begins"),
        (1876, "First Ottoman constitution"),
        (1914, "First World War begins"),
        (1918, "First World War ends"),
        (1923, "Republic of Türkiye proclaimed"),
        (1929, "Great Depression begins"),
        (1939, "Second World War begins"),
        (1945, "Second World War ends"),
        (1969, "First Moon landing"),
        (1989, "Fall of the Berlin Wall"),
        (2001, "Start of the 21st century"),
    ]
    .iter()
    .map(|(y, t)| HistoryEvent {
        year: *y,
        text: t.to_string(),
    })
    .collect()
}

pub fn history_events(store: &Store) -> Result<Vec<HistoryEvent>> {
    let json: Option<String> = store
        .conn()
        .query_row(
            "SELECT value FROM setting WHERE id = 'history_events'",
            [],
            |r| r.get(0),
        )
        .ok();
    Ok(json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_else(default_history))
}

fn label(kind: &str) -> &'static str {
    match kind {
        "BIRT" => "Birth",
        "CHR" => "Christening",
        "BAPM" => "Baptism",
        "DEAT" => "Death",
        "BURI" => "Burial",
        "CREM" => "Cremation",
        "MARR" => "Marriage",
        "DIV" => "Divorce",
        "ENGA" => "Engagement",
        "RESI" => "Residence",
        "OCCU" => "Occupation",
        "EDUC" => "Education",
        "GRAD" => "Graduation",
        "EMIG" => "Emigration",
        "IMMI" => "Immigration",
        "NATU" => "Naturalization",
        "CENS" => "Census",
        "RETI" => "Retirement",
        "ADOP" => "Adoption",
        "PROB" => "Probate",
        "WILL" => "Will",
        _ => "Event",
    }
}

pub fn timeline(store: &Store, scope: Scope, overlay: bool, loc: Locale) -> Result<Vec<Entry>> {
    let f = Facts::load(store)?;
    let mut people: Vec<String> = vec![];
    let mut families: Vec<String> = vec![];
    match scope {
        Scope::All => {
            people = f.order.clone();
            families = f.families.iter().map(|x| x.id.clone()).collect();
        }
        Scope::Person(id) => {
            people.push(id.to_string());
            families.extend(
                f.families
                    .iter()
                    .filter(|x| x.partners.iter().any(|p| p == id))
                    .map(|x| x.id.clone()),
            );
        }
        Scope::Family(fid) => {
            if let Some(fam) = f.families.iter().find(|x| x.id == fid) {
                people.extend(fam.partners.clone());
                people.extend(fam.children.clone());
                families.push(fid.to_string());
            }
        }
        Scope::Surname(sn) => {
            let key = crate::name::fold(sn);
            people = f
                .order
                .iter()
                .filter(|id| crate::name::fold(&f.persons[*id].surname) == key)
                .cloned()
                .collect();
            families = f
                .families
                .iter()
                .filter(|x| x.partners.iter().any(|p| people.contains(p)))
                .map(|x| x.id.clone())
                .collect();
        }
    }
    let mut out = vec![];
    for e in &f.events {
        let (Some(sort), true) = (
            e.start,
            !matches!(
                e.kind.as_str(),
                "NCHI" | "FACT" | "DSCR" | "IDNO" | "SSN" | "CAST"
            ),
        ) else {
            continue;
        };
        let (who, fam) = match e.owner_type.as_str() {
            "person" if people.contains(&e.owner_id) => (Some(e.owner_id.clone()), None),
            "family" if families.contains(&e.owner_id) => (None, Some(e.owner_id.clone())),
            _ => continue,
        };
        let name = match (&who, &fam) {
            (Some(p), _) => f.display_name(p),
            (_, Some(fid)) => f
                .families
                .iter()
                .find(|x| &x.id == fid)
                .map(|x| {
                    x.partners
                        .iter()
                        .map(|p| f.display_name(p))
                        .collect::<Vec<_>>()
                        .join(" & ")
                })
                .unwrap_or_default(),
            _ => String::new(),
        };
        let mut text = format!("{}: {}", label(&e.kind), name);
        if let Some(v) = e.value.as_ref().filter(|v| *v != "Y") {
            text.push_str(&format!(" – {}", v));
        }
        if let Some(p) = f.place_name(e) {
            text.push_str(&format!(" ({})", p));
        }
        out.push(Entry {
            sort,
            date_text: e.date.as_ref().map(|d| d.format(loc)).unwrap_or_default(),
            kind: e.kind.clone(),
            person_id: who,
            family_id: fam,
            text,
            history: false,
        });
    }
    out.sort_by(|a, b| a.sort.cmp(&b.sort).then(a.text.cmp(&b.text)));
    if overlay && !out.is_empty() {
        let (lo, hi) = (out.first().unwrap().sort, out.last().unwrap().sort);
        for h in history_events(store)? {
            let j = to_jdn(Calendar::Gregorian, h.year, 1, 1);
            if j >= lo - 365 && j <= hi + 365 {
                out.push(Entry {
                    sort: j,
                    date_text: h.year.to_string(),
                    kind: "HISTORY".into(),
                    person_id: None,
                    family_id: None,
                    text: h.text,
                    history: true,
                });
            }
        }
        out.sort_by(|a, b| {
            a.sort
                .cmp(&b.sort)
                .then(a.history.cmp(&b.history))
                .then(a.text.cmp(&b.text))
        });
    }
    Ok(out)
}

/// Life spans for a chart; people without a birth or death year are skipped. Living people run to `current_year`.
pub fn lifespans(
    store: &Store,
    surname: Option<&str>,
    current_year: i32,
    living_years: i32,
) -> Result<Vec<Lifespan>> {
    let f = Facts::load(store)?;
    let living = living_ids(store, living_years, current_year)?;
    let key = surname.map(crate::name::fold);
    let mut out = vec![];
    for id in &f.order {
        let p = &f.persons[id];
        if let Some(k) = &key {
            if crate::name::fold(&p.surname) != *k {
                continue;
            }
        }
        let Some(b) = f
            .birth(id)
            .and_then(|e| e.start)
            .map(|j| jdn_to_gregorian(j).0)
        else {
            continue;
        };
        let (end, est) = match f
            .death(id)
            .and_then(|e| e.start)
            .map(|j| jdn_to_gregorian(j).0)
        {
            Some(d) => (d, false),
            None if living.contains(id) => (current_year, true),
            None => (b + 70, true),
        };
        out.push(Lifespan {
            person_id: id.clone(),
            name: f.display_name(id),
            start_year: b,
            end_year: end.max(b),
            estimated_end: est,
        });
    }
    out.sort_by_key(|l| (l.start_year, l.name.clone()));
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DayEntry {
    pub day: u8,
    pub kind: String,
    pub person_id: Option<String>,
    pub text: String,
    pub years: i32,
}

/// Birthdays, wedding anniversaries and death anniversaries falling in a month.
pub fn calendar_month(
    store: &Store,
    year: i32,
    month: u8,
    include_deceased_birthdays: bool,
    current_year: i32,
    living_years: i32,
) -> Result<Vec<DayEntry>> {
    let f = Facts::load(store)?;
    let living = living_ids(store, living_years, current_year)?;
    let mut out = vec![];
    let dim = days_in_month(Calendar::Gregorian, year, month);
    for e in &f.events {
        let (Some(s), Some(en)) = (e.start, e.end) else {
            continue;
        };
        if s != en
            || e.date
                .as_ref()
                .map(|d| d.calendar != Calendar::Gregorian)
                .unwrap_or(true)
        {
            continue;
        }
        let (y, m, d) = jdn_to_gregorian(s);
        let day = if m == 2 && d == 29 && month == 2 {
            dim.min(29)
        } else {
            d
        };
        if m != month || day > dim || y > year {
            continue;
        }
        let years = year - y;
        match (e.kind.as_str(), e.owner_type.as_str()) {
            ("BIRT", "person") => {
                if living.contains(&e.owner_id) || include_deceased_birthdays {
                    out.push(DayEntry {
                        day,
                        kind: "BIRT".into(),
                        person_id: Some(e.owner_id.clone()),
                        text: f.display_name(&e.owner_id),
                        years,
                    });
                }
            }
            ("DEAT", "person") => out.push(DayEntry {
                day,
                kind: "DEAT".into(),
                person_id: Some(e.owner_id.clone()),
                text: f.display_name(&e.owner_id),
                years,
            }),
            ("MARR", "family") => {
                if let Some(fam) = f.families.iter().find(|x| x.id == e.owner_id) {
                    let both_alive = fam.partners.iter().all(|p| !f.has_death_event(p));
                    if both_alive || include_deceased_birthdays {
                        out.push(DayEntry {
                            day,
                            kind: "MARR".into(),
                            person_id: fam.partners.first().cloned(),
                            text: fam
                                .partners
                                .iter()
                                .map(|p| f.display_name(p))
                                .collect::<Vec<_>>()
                                .join(" & "),
                            years,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    out.sort_by(|a, b| a.day.cmp(&b.day).then(a.text.cmp(&b.text)));
    Ok(out)
}

fn ics_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

/// RFC 5545 line folding at 75 octets, never splitting a UTF-8 character.
fn fold_line(line: &str) -> String {
    let mut out = String::new();
    let mut len = 0;
    for ch in line.chars() {
        let w = ch.len_utf8();
        if len + w > 75 {
            out.push_str("\r\n ");
            len = 1;
        }
        out.push(ch);
        len += w;
    }
    out
}

/// Yearly-recurring birthdays and anniversaries as an iCalendar file.
pub fn ical(
    store: &Store,
    include_deceased: bool,
    current_year: i32,
    living_years: i32,
) -> Result<String> {
    let f = Facts::load(store)?;
    let living = living_ids(store, living_years, current_year)?;
    let mut lines: Vec<String> = vec![
        "BEGIN:VCALENDAR".into(),
        "VERSION:2.0".into(),
        "PRODID:-//KinTree//Family calendar//EN".into(),
        "CALSCALE:GREGORIAN".into(),
        "X-WR-CALNAME:Family birthdays and anniversaries".into(),
    ];
    let mut add = |uid: String, jdn: i64, summary: String| {
        let (y, m, d) = jdn_to_gregorian(jdn);
        if y < 1 {
            return;
        }
        lines.push("BEGIN:VEVENT".into());
        lines.push(format!("UID:{}@kintree", uid));
        lines.push("DTSTAMP:20000101T000000Z".into());
        lines.push(format!("DTSTART;VALUE=DATE:{:04}{:02}{:02}", y, m, d));
        lines.push("RRULE:FREQ=YEARLY".into());
        lines.push(format!("SUMMARY:{}", ics_escape(&summary)));
        lines.push("TRANSP:TRANSPARENT".into());
        lines.push("END:VEVENT".into());
    };
    for e in &f.events {
        let (Some(s), Some(en)) = (e.start, e.end) else {
            continue;
        };
        if s != en
            || e.date
                .as_ref()
                .map(|d| d.calendar != Calendar::Gregorian)
                .unwrap_or(true)
        {
            continue;
        }
        match (e.kind.as_str(), e.owner_type.as_str()) {
            ("BIRT", "person") if living.contains(&e.owner_id) || include_deceased => add(
                e.id.clone(),
                s,
                format!("{}'s birthday", f.display_name(&e.owner_id)),
            ),
            ("MARR", "family") => {
                if let Some(fam) = f.families.iter().find(|x| x.id == e.owner_id) {
                    if include_deceased || fam.partners.iter().all(|p| !f.has_death_event(p)) {
                        add(
                            e.id.clone(),
                            s,
                            format!(
                                "{} – wedding anniversary",
                                fam.partners
                                    .iter()
                                    .map(|p| f.display_name(p))
                                    .collect::<Vec<_>>()
                                    .join(" & ")
                            ),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    lines.push("END:VCALENDAR".into());
    Ok(lines
        .iter()
        .map(|l| fold_line(l))
        .collect::<Vec<_>>()
        .join("\r\n")
        + "\r\n")
}
