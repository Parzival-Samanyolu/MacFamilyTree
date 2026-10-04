use kintree_core::date::Locale;
use kintree_core::gedcom;
use kintree_core::geo::geocode_offline;
use kintree_core::media::*;
use kintree_core::places::full_names;
use kintree_core::store::Store;

fn project() -> (Store, String) {
    let mut s = Store::open_memory().unwrap();
    gedcom::import(
        &mut s,
        b"0 HEAD\n1 GEDC\n2 VERS 5.5.1\n0 @I1@ INDI\n1 NAME Ali /Kaya/\n1 BIRT\n2 PLAC Istanbul, Turkey\n0 TRLR\n",
    )
    .unwrap();
    let id = s.rows("person").unwrap()[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    (s, id)
}

fn png(w: u32, h: u32, shade: u8) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, image::Rgb([shade, 120, 200]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

/// Minimal little-endian TIFF carrying DateTimeOriginal and a GPS position (41°1′N, 28°58′E).
fn tiff_with_gps() -> Vec<u8> {
    fn entry(b: &mut Vec<u8>, tag: u16, typ: u16, count: u32, val: [u8; 4]) {
        b.extend(tag.to_le_bytes());
        b.extend(typ.to_le_bytes());
        b.extend(count.to_le_bytes());
        b.extend(val);
    }
    let mut b = vec![b'I', b'I', 42, 0, 8, 0, 0, 0];
    b.extend(2u16.to_le_bytes());
    entry(&mut b, 0x8769, 4, 1, 38u32.to_le_bytes());
    entry(&mut b, 0x8825, 4, 1, 56u32.to_le_bytes());
    b.extend(0u32.to_le_bytes());
    assert_eq!(b.len(), 38);
    b.extend(1u16.to_le_bytes());
    entry(&mut b, 0x9003, 2, 20, 110u32.to_le_bytes());
    b.extend(0u32.to_le_bytes());
    assert_eq!(b.len(), 56);
    b.extend(4u16.to_le_bytes());
    entry(&mut b, 1, 2, 2, [b'N', 0, 0, 0]);
    entry(&mut b, 2, 5, 3, 130u32.to_le_bytes());
    entry(&mut b, 3, 2, 2, [b'E', 0, 0, 0]);
    entry(&mut b, 4, 5, 3, 154u32.to_le_bytes());
    b.extend(0u32.to_le_bytes());
    assert_eq!(b.len(), 110);
    b.extend(b"2019:05:04 10:11:12\0");
    for (d, m, s) in [(41u32, 1u32, 0u32), (28, 58, 0)] {
        for v in [d, 1, m, 1, s, 1] {
            b.extend(v.to_le_bytes());
        }
    }
    b
}

#[test]
fn import_dedupes_by_content_and_links_once() {
    let (mut s, pid) = project();
    let a = import(
        &mut s,
        "photos/a.png",
        &png(40, 20, 10),
        Some(("person", &pid)),
    )
    .unwrap();
    assert!(!a.duplicate && a.kind == "image");
    let b = import(
        &mut s,
        "copy-of-a.png",
        &png(40, 20, 10),
        Some(("person", &pid)),
    )
    .unwrap();
    assert!(b.duplicate && b.id == a.id);
    assert_eq!(s.rows("media").unwrap().len(), 1);
    assert_eq!(links(&s, &a.id).unwrap().len(), 1, "no duplicate link");
    let c = import(&mut s, "b.png", &png(40, 20, 99), None).unwrap();
    assert_ne!(c.id, a.id);
    assert!(import(&mut s, "empty.png", b"", None).is_err());
    let (name, mime, bytes) = file(&s, &a.id).unwrap().unwrap();
    assert_eq!((name.as_str(), mime.as_str()), ("a.png", "image/png"));
    assert_eq!(
        sha256_hex(&bytes),
        s.rows("media")
            .unwrap()
            .iter()
            .find(|m| m["id"] == a.id.as_str())
            .unwrap()["hash"]
            .as_str()
            .unwrap()
    );
}

#[test]
fn thumbnails_are_bounded_and_dimensions_recorded() {
    let (mut s, _) = project();
    let m = import(&mut s, "big.png", &png(1000, 500, 5), None).unwrap();
    let row = s.rows("media").unwrap().remove(0);
    assert_eq!(
        (row["width"].as_i64(), row["height"].as_i64()),
        (Some(1000), Some(500))
    );
    let th = thumb(&s, &m.id).unwrap();
    let img = image::load_from_memory(&th).unwrap();
    assert_eq!((img.width(), img.height()), (320, 160));
    // Non-images get no thumbnail but are still stored.
    let d = import(&mut s, "notes.pdf", b"%PDF-1.4 fake", None).unwrap();
    assert_eq!(d.kind, "document");
    assert!(thumb(&s, &d.id).is_none() && file(&s, &d.id).unwrap().is_some());
}

#[test]
fn exif_date_and_gps_suggest_a_nearby_place() {
    let (mut s, _) = project();
    let ex = read_exif(&tiff_with_gps());
    assert_eq!(ex.date.as_deref(), Some("2019-05-04"));
    assert!((ex.lat.unwrap() - 41.0167).abs() < 0.001 && (ex.lon.unwrap() - 28.9667).abs() < 0.001);
    let names = full_names(&s).unwrap();
    s.transact("g", |tx| geocode_offline(tx, &names)).unwrap();
    let m = import(&mut s, "scan.tif", &tiff_with_gps(), None).unwrap();
    let sg = suggest(&s, &m.id, 50.0).unwrap();
    assert_eq!(sg["date"], "2019-05-04");
    assert_eq!(sg["place"]["name"], "Istanbul, Turkey");
    assert!(suggest(&s, &m.id, 0.001).unwrap()["place"].is_null());
    let listed = list(&s, &Filter::default(), Locale::En).unwrap();
    assert_eq!(listed[0]["exif"]["date"], "2019-05-04");
    assert_eq!(read_exif(b"not an image"), ExifInfo::default());
}

#[test]
fn delete_is_undoable_and_blobs_are_purged_only_when_orphaned() {
    let (mut s, pid) = project();
    let m = import(&mut s, "a.png", &png(10, 10, 1), Some(("person", &pid))).unwrap();
    s.transact("primary", |tx| {
        let mut p = tx.get("person", &pid)?.unwrap();
        p.insert("primary_media".into(), m.id.clone().into());
        tx.put_row("person", p)?;
        Ok(())
    })
    .unwrap();
    delete(&mut s, &m.id).unwrap();
    assert!(s.rows("media").unwrap().is_empty() && s.rows("media_link").unwrap().is_empty());
    s.undo().unwrap();
    assert_eq!(s.rows("media").unwrap().len(), 1);
    assert!(
        file(&s, &m.id).unwrap().is_some(),
        "bytes survive delete + undo"
    );
    assert_eq!(s.rows("person").unwrap()[0]["primary_media"], m.id.as_str());
    assert_eq!(purge_orphans(&s).unwrap(), 0);
    delete(&mut s, &m.id).unwrap();
    assert_eq!(purge_orphans(&s).unwrap(), 1);
    assert!(file(&s, &m.id).unwrap().is_none());
}

#[test]
fn unlink_clears_primary_photo_and_list_filters_work() {
    let (mut s, pid) = project();
    let a = import(
        &mut s,
        "grandpa.png",
        &png(10, 10, 1),
        Some(("person", &pid)),
    )
    .unwrap();
    let b = import(&mut s, "orphan.png", &png(10, 10, 2), None).unwrap();
    let f = |f: Filter| {
        list(&s, &f, Locale::En)
            .unwrap()
            .into_iter()
            .map(|v| v["id"].as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        f(Filter {
            target: Some(("person".into(), pid.clone())),
            ..Default::default()
        }),
        vec![a.id.clone()]
    );
    assert_eq!(
        f(Filter {
            unlinked: true,
            ..Default::default()
        }),
        vec![b.id.clone()]
    );
    assert_eq!(
        f(Filter {
            query: Some("GRANDPA".into()),
            ..Default::default()
        }),
        vec![a.id.clone()]
    );
    assert!(f(Filter {
        kind: Some("audio".into()),
        ..Default::default()
    })
    .is_empty());
    assert!(link(&mut s, &b.id, "bogus", &pid).is_err());
    unlink(&mut s, &a.id, "person", &pid).unwrap();
    assert!(links(&s, &a.id).unwrap().is_empty());
}

#[test]
fn relink_attaches_bytes_to_path_only_media() {
    let (mut s, _) = project();
    s.transact("gedcom-like", |tx| {
        let mut r = serde_json::Map::new();
        r.insert("id".into(), "m1".into());
        r.insert("path".into(), "C:\\old\\pic.png".into());
        r.insert("mode".into(), "linked".into());
        tx.put_row("media", r)?;
        Ok(())
    })
    .unwrap();
    let miss = list(
        &s,
        &Filter {
            missing: true,
            ..Default::default()
        },
        Locale::En,
    )
    .unwrap();
    assert_eq!(miss.len(), 1);
    relink(&mut s, "m1", "pic.png", &png(30, 30, 7)).unwrap();
    assert!(list(
        &s,
        &Filter {
            missing: true,
            ..Default::default()
        },
        Locale::En
    )
    .unwrap()
    .is_empty());
    assert!(thumb(&s, "m1").is_some());
    assert_eq!(s.rows("media").unwrap()[0]["mode"], "embedded");
    assert!(relink(&mut s, "nope", "x.png", b"1").is_err());
}

#[test]
fn media_survives_file_backup() {
    let dir = std::env::temp_dir().join(format!("kt-m-{}", kintree_core::store::new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Store::open(&dir.join("p.db")).unwrap();
    let m = import(&mut s, "a.png", &png(10, 10, 1), None).unwrap();
    s.backup_to(&dir.join("b.db")).unwrap();
    let b = Store::open(&dir.join("b.db")).unwrap();
    assert!(file(&b, &m.id).unwrap().is_some());
}
