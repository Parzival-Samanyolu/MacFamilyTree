//! Media library: embedded blobs, content-hash de-duplication, thumbnails, EXIF suggestions, relinking.

use crate::date::{GenDate, Locale};
use crate::geo::{effective_coords, haversine_km};
use crate::store::{new_id, Result, Row, Store, StoreError};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Cursor;

pub const THUMB_PX: u32 = 320;

#[derive(Debug, Serialize, PartialEq)]
pub struct Imported {
    pub id: String,
    pub duplicate: bool,
    pub kind: String,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// (kind, mime) from the file extension, falling back to magic bytes.
pub fn classify(name: &str, bytes: &[u8]) -> (&'static str, &'static str) {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    let by_ext = match ext.as_str() {
        "jpg" | "jpeg" => ("image", "image/jpeg"),
        "png" => ("image", "image/png"),
        "gif" => ("image", "image/gif"),
        "webp" => ("image", "image/webp"),
        "bmp" => ("image", "image/bmp"),
        "tif" | "tiff" => ("image", "image/tiff"),
        "pdf" => ("document", "application/pdf"),
        "txt" | "md" => ("document", "text/plain"),
        "mp3" => ("audio", "audio/mpeg"),
        "wav" => ("audio", "audio/wav"),
        "ogg" | "oga" => ("audio", "audio/ogg"),
        "m4a" => ("audio", "audio/mp4"),
        "flac" => ("audio", "audio/flac"),
        "mp4" | "m4v" => ("video", "video/mp4"),
        "mov" => ("video", "video/quicktime"),
        "webm" => ("video", "video/webm"),
        _ => ("", ""),
    };
    if !by_ext.0.is_empty() {
        return by_ext;
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        ("image", "image/jpeg")
    } else if bytes.starts_with(b"\x89PNG") {
        ("image", "image/png")
    } else if bytes.starts_with(b"GIF8") {
        ("image", "image/gif")
    } else if bytes.starts_with(b"%PDF") {
        ("document", "application/pdf")
    } else {
        ("other", "application/octet-stream")
    }
}

#[derive(Debug, Default, Clone, Serialize, PartialEq)]
pub struct ExifInfo {
    /// `YYYY-MM-DD`
    pub date: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub camera: Option<String>,
    pub orientation: Option<u32>,
}

fn dms(v: &exif::Value, reference: &str) -> Option<f64> {
    let exif::Value::Rational(r) = v else {
        return None;
    };
    if r.len() < 3 {
        return None;
    }
    let deg = r[0].to_f64() + r[1].to_f64() / 60.0 + r[2].to_f64() / 3600.0;
    Some(
        if reference.starts_with('S') || reference.starts_with('W') {
            -deg
        } else {
            deg
        },
    )
}

pub fn read_exif(bytes: &[u8]) -> ExifInfo {
    let mut out = ExifInfo::default();
    let Ok(ex) = exif::Reader::new().read_from_container(&mut Cursor::new(bytes)) else {
        return out;
    };
    let text = |t: exif::Tag| {
        ex.get_field(t, exif::In::PRIMARY)
            .map(|f| f.display_value().to_string())
    };
    if let Some(d) = text(exif::Tag::DateTimeOriginal).or_else(|| text(exif::Tag::DateTime)) {
        let day = d.split_whitespace().next().unwrap_or("").replace(':', "-");
        if day.len() == 10 && !day.starts_with("0000") {
            out.date = Some(day);
        }
    }
    let rf = |t: exif::Tag| {
        ex.get_field(t, exif::In::PRIMARY)
            .map(|f| f.display_value().to_string().replace('"', ""))
            .unwrap_or_default()
    };
    let g = |t: exif::Tag| ex.get_field(t, exif::In::PRIMARY);
    if let (Some(la), Some(lo)) = (g(exif::Tag::GPSLatitude), g(exif::Tag::GPSLongitude)) {
        out.lat = dms(&la.value, &rf(exif::Tag::GPSLatitudeRef));
        out.lon = dms(&lo.value, &rf(exif::Tag::GPSLongitudeRef));
    }
    out.camera = match (text(exif::Tag::Make), text(exif::Tag::Model)) {
        (Some(a), Some(b)) => Some(
            format!("{} {}", a.trim_matches('"'), b.trim_matches('"'))
                .trim()
                .to_string(),
        ),
        (None, Some(b)) | (Some(b), None) => Some(b.trim_matches('"').to_string()),
        _ => None,
    };
    out.orientation = ex
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0));
    out
}

