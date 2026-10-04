//! Gramps XML (`.gramps`, plain or gzip-compressed) import by translation to GEDCOM 5.5.1, which the importer already maps losslessly.
//!
//! Covered: people (names, gender, nickname), events with dates / places / descriptions, families (partners, children,
//! relationship type, family events), places (hierarchy, coordinates), sources, citations (page), notes.
//! Gramps features without a GEDCOM equivalent (tags, attributes, addresses, media paths) are not translated.

use crate::store::{Result, StoreError};
use flate2::read::GzDecoder;
use roxmltree::{Document, Node};
use std::collections::HashMap;
use std::io::Read;

fn err(m: impl std::fmt::Display) -> StoreError {
    StoreError::Other(format!("Gramps XML: {m}"))
}

pub fn looks_like_gramps(bytes: &[u8]) -> bool {
    if bytes.starts_with(&[0x1F, 0x8B]) {
        return true;
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(2000)]);
    head.contains("<database") && head.contains("gramps-project.org")
}

fn child<'a>(n: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}
fn children<'a>(n: Node<'a, 'a>, name: &'a str) -> impl Iterator<Item = Node<'a, 'a>> {
    n.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == name)
}
fn text(n: Node, name: &str) -> String {
    child(n, name)
        .and_then(|c| c.text())
        .unwrap_or("")
        .trim()
        .to_string()
}
fn hlink<'a>(n: Node<'a, 'a>, name: &str) -> Option<String> {
    child(n, name)
        .and_then(|c| c.attribute("hlink"))
        .map(|s| s.trim_start_matches('_').to_string())
}
fn clean(s: &str) -> String {
    s.replace(['\r', '\n'], " ").trim().to_string()
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// Gramps `1850-03-04` (parts may be 00) to GEDCOM `4 MAR 1850`.
fn date_val(v: &str) -> String {
    let neg = v.starts_with('-');
    let parts: Vec<&str> = v.trim_start_matches('-').split('-').collect();
    let y: i32 = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
    let m: usize = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
    let d: u32 = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
    let mut out = String::new();
    if d > 0 && m > 0 {
        out.push_str(&format!("{d} "));
    }
    if m > 0 && m <= 12 {
        out.push_str(MONTHS[m - 1]);
        out.push(' ');
    }
    if y > 0 {
        out.push_str(&y.to_string());
        if neg {
            out.push_str(" BC");
        }
    }
    out.trim().to_string()
}

fn date_text(ev: Node) -> Option<String> {
    if let Some(d) = child(ev, "dateval") {
        let v = date_val(d.attribute("val")?);
        let q = match d.attribute("type") {
            Some("about") => "ABT ",
            Some("before") => "BEF ",
            Some("after") => "AFT ",
            _ => "",
        };
        let q = if d.attribute("quality") == Some("estimated") && q.is_empty() {
            "EST "
        } else if d.attribute("quality") == Some("calculated") && q.is_empty() {
            "CAL "
        } else {
            q
        };
        return Some(format!("{q}{v}"));
    }
    if let Some(d) = child(ev, "daterange") {
        return Some(format!(
            "BET {} AND {}",
            date_val(d.attribute("start")?),
            date_val(d.attribute("stop")?)
        ));
    }
    if let Some(d) = child(ev, "datespan") {
        return Some(format!(
            "FROM {} TO {}",
            date_val(d.attribute("start")?),
            date_val(d.attribute("stop")?)
        ));
    }
    child(ev, "datestr")
        .and_then(|d| d.attribute("val"))
        .map(|v| format!("({v})"))
}

fn event_tag(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "Birth" => "BIRT",
        "Death" => "DEAT",
        "Marriage" => "MARR",
        "Divorce" => "DIV",
        "Burial" => "BURI",
        "Cremation" => "CREM",
        "Baptism" => "BAPM",
        "Christening" => "CHR",
        "Residence" => "RESI",
        "Occupation" => "OCCU",
        "Immigration" => "IMMI",
        "Emigration" => "EMIG",
        "Census" => "CENS",
        "Education" => "EDUC",
        "Engagement" => "ENGA",
        "Graduation" => "GRAD",
        "Retirement" => "RETI",
        "Probate" => "PROB",
        "Will" => "WILL",
        "Naturalization" => "NATU",
        "Confirmation" => "CONF",
        "Adopted" => "ADOP",
        _ => return None,
    })
}

/// name, parent handle, (lat, lon) as written in the file
type PlaceInfo = (String, Option<String>, Option<(String, String)>);

struct Ctx {
    places: HashMap<String, PlaceInfo>,
    notes: HashMap<String, String>,
    citations: HashMap<String, (String, String)>,
}

impl Ctx {
    fn place_name(&self, id: &str) -> String {
        let mut parts = vec![];
        let mut cur = Some(id.to_string());
        let mut guard = 0;
        while let Some(c) = cur {
            let Some((name, parent, _)) = self.places.get(&c) else {
                break;
            };
            if !name.is_empty() {
                parts.push(name.clone());
            }
            cur = parent.clone();
            guard += 1;
            if guard > 32 {
                break;
            }
        }
        parts.join(", ")
    }
}

