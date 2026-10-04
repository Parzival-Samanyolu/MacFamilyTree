//! Geography: embedded offline gazetteer, place geocoding, map points, routes, heat map, migration arcs, GeoJSON/KML.

use crate::date::jdn_to_gregorian;
use crate::facts::Facts;
use crate::model::living_ids;
use crate::name::fold;
use crate::places::split_place;
use crate::store::{Result, Store, Tx};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

const GAZETTEER_CSV: &str = include_str!("../data/gazetteer.csv");

#[derive(Debug, Clone)]
pub struct GazEntry {
    pub kind: String,
    pub name: String,
    pub country: String,
    pub lat: f64,
    pub lon: f64,
}

pub struct Gazetteer {
    by_name: HashMap<String, Vec<GazEntry>>,
}

impl Gazetteer {
    pub fn embedded() -> Gazetteer {
        let mut by_name: HashMap<String, Vec<GazEntry>> = HashMap::new();
        for line in GAZETTEER_CSV.lines().skip(1) {
            let f: Vec<&str> = line.split(',').collect();
            if f.len() < 5 {
                continue;
            }
            let (Ok(lat), Ok(lon)) = (f[3].parse::<f64>(), f[4].parse::<f64>()) else {
                continue;
            };
            let e = GazEntry {
                kind: f[0].into(),
                name: f[1].into(),
                country: f[2].into(),
                lat,
                lon,
            };
            let mut keys = vec![fold(f[1])];
            if let Some(al) = f.get(5) {
                keys.extend(al.split(';').filter(|a| !a.is_empty()).map(fold));
            }
            for k in keys {
                by_name.entry(k).or_default().push(e.clone());
            }
        }
        Gazetteer { by_name }
    }

    pub fn len(&self) -> usize {
        self.by_name.values().map(|v| v.len()).sum()
    }
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    /// Resolve place parts (most specific first). A part matches a gazetteer entry; when other parts name a
    /// country, entries of that country win. Returns (lat, lon, matched part index).
    pub fn lookup(&self, parts: &[String]) -> Option<(f64, f64, usize)> {
        // Canonical country names named anywhere in the place ("Türkiye" and "Turkey" are one country).
        let countries: HashSet<String> = parts
            .iter()
            .filter_map(|p| self.by_name.get(&fold(p)))
            .flat_map(|v| {
                v.iter()
                    .filter(|e| e.kind == "country")
                    .map(|e| fold(&e.country))
            })
            .collect();
        for (i, p) in parts.iter().enumerate() {
            let Some(cands) = self.by_name.get(&fold(p)) else {
                continue;
            };
            let in_country: Vec<&GazEntry> = cands
                .iter()
                .filter(|e| countries.contains(&fold(&e.country)))
                .collect();
            let pick = in_country
                .first()
                .copied()
                .or(if countries.is_empty() {
                    cands.first()
                } else {
                    None
                })
                .or_else(|| cands.iter().find(|e| e.kind == "country"));
            if let Some(e) = pick {
                return Some((e.lat, e.lon, i));
            }
        }
        None
    }
}