/// Decode, apply EXIF orientation, return (width, height, jpeg thumbnail).
fn decode(bytes: &[u8], orientation: Option<u32>) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory(bytes).ok()?;
    let (w, h) = (img.width(), img.height());
    let img = match orientation {
        Some(3) => img.rotate180(),
        Some(6) => img.rotate90(),
        Some(8) => img.rotate270(),
        _ => img,
    };
    let th = img.thumbnail(THUMB_PX, THUMB_PX).to_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80)
        .encode_image(&th)
        .ok()?;
    Some((w, h, out))
}

fn prepared(name: &str, bytes: &[u8]) -> (Row, Option<Vec<u8>>) {
    let (kind, mime) = classify(name, bytes);
    let mut meta = Map::new();
    meta.insert("size".into(), json!(bytes.len()));
    meta.insert("mime".into(), json!(mime));
    let (mut width, mut height, mut thumb) = (None, None, None);
    if kind == "image" {
        let ex = read_exif(bytes);
        if let Some((w, h, t)) = decode(bytes, ex.orientation) {
            width = Some(w);
            height = Some(h);
            thumb = Some(t);
        }
        if ex.date.is_some() || ex.lat.is_some() || ex.camera.is_some() {
            meta.insert(
                "exif".into(),
                serde_json::to_value(&ex).unwrap_or(Value::Null),
            );
        }
    }
    let mut r = Row::new();
    r.insert("kind".into(), json!(kind));
    r.insert("hash".into(), json!(sha256_hex(bytes)));
    r.insert("width".into(), json!(width));
    r.insert("height".into(), json!(height));
    r.insert("meta".into(), json!(Value::Object(meta).to_string()));
    (r, thumb)
}

fn put_blob(store: &Store, id: &str, bytes: &[u8], thumb: Option<&[u8]>) -> Result<()> {
    store.conn().execute(
        "INSERT OR REPLACE INTO media_blob (media_id, data, thumb) VALUES (?1, ?2, ?3)",
        rusqlite::params![id, bytes, thumb],
    )?;
    Ok(())
}

fn has_blob(store: &Store, id: &str) -> bool {
    store
        .conn()
        .query_row("SELECT 1 FROM media_blob WHERE media_id = ?1", [id], |_| {
            Ok(())
        })
        .is_ok()
}

/// Add a file to the project. Identical content (same SHA-256) is stored once; the existing item is returned and,
/// when `link` is given, linked.
pub fn import(
    store: &mut Store,
    name: &str,
    bytes: &[u8],
    link: Option<(&str, &str)>,
) -> Result<Imported> {
    if bytes.is_empty() {
        return Err(StoreError::Other("empty file".into()));
    }
    let (mut row, thumb) = prepared(name, bytes);
    let hash = row["hash"].as_str().unwrap_or("").to_string();
    let existing = store
        .rows_where("media", "hash", &hash)?
        .into_iter()
        .find(|r| r["mode"] == "embedded" || has_blob(store, r["id"].as_str().unwrap_or("")));
    let kind = row["kind"].as_str().unwrap_or("other").to_string();
    let (id, duplicate) = match existing {
        Some(r) => (r["id"].as_str().unwrap_or("").to_string(), true),
        None => (new_id(), false),
    };
    if !duplicate {
        put_blob(store, &id, bytes, thumb.as_deref())?;
    }
    store.transact("Add media", |tx| {
        if !duplicate {
            row.insert("id".into(), json!(id));
            row.insert(
                "path".into(),
                json!(name.rsplit(['/', '\\']).next().unwrap_or(name)),
            );
            row.insert("mode".into(), json!("embedded"));
            tx.put_row("media", row)?;
        }
        if let Some((t, i)) = link {
            link_in(tx, &id, t, i, None)?;
        }
        Ok(())
    })?;
    Ok(Imported {
        id,
        duplicate,
        kind,
    })
}