fn emit_event(out: &mut String, ev: Node, ctx: &Ctx, level: u8, tag: &str, custom: Option<&str>) {
    out.push_str(&format!("{level} {tag}\n"));
    let l = level + 1;
    if let Some(c) = custom {
        out.push_str(&format!("{l} TYPE {}\n", clean(c)));
    }
    if let Some(d) = date_text(ev) {
        out.push_str(&format!("{l} DATE {d}\n"));
    }
    if let Some(p) = hlink(ev, "place") {
        let name = ctx.place_name(&p);
        if !name.is_empty() {
            out.push_str(&format!("{l} PLAC {name}\n"));
            if let Some((_, _, Some((la, lo)))) = ctx.places.get(&p) {
                let fmt = |v: &str, pos: &str, neg: &str| {
                    let f: f64 = v.parse().unwrap_or(0.0);
                    format!("{}{}", if f < 0.0 { neg } else { pos }, f.abs())
                };
                out.push_str(&format!(
                    "{} MAP\n{} LATI {}\n{} LONG {}\n",
                    l + 1,
                    l + 2,
                    fmt(la, "N", "S"),
                    l + 2,
                    fmt(lo, "E", "W")
                ));
            }
        }
    }
    let d = text(ev, "description");
    if !d.is_empty() {
        out.push_str(&format!("{l} NOTE {}\n", clean(&d)));
    }
    for c in children(ev, "citationref") {
        if let Some((src, page)) = c
            .attribute("hlink")
            .and_then(|h| ctx.citations.get(h.trim_start_matches('_')))
        {
            out.push_str(&format!("{l} SOUR @S{src}@\n"));
            if !page.is_empty() {
                out.push_str(&format!("{} PAGE {page}\n", l + 1));
            }
        }
    }
}