/// Place id → (lat, lon, inherited) using the nearest ancestor with coordinates.
pub fn effective_coords(store: &Store) -> Result<HashMap<String, (f64, f64, bool)>> {
    let rows = store.rows("place")?;
    let by_id: HashMap<&str, &crate::store::Row> = rows
        .iter()
        .filter_map(|r| r["id"].as_str().map(|i| (i, r)))
        .collect();
    let mut out = HashMap::new();
    for r in &rows {
        let id = r["id"].as_str().unwrap_or("");
        let mut cur = Some(*by_id.get(id).unwrap());
        let mut inherited = false;
        let mut guard = 0;
        while let Some(c) = cur {
            if let (Some(la), Some(lo)) = (c["lat"].as_f64(), c["lon"].as_f64()) {
                out.insert(id.to_string(), (la, lo, inherited));
                break;
            }
            inherited = true;
            cur = c["parent_id"].as_str().and_then(|p| by_id.get(p).copied());
            guard += 1;
            if guard > 64 {
                break;
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Serialize, PartialEq)]
pub struct GeocodeReport {
    pub geocoded: usize,
    pub already: usize,
    pub unresolved: Vec<String>,
}

/// Fill missing coordinates from the offline gazetteer. Places keep status `offline`; existing coordinates are untouched.
pub fn geocode_offline(
    tx: &mut Tx,
    store_names: &HashMap<String, String>,
) -> Result<GeocodeReport> {
    let gaz = Gazetteer::embedded();
    let mut rep = GeocodeReport {
        geocoded: 0,
        already: 0,
        unresolved: vec![],
    };
    let mut ids = tx.all_ids("place")?;
    ids.sort();
    for id in ids {
        let Some(mut r) = tx.get("place", &id)? else {
            continue;
        };
        if r["lat"].as_f64().is_some() && r["lon"].as_f64().is_some() {
            rep.already += 1;
            continue;
        }
        let full = store_names.get(&id).cloned().unwrap_or_default();
        let parts = split_place(&full);
        match gaz.lookup(&parts) {
            // Only the place itself or its own name may receive coordinates; ancestors resolve on their own row.
            Some((la, lo, 0)) => {
                r.insert("lat".into(), json!(la));
                r.insert("lon".into(), json!(lo));
                r.insert("geocode_status".into(), json!("offline"));
                tx.put_row("place", r)?;
                rep.geocoded += 1;
            }
            _ => rep.unresolved.push(full),
        }
    }
    Ok(rep)
}

pub fn valid_coords(lat: f64, lon: f64) -> bool {
    (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub kinds: Vec<String>,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
    pub person: Option<String>,
    pub surname: Option<String>,
    /// Hide events of living persons.
    pub hide_living: bool,
    pub current_year: i32,
    pub living_years: i32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MapPoint {
    pub event_id: String,
    pub person_id: String,
    pub name: String,
    pub kind: String,
    pub year: Option<i32>,
    pub lat: f64,
    pub lon: f64,
    pub place_id: String,
    pub place: String,
    pub inherited: bool,
}

pub fn points(store: &Store, flt: &Filter) -> Result<Vec<MapPoint>> {
    let f = Facts::load(store)?;
    let coords = effective_coords(store)?;
    let living = if flt.hide_living {
        living_ids(store, flt.living_years, flt.current_year)?
    } else {
        HashSet::new()
    };
    let sn = flt.surname.as_deref().map(fold);
    let mut out = vec![];
    for e in &f.events {
        if e.owner_type != "person" {
            continue;
        }
        let Some(pid) = &e.place else { continue };
        let Some(&(lat, lon, inherited)) = coords.get(pid) else {
            continue;
        };
        if !flt.kinds.is_empty() && !flt.kinds.contains(&e.kind) {
            continue;
        }
        if let Some(p) = &flt.person {
            if &e.owner_id != p {
                continue;
            }
        }
        let Some(pf) = f.persons.get(&e.owner_id) else {
            continue;
        };
        if let Some(s) = &sn {
            if fold(&pf.surname) != *s {
                continue;
            }
        }
        if living.contains(&e.owner_id) {
            continue;
        }
        let year = e.start.map(|j| jdn_to_gregorian(j).0);
        if let Some(y) = year {
            if flt.year_from.map(|a| y < a).unwrap_or(false)
                || flt.year_to.map(|b| y > b).unwrap_or(false)
            {
                continue;
            }
        } else if flt.year_from.is_some() || flt.year_to.is_some() {
            continue;
        }
        out.push(MapPoint {
            event_id: e.id.clone(),
            person_id: e.owner_id.clone(),
            name: f.display_name(&e.owner_id),
            kind: e.kind.clone(),
            year,
            lat,
            lon,
            place_id: pid.clone(),
            place: f.places.get(pid).cloned().unwrap_or_default(),
            inherited,
        });
    }
    out.sort_by(|a, b| {
        (a.year.unwrap_or(i32::MAX), &a.event_id).cmp(&(b.year.unwrap_or(i32::MAX), &b.event_id))
    });
    Ok(out)
}

/// One person's dated, located events in chronological order (undated events keep their relative order at the end).
pub fn route(store: &Store, person: &str, flt: &Filter) -> Result<Vec<MapPoint>> {
    let mut f2 = flt.clone();
    f2.person = Some(person.to_string());
    points(store, &f2)
}

#[derive(Debug, Serialize, PartialEq)]
pub struct HeatCell {
    pub place_id: String,
    pub place: String,
    pub lat: f64,
    pub lon: f64,
    pub count: usize,
}

pub fn heat(store: &Store, flt: &Filter) -> Result<Vec<HeatCell>> {
    let mut m: HashMap<String, HeatCell> = HashMap::new();
    for p in points(store, flt)? {
        m.entry(p.place_id.clone())
            .and_modify(|c| c.count += 1)
            .or_insert(HeatCell {
                place_id: p.place_id,
                place: p.place,
                lat: p.lat,
                lon: p.lon,
                count: 1,
            });
    }
    let mut v: Vec<HeatCell> = m.into_values().collect();
    v.sort_by(|a, b| b.count.cmp(&a.count).then(a.place.cmp(&b.place)));
    Ok(v)
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Arc {
    pub person_id: String,
    pub name: String,
    pub from: [f64; 2],
    pub to: [f64; 2],
    pub from_place: String,
    pub to_place: String,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
    /// `life` = birth→death of the same person, `generation` = parent's birth → child's birth.
    pub kind: String,
}

/// Migration arcs between distinct locations, either per life or across generations.
pub fn arcs(store: &Store, flt: &Filter, generations: bool) -> Result<Vec<Arc>> {
    let mut f2 = flt.clone();
    f2.kinds = vec!["BIRT".into(), "DEAT".into()];
    f2.year_from = None;
    f2.year_to = None;
    let pts = points(store, &f2)?;
    let mut birth: HashMap<String, &MapPoint> = HashMap::new();
    let mut death: HashMap<String, &MapPoint> = HashMap::new();
    for p in &pts {
        let m = if p.kind == "BIRT" {
            &mut birth
        } else {
            &mut death
        };
        m.entry(p.person_id.clone()).or_insert(p);
    }
    let in_range = |y: Option<i32>| match y {
        Some(y) => {
            flt.year_from.map(|a| y >= a).unwrap_or(true)
                && flt.year_to.map(|b| y <= b).unwrap_or(true)
        }
        None => flt.year_from.is_none() && flt.year_to.is_none(),
    };
    let mut out = vec![];
    let mk = |a: &MapPoint, b: &MapPoint, who: &MapPoint, kind: &str| Arc {
        person_id: who.person_id.clone(),
        name: who.name.clone(),
        from: [a.lat, a.lon],
        to: [b.lat, b.lon],
        from_place: a.place.clone(),
        to_place: b.place.clone(),
        year_from: a.year,
        year_to: b.year,
        kind: kind.into(),
    };
    if !generations {
        for (id, b) in &birth {
            if let Some(d) = death.get(id) {
                if b.place_id != d.place_id && (in_range(b.year) || in_range(d.year)) {
                    out.push(mk(b, d, b, "life"));
                }
            }
        }
    } else {
        let f = Facts::load(store)?;
        for fam in &f.families {
            for parent in &fam.partners {
                let Some(pb) = birth.get(parent) else {
                    continue;
                };
                for c in &fam.children {
                    let Some(cb) = birth.get(c) else { continue };
                    if pb.place_id != cb.place_id && in_range(cb.year) {
                        out.push(mk(pb, cb, cb, "generation"));
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| {
        (a.year_to, &a.person_id, &a.from_place).cmp(&(b.year_to, &b.person_id, &b.from_place))
    });
    Ok(out)
}

pub fn to_geojson(points: &[MapPoint]) -> Value {
    json!({
        "type": "FeatureCollection",
        "features": points.iter().map(|p| json!({
            "type": "Feature",
            "geometry": {"type": "Point", "coordinates": [p.lon, p.lat]},
            "properties": {"event_id": p.event_id, "person_id": p.person_id, "name": p.name,
                           "kind": p.kind, "year": p.year, "place": p.place}
        })).collect::<Vec<_>>()
    })
}

fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn to_kml(points: &[MapPoint], title: &str) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<kml xmlns=\"http://www.opengis.net/kml/2.2\"><Document>\n",
    );
    s.push_str(&format!("<name>{}</name>\n", xml_esc(title)));
    for p in points {
        let when = p.year.map(|y| format!(" ({y})")).unwrap_or_default();
        s.push_str(&format!(
            "<Placemark><name>{}</name><description>{}{}</description><Point><coordinates>{},{},0</coordinates></Point></Placemark>\n",
            xml_esc(&p.name),
            xml_esc(&format!("{} – {}", p.kind, p.place)),
            xml_esc(&when),
            p.lon,
            p.lat
        ));
    }
    s.push_str("</Document></kml>\n");
    s
}

/// Great-circle distance in kilometres.
pub fn haversine_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let r = 6371.0088;
    let (la1, la2) = (a.0.to_radians(), b.0.to_radians());
    let dla = la2 - la1;
    let dlo = (b.1 - a.1).to_radians();
    let h = (dla / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlo / 2.0).sin().powi(2);
    2.0 * r * h.sqrt().asin()
}