fn link_in(
    tx: &mut crate::store::Tx,
    media: &str,
    target_type: &str,
    target: &str,
    region: Option<&str>,
) -> Result<()> {
    let already = tx
        .ids_where("media_link", "media_id", media)?
        .into_iter()
        .filter_map(|l| tx.get("media_link", &l).ok().flatten())
        .any(|r| r["target_type"] == target_type && r["target_id"] == target);
    if already {
        return Ok(());
    }
    let mut r = Row::new();
    r.insert("id".into(), json!(new_id()));
    r.insert("media_id".into(), json!(media));
    r.insert("target_type".into(), json!(target_type));
    r.insert("target_id".into(), json!(target));
    r.insert("region".into(), json!(region));
    tx.put_row("media_link", r)?;
    Ok(())
}

pub fn link(store: &mut Store, media: &str, target_type: &str, target: &str) -> Result<()> {
    if !matches!(
        target_type,
        "person" | "family" | "event" | "source" | "place"
    ) {
        return Err(StoreError::Other(format!(
            "cannot link media to {target_type}"
        )));
    }
    store.transact("Link media", |tx| {
        link_in(tx, media, target_type, target, None)
    })
}

pub fn unlink(store: &mut Store, media: &str, target_type: &str, target: &str) -> Result<()> {
    store.transact("Unlink media", |tx| {
        for l in tx.ids_where("media_link", "media_id", media)? {
            if let Some(r) = tx.get("media_link", &l)? {
                if r["target_type"] == target_type && r["target_id"] == target {
                    tx.delete("media_link", &l)?;
                }
            }
        }
        // A primary photo that is no longer linked stops being primary.
        if target_type == "person" {
            if let Some(mut p) = tx.get("person", target)? {
                if p["primary_media"] == media {
                    p.insert("primary_media".into(), Value::Null);
                    tx.put_row("person", p)?;
                }
            }
        }
        Ok(())
    })
}

pub fn delete(store: &mut Store, media: &str) -> Result<()> {
    store.transact("Delete media", |tx| {
        for l in tx.ids_where("media_link", "media_id", media)? {
            tx.delete("media_link", &l)?;
        }
        for p in tx.all_ids("person")? {
            if let Some(mut r) = tx.get("person", &p)? {
                if r["primary_media"] == media {
                    r.insert("primary_media".into(), Value::Null);
                    tx.put_row("person", r)?;
                }
            }
        }
        tx.delete("media", media)?;
        Ok(())
    })
}

/// Remove blobs whose media row no longer exists (they are kept after deletion so undo can restore them).
pub fn purge_orphans(store: &Store) -> Result<usize> {
    Ok(store.conn().execute(
        "DELETE FROM media_blob WHERE media_id NOT IN (SELECT id FROM media)",
        [],
    )?)
}

pub fn file(store: &Store, id: &str) -> Result<Option<(String, String, Vec<u8>)>> {
    let Some(m) = store.rows_where("media", "id", id)?.into_iter().next() else {
        return Ok(None);
    };
    let data: Option<Vec<u8>> = store
        .conn()
        .query_row(
            "SELECT data FROM media_blob WHERE media_id = ?1",
            [id],
            |r| r.get(0),
        )
        .ok();
    let meta: Value = m["meta"]
        .as_str()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(Value::Null);
    Ok(data.map(|d| {
        (
            m["path"].as_str().unwrap_or("file").to_string(),
            meta["mime"]
                .as_str()
                .unwrap_or("application/octet-stream")
                .to_string(),
            d,
        )
    }))
}

pub fn thumb(store: &Store, id: &str) -> Option<Vec<u8>> {
    store
        .conn()
        .query_row(
            "SELECT thumb FROM media_blob WHERE media_id = ?1",
            [id],
            |r| r.get::<_, Option<Vec<u8>>>(0),
        )
        .ok()
        .flatten()
}

/// Attach a file to an existing item that has no bytes yet (media imported from GEDCOM with only a path) or replace its bytes.
pub fn relink(store: &mut Store, id: &str, name: &str, bytes: &[u8]) -> Result<()> {
    let Some(mut m) = store.rows_where("media", "id", id)?.into_iter().next() else {
        return Err(StoreError::Other(format!("no media {id}")));
    };
    let (prep, thumb) = prepared(name, bytes);
    put_blob(store, id, bytes, thumb.as_deref())?;
    for k in ["kind", "hash", "width", "height", "meta"] {
        m.insert(k.into(), prep[k].clone());
    }
    m.insert("mode".into(), json!("embedded"));
    store.transact("Relink media", |tx| {
        tx.put_row("media", m)?;
        Ok(())
    })
}