pub fn to_gedcom(bytes: &[u8]) -> Result<String> {
    let mut xml = String::new();
    if bytes.starts_with(&[0x1F, 0x8B]) {
        GzDecoder::new(bytes)
            .read_to_string(&mut xml)
            .map_err(|e| err(format!("cannot decompress: {e}")))?;
    } else {
        xml = String::from_utf8_lossy(bytes).into_owned();
    }
    let doc = Document::parse_with_options(
        &xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .map_err(err)?;
    let root = doc.root_element();
    if root.tag_name().name() != "database" {
        return Err(err("this is not a Gramps database"));
    }
    let section = |n: &'static str| {
        child(root, n)
            .into_iter()
            .flat_map(|s| s.children().filter(|c| c.is_element()))
    };

    let mut ctx = Ctx {
        places: HashMap::new(),
        notes: HashMap::new(),
        citations: HashMap::new(),
    };
    for p in section("places") {
        let h = p
            .attribute("handle")
            .unwrap_or("")
            .trim_start_matches('_')
            .to_string();
        let name = child(p, "pname")
            .and_then(|n| n.attribute("value"))
            .map(clean)
            .or_else(|| Some(text(p, "ptitle")))
            .unwrap_or_default();
        let coord = child(p, "coord").and_then(|c| {
            Some((
                c.attribute("lat")?.to_string(),
                c.attribute("long")
                    .or_else(|| c.attribute("lon"))?
                    .to_string(),
            ))
        });
        ctx.places.insert(h, (name, hlink(p, "placeref"), coord));
    }
    for n in section("notes") {
        let h = n
            .attribute("handle")
            .unwrap_or("")
            .trim_start_matches('_')
            .to_string();
        ctx.notes.insert(h, text(n, "text"));
    }
    let mut sources = String::new();
    for s in section("sources") {
        let h = s.attribute("handle").unwrap_or("").trim_start_matches('_');
        sources.push_str(&format!(
            "0 @S{h}@ SOUR\n1 TITL {}\n",
            clean(&text(s, "stitle"))
        ));
        for (tag, el) in [("AUTH", "sauthor"), ("PUBL", "spubinfo")] {
            let v = text(s, el);
            if !v.is_empty() {
                sources.push_str(&format!("1 {tag} {}\n", clean(&v)));
            }
        }
    }
    for c in section("citations") {
        let h = c
            .attribute("handle")
            .unwrap_or("")
            .trim_start_matches('_')
            .to_string();
        ctx.citations.insert(
            h,
            (
                hlink(c, "sourceref").unwrap_or_default(),
                clean(&text(c, "page")),
            ),
        );
    }
    let events: HashMap<String, Node> = section("events")
        .map(|e| {
            (
                e.attribute("handle")
                    .unwrap_or("")
                    .trim_start_matches('_')
                    .to_string(),
                e,
            )
        })
        .collect();

    // map handles to xrefs
    let mut person_x: HashMap<String, String> = HashMap::new();
    for (i, p) in section("people").enumerate() {
        person_x.insert(
            p.attribute("handle")
                .unwrap_or("")
                .trim_start_matches('_')
                .to_string(),
            format!("@I{}@", i + 1),
        );
    }
    let mut fam_x: HashMap<String, String> = HashMap::new();
    for (i, f) in section("families").enumerate() {
        fam_x.insert(
            f.attribute("handle")
                .unwrap_or("")
                .trim_start_matches('_')
                .to_string(),
            format!("@F{}@", i + 1),
        );
    }

    let mut out = String::from(
        "0 HEAD\n1 SOUR Gramps\n1 GEDC\n2 VERS 5.5.1\n2 FORM LINEAGE-LINKED\n1 CHAR UTF-8\n",
    );
    for p in section("people") {
        let h = p.attribute("handle").unwrap_or("").trim_start_matches('_');
        out.push_str(&format!("0 {} INDI\n", person_x[h]));
        let mut first = true;
        for n in children(p, "name").chain(children(p, "alt-name")) {
            let given = clean(&text(n, "first"));
            let surname = children(n, "surname")
                .filter_map(|s| s.text())
                .map(|s| s.trim().to_string())
                .collect::<Vec<_>>()
                .join(" ");
            out.push_str(&format!("1 NAME {} /{}/\n", given, surname));
            if !given.is_empty() {
                out.push_str(&format!("2 GIVN {given}\n"));
            }
            if !surname.is_empty() {
                out.push_str(&format!("2 SURN {surname}\n"));
            }
            let nick = text(n, "nick");
            if !nick.is_empty() {
                out.push_str(&format!("2 NICK {}\n", clean(&nick)));
            }
            if !first {
                out.push_str("2 TYPE aka\n");
            }
            first = false;
        }
        match text(p, "gender").as_str() {
            "M" => out.push_str("1 SEX M\n"),
            "F" => out.push_str("1 SEX F\n"),
            _ => {}
        }
        for er in children(p, "eventref") {
            if let Some(ev) = er
                .attribute("hlink")
                .and_then(|h| events.get(h.trim_start_matches('_')))
            {
                let kind = text(*ev, "type");
                match event_tag(&kind) {
                    Some(tag) => emit_event(&mut out, *ev, &ctx, 1, tag, None),
                    None => emit_event(&mut out, *ev, &ctx, 1, "EVEN", Some(&kind)),
                }
            }
        }
        for f in children(p, "childof") {
            if let Some(x) = f
                .attribute("hlink")
                .and_then(|h| fam_x.get(h.trim_start_matches('_')))
            {
                out.push_str(&format!("1 FAMC {x}\n"));
            }
        }
        for f in children(p, "parentin") {
            if let Some(x) = f
                .attribute("hlink")
                .and_then(|h| fam_x.get(h.trim_start_matches('_')))
            {
                out.push_str(&format!("1 FAMS {x}\n"));
            }
        }
        for nr in children(p, "noteref") {
            if let Some(t) = nr
                .attribute("hlink")
                .and_then(|h| ctx.notes.get(h.trim_start_matches('_')))
            {
                let mut lines = t
                    .replace('\r', "")
                    .split('\n')
                    .map(|l| l.to_string())
                    .collect::<Vec<_>>()
                    .into_iter();
                out.push_str(&format!("1 NOTE {}\n", lines.next().unwrap_or_default()));
                for l in lines {
                    out.push_str(&format!("2 CONT {l}\n"));
                }
            }
        }
        for c in children(p, "citationref") {
            if let Some((src, page)) = c
                .attribute("hlink")
                .and_then(|h| ctx.citations.get(h.trim_start_matches('_')))
            {
                out.push_str(&format!("1 SOUR @S{src}@\n"));
                if !page.is_empty() {
                    out.push_str(&format!("2 PAGE {page}\n"));
                }
            }
        }
    }
    for f in section("families") {
        let h = f.attribute("handle").unwrap_or("").trim_start_matches('_');
        out.push_str(&format!("0 {} FAM\n", fam_x[h]));
        for (tag, el) in [("HUSB", "father"), ("WIFE", "mother")] {
            if let Some(x) = hlink(f, el).and_then(|h| person_x.get(&h).cloned()) {
                out.push_str(&format!("1 {tag} {x}\n"));
            }
        }
        for c in children(f, "childref") {
            if let Some(x) = c
                .attribute("hlink")
                .and_then(|h| person_x.get(h.trim_start_matches('_')))
            {
                out.push_str(&format!("1 CHIL {x}\n"));
            }
        }
        for er in children(f, "eventref") {
            if let Some(ev) = er
                .attribute("hlink")
                .and_then(|h| events.get(h.trim_start_matches('_')))
            {
                let kind = text(*ev, "type");
                match event_tag(&kind) {
                    Some(tag) => emit_event(&mut out, *ev, &ctx, 1, tag, None),
                    None => emit_event(&mut out, *ev, &ctx, 1, "EVEN", Some(&kind)),
                }
            }
        }
    }
    out.push_str(&sources);
    out.push_str("0 TRLR\n");
    Ok(out)
}