#[derive(Debug, Default)]
pub struct Filter {
    pub target: Option<(String, String)>,
    pub kind: Option<String>,
    pub query: Option<String>,
    pub unlinked: bool,
    pub missing: bool,
}

/// Library listing, newest first.
pub fn list(store: &Store, f: &Filter, loc: Locale) -> Result<Vec<Value>> {
    let links = store.rows("media_link")?;
    let mut count: HashMap<String, usize> = HashMap::new();
    let mut targeted: HashMap<String, bool> = HashMap::new();
    for l in &links {
        let mid = l["media_id"].as_str().unwrap_or("").to_string();
        *count.entry(mid.clone()).or_default() += 1;
        if let Some((t, i)) = &f.target {
            if l["target_type"] == t.as_str() && l["target_id"] == i.as_str() {
                targeted.insert(mid, true);
            }
        }
    }
    let q = f.query.as_deref().map(crate::name::fold);
    let mut rows = store.rows("media")?;
    rows.sort_by_key(|r| std::cmp::Reverse(r["created"].as_i64().unwrap_or(0)));
    let mut out = vec![];
    for m in rows {
        let id = m["id"].as_str().unwrap_or("").to_string();
        if f.target.is_some() && !targeted.contains_key(&id) {
            continue;
        }
        if let Some(k) = &f.kind {
            if m["kind"] != k.as_str() {
                continue;
            }
        }
        let linked = count.get(&id).copied().unwrap_or(0);
        if f.unlinked && linked > 0 {
            continue;
        }
        let blob = has_blob(store, &id);
        if f.missing && blob {
            continue;
        }
        if let Some(q) = &q {
            let hay = crate::name::fold(&format!(
                "{} {}",
                m["path"].as_str().unwrap_or(""),
                m["caption"].as_str().unwrap_or("")
            ));
            if !hay.contains(q.as_str()) {
                continue;
            }
        }
        let meta: Value = m["meta"]
            .as_str()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(Value::Null);
        let date_text = m["date_json"]
            .as_str()
            .and_then(|j| serde_json::from_str::<GenDate>(j).ok())
            .map(|d| d.format_long(loc));
        out.push(json!({
            "id": id, "name": m["path"], "kind": m["kind"], "caption": m["caption"], "hash": m["hash"],
            "width": m["width"], "height": m["height"], "mode": m["mode"], "place_id": m["place_id"],
            "date": date_text, "size": meta["size"], "mime": meta["mime"], "exif": meta["exif"],
            "links": linked, "has_file": blob, "has_thumb": thumb(store, &id).is_some(),
        }));
    }
    Ok(out)
}

pub fn links(store: &Store, media: &str) -> Result<Vec<Row>> {
    store.rows_where("media_link", "media_id", media)
}

/// Date and nearest known place (within `radius_km`) suggested by the photo's EXIF data.
pub fn suggest(store: &Store, id: &str, radius_km: f64) -> Result<Value> {
    let Some((_, _, bytes)) = file(store, id)? else {
        return Ok(json!({}));
    };
    let ex = read_exif(&bytes);
    let mut place = Value::Null;
    if let (Some(la), Some(lo)) = (ex.lat, ex.lon) {
        let names = crate::places::full_names(store)?;
        let best = effective_coords(store)?
            .into_iter()
            .map(|(pid, (a, b, _))| (haversine_km((la, lo), (a, b)), pid))
            .filter(|(d, _)| *d <= radius_km)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, pid)) = best {
            place = json!({"id": pid, "name": names.get(&pid), "distance_km": (d * 10.0).round() / 10.0});
        }
    }
    Ok(json!({"date": ex.date, "lat": ex.lat, "lon": ex.lon, "place": place, "camera": ex.camera}))
}

/// Duplicate groups by content hash (can occur after GEDCOM imports that reference the same file twice).
pub fn duplicate_groups(store: &Store) -> Result<Vec<Vec<String>>> {
    let mut by: HashMap<String, Vec<String>> = HashMap::new();
    for m in store.rows("media")? {
        if let Some(h) = m["hash"].as_str() {
            by.entry(h.to_string())
                .or_default()
                .push(m["id"].as_str().unwrap_or("").to_string());
        }
    }
    let mut v: Vec<Vec<String>> = by.into_values().filter(|g| g.len() > 1).collect();
    v.iter_mut().for_each(|g| g.sort());
    v.sort();
    Ok(v)
}
