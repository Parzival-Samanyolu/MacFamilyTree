//! Extra behaviour tests that pin down merge, charset and data-integrity paths.

use kintree_core::duplicates;
use kintree_core::gedcom::{self, charset, Charset};
use kintree_core::name::PersonName;
use kintree_core::places;
use kintree_core::quality::{self, Fix, Rules};
use kintree_core::store::Store;
use serde_json::{json, Map, Value};

fn row(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

// ---------------- places ----------------

#[test]
fn place_merge_moves_events_and_children_and_keeps_coordinates() {
    let mut s = Store::open_memory().unwrap();
    let (a, b, a_child, b_child_same, b_child_new) = s
        .transact("places", |tx| {
            let a = places::find_or_create(tx, "Konya, Turkey")?.unwrap();
            let b = places::find_or_create(tx, "Türkiye")?; // distinct root named differently
            let _ = b;
            let konya_a = places::find_or_create(tx, "Selçuklu, Konya, Turkey")?.unwrap();
            // a second, separate hierarchy under another root with the same child names
            let root2 = tx.put_row(
                "place",
                row(&[
                    ("id", "r2".into()),
                    ("name", "Turkey".into()),
                    ("lat", json!(39.0)),
                    ("lon", json!(35.0)),
                ]),
            )?;
            let k2 = tx.put_row(
                "place",
                row(&[
                    ("id", "k2".into()),
                    ("name", "Konya".into()),
                    ("parent_id", root2.clone().into()),
                ]),
            )?;
            let sel2 = tx.put_row(
                "place",
                row(&[
                    ("id", "s2".into()),
                    ("name", "Selçuklu".into()),
                    ("parent_id", k2.clone().into()),
                ]),
            )?;
            let new2 = tx.put_row(
                "place",
                row(&[
                    ("id", "n2".into()),
                    ("name", "Meram".into()),
                    ("parent_id", k2.clone().into()),
                ]),
            )?;
            let p = tx.create_person(&PersonName::new("Ali", "Kaya"), "M")?;
            tx.add_event("person", &p, "BIRT", None, Some(&sel2))?;
            tx.put_row(
                "media",
                row(&[
                    ("id", "m1".into()),
                    ("path", "x.png".into()),
                    ("place_id", k2.clone().into()),
                ]),
            )?;
            let _ = a;
            Ok((k2, root2, konya_a, sel2, new2))
        })
        .unwrap();
    let _ = (&a, &b);
    // merge the second Konya into the first one
    let target = s
        .rows("place")
        .unwrap()
        .into_iter()
        .find(|r| r["name"] == "Konya" && r["id"] != a.as_str())
        .unwrap();
    let keep = target["id"].as_str().unwrap().to_string();
    let _ = a_child;
    s.transact("merge", |tx| places::merge(tx, &keep, &a))
        .unwrap();
    let rows = s.rows("place").unwrap();
    assert!(
        !rows.iter().any(|r| r["id"] == a.as_str()),
        "removed place is gone"
    );
    assert!(
        rows.iter()
            .any(|r| r["id"] == b_child_new.as_str() && r["parent_id"] == keep.as_str()),
        "unique child re-parented"
    );
    assert!(
        !rows.iter().any(|r| r["id"] == b_child_same.as_str()),
        "same-named child merged away"
    );
    let ev = s.rows("event").unwrap();
    let survivor = rows.iter().find(|r| r["name"] == "Selçuklu").unwrap()["id"].clone();
    assert_eq!(
        ev[0]["place_id"], survivor,
        "events follow the merged child"
    );
    assert_eq!(s.rows("media").unwrap()[0]["place_id"], json!(keep));
    s.transact("noop", |tx| places::merge(tx, &keep, &keep))
        .unwrap();
}

#[test]
fn place_merge_copies_coordinates_to_a_survivor_without_any() {
    let mut s = Store::open_memory().unwrap();
    s.transact("p", |tx| {
        tx.put_row(
            "place",
            row(&[("id", "keep".into()), ("name", "Ankara".into())]),
        )?;
        tx.put_row(
            "place",
            row(&[
                ("id", "dup".into()),
                ("name", "ankara".into()),
                ("lat", json!(39.9)),
                ("lon", json!(32.8)),
            ]),
        )?;
        Ok(())
    })
    .unwrap();
    assert_eq!(places::find_duplicate_places(&s).unwrap().len(), 1);
    s.transact("m", |tx| places::merge(tx, "keep", "dup"))
        .unwrap();
    let k = s.rows("place").unwrap();
    assert_eq!(k.len(), 1);
    assert_eq!(k[0]["lat"], json!(39.9));
    assert!(places::find_duplicate_places(&s).unwrap().is_empty());
}

// ---------------- family merge ----------------

#[test]
fn merging_families_moves_children_and_collapses_duplicate_events() {
    let mut s = Store::open_memory().unwrap();
    let (f1, f2, c1, c2) = s
        .transact("fam", |tx| {
            let h = tx.create_person(&PersonName::new("Ali", "Kaya"), "M")?;
            let w = tx.create_person(&PersonName::new("Ayşe", "Kaya"), "F")?;
            let c1 = tx.create_person(&PersonName::new("Can", "Kaya"), "M")?;
            let c2 = tx.create_person(&PersonName::new("Cem", "Kaya"), "M")?;
            let f1 = tx.create_family(Some(&h), Some(&w), "married")?;
            let f2 = tx.create_family(Some(&h), Some(&w), "married")?;
            tx.add_child(&f1, &c1, "biological")?;
            tx.add_child(&f2, &c1, "biological")?; // duplicate child
            tx.add_child(&f2, &c2, "biological")?;
            let d = kintree_core::date::GenDate::parse("1 Jan 1900").unwrap();
            tx.add_event("family", &f1, "MARR", Some(&d), None)?;
            tx.add_event("family", &f2, "MARR", Some(&d), None)?; // same signature
            let d2 = kintree_core::date::GenDate::parse("1 Jan 1920").unwrap();
            tx.add_event("family", &f2, "DIV", Some(&d2), None)?;
            Ok((f1, f2, c1, c2))
        })
        .unwrap();
    s.transact("merge", |tx| duplicates::merge_families(tx, &f1, &f2))
        .unwrap();
    assert_eq!(s.rows("family").unwrap().len(), 1);
    let kids: Vec<String> = s
        .rows("family_child")
        .unwrap()
        .iter()
        .map(|r| r["person_id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(kids.len(), 2);
    assert!(kids.contains(&c1) && kids.contains(&c2));
    let kinds: Vec<String> = s
        .rows("event")
        .unwrap()
        .iter()
        .map(|r| r["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        kinds.iter().filter(|k| *k == "MARR").count(),
        1,
        "identical marriage collapsed"
    );
    assert!(kinds.contains(&"DIV".to_string()));
}

// ---------------- charsets ----------------

#[test]
fn ansel_decodes_specials_and_combining_marks() {
    let mut warns = vec![];
    let mut body = b"0 HEAD\n1 CHAR ANSEL\n1 NOTE ".to_vec();
    body.extend([0xA1, 0xA2, 0xA5, 0xB1, 0xB2, 0xB5, 0xB9, 0xBA, 0xC3, 0xCF]);
    body.extend(b" e");
    body.extend([
        0xE2, b'e', 0xE8, b'u', 0xE9, b'c', 0xF0, b's', 0xE4, b'n', 0xE1, b'a', 0xE3, b'o', 0xEA,
        b'a',
    ]);
    body.extend(b"\n0 TRLR\n");
    let (text, cs) = charset::decode(&body, &mut |w| warns.push(w));
    assert_eq!(cs, Charset::Ansel);
    for ch in ["Ł", "Ø", "Æ", "ł", "ø", "æ", "£", "ð", "©", "ß"] {
        assert!(text.contains(ch), "{ch} in {text:?}");
    }
    assert!(
        text.contains('é')
            && text.contains('ü')
            && text.contains('č')
            && text.contains('ş')
            && text.contains('ñ')
            && text.contains('à')
            && text.contains('ô')
            && text.contains('å'),
        "{text:?}"
    );
}

#[test]
fn every_charset_round_trips_text_it_can_represent() {
    let sample = "0 HEAD\n1 NOTE Çağrı Öz ğüşıİ café\n0 TRLR\n";
    for cs in [Charset::Utf8, Charset::Utf16Le, Charset::Utf16Be] {
        let bytes = charset::encode(sample, cs);
        let (text, got) = charset::decode(&bytes, &mut |_| {});
        assert_eq!(text, sample, "{cs:?}");
        assert!(
            matches!(
                (cs, got),
                (Charset::Utf8, Charset::Utf8)
                    | (Charset::Utf16Le, Charset::Utf16Le)
                    | (Charset::Utf16Be, Charset::Utf16Be)
            ),
            "{cs:?} -> {got:?}"
        );
    }
    // Lossy single-byte sets fold what they cannot hold instead of failing.
    let ascii = charset::encode("Çağrı", Charset::Ascii);
    assert!(ascii.iter().all(|b| b.is_ascii()) && ascii.starts_with(b"Cag"));
    let latin = charset::encode("café", Charset::Latin1);
    assert_eq!(latin, b"caf\xE9");
    let (back, _) = charset::decode(
        b"0 HEAD\n1 CHAR ISO-8859-1\n1 NOTE caf\xE9\n0 TRLR\n",
        &mut |_| {},
    );
    assert!(back.contains("café"));
    let (cp, c) = charset::decode(
        b"0 HEAD\n1 CHAR ANSI\n1 NOTE \x93q\x94 \x80\n0 TRLR\n",
        &mut |_| {},
    );
    assert_eq!(c, Charset::Cp1252);
    assert!(cp.contains("“q” €"));
    let (bom, c) = charset::decode(b"\xEF\xBB\xBF0 HEAD\n0 TRLR\n", &mut |_| {});
    assert!(bom.starts_with("0 HEAD") && c == Charset::Utf8);
    for (cs, name) in [
        (Charset::Utf8, "UTF-8"),
        (Charset::Utf16Le, "UNICODE"),
        (Charset::Ansel, "ANSEL"),
        (Charset::Ascii, "ASCII"),
        (Charset::Latin1, "ISO-8859-1"),
        (Charset::Cp1252, "ANSI"),
    ] {
        assert_eq!(charset::gedcom_char_name(cs), name);
    }
}

#[test]
fn bomless_utf16_is_detected() {
    let sample = "0 HEAD\n0 TRLR\n";
    let le: Vec<u8> = sample
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    let be: Vec<u8> = sample
        .encode_utf16()
        .flat_map(|u| u.to_be_bytes())
        .collect();
    assert_eq!(charset::decode(&le, &mut |_| {}).1, Charset::Utf16Le);
    assert_eq!(charset::decode(&be, &mut |_| {}).1, Charset::Utf16Be);
}

// ---------------- data-quality rules and fixes ----------------

const MESSY: &[u8] = b"0 HEAD\n1 GEDC\n2 VERS 5.5.1\n\
0 @I1@ INDI\n1 NAME Ali /Kaya/\n1 SEX M\n1 BIRT\n2 DATE 1800\n1 RESI\n2 DATE FROM 1820 TO 1840\n2 PLAC Konya\n1 RESI\n2 DATE FROM 1830 TO 1850\n2 PLAC Ankara\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Ayse /Demir/\n1 SEX F\n1 BIRT\n2 DATE 1820\n1 DEAT\n2 DATE 1840\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Can /Kaya/\n1 BIRT\n2 DATE 1895\n1 FAMC @F1@\n1 FAMS @F2@\n\
0 @I4@ INDI\n1 NAME Zed /Kaya/\n1 BIRT\n2 DATE 1700\n1 FAMS @F2@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 CHIL @I5@\n1 MARR\n2 DATE 1790\n\
0 @F2@ FAM\n1 HUSB @I3@\n1 WIFE @I4@\n1 MARR\n2 DATE 1850\n\
0 @F3@ FAM\n\
0 @F4@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 MARR\n2 DATE 1860\n\
0 @I5@ INDI\n1 NAME Kid /Kaya/\n1 BIRT\n2 DATE 1805\n1 FAMC @F1@\n0 TRLR\n";

#[test]
fn messy_tree_triggers_the_expected_rules() {
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, MESSY).unwrap();
    let found = quality::check_all(&s, &Rules::default()).unwrap();
    let has = |rule: &str| found.iter().any(|f| f.rule == rule);
    for rule in [
        "overlapping_residence",
        "parent_too_old",
        "marriage_before_birth",
        "marriage_after_death",
        "empty_family",
        "missing_sex",
        "parent_too_young",
    ] {
        assert!(
            has(rule),
            "{rule} not raised; got {:?}",
            found.iter().map(|f| &f.rule).collect::<Vec<_>>()
        );
    }
}

#[test]
fn dangling_references_are_reported_and_fixes_apply() {
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, MESSY).unwrap();
    s.transact("break", |tx| {
        tx.put_row(
            "event",
            row(&[
                ("id", "e-orphan".into()),
                ("owner_type", "person".into()),
                ("owner_id", "ghost".into()),
                ("kind", "BIRT".into()),
            ]),
        )?;
        tx.put_row(
            "family_child",
            row(&[
                ("id", "fc-bad".into()),
                ("family_id", "ghost".into()),
                ("person_id", "ghost2".into()),
            ]),
        )?;
        tx.put_row(
            "citation",
            row(&[
                ("id", "c-bad".into()),
                ("source_id", "ghost".into()),
                ("target_type", "person".into()),
                ("target_id", "ghost".into()),
            ]),
        )?;
        tx.put_row(
            "media_link",
            row(&[
                ("id", "ml-bad".into()),
                ("media_id", "ghost".into()),
                ("target_type", "person".into()),
                ("target_id", "ghost".into()),
            ]),
        )?;
        tx.put_row(
            "note_link",
            row(&[
                ("id", "nl-bad".into()),
                ("note_id", "ghost".into()),
                ("target_type", "person".into()),
                ("target_id", "ghost".into()),
            ]),
        )?;
        Ok(())
    })
    .unwrap();
    let refs = quality::check_references(&s, &Rules::default()).unwrap();
    let kinds: std::collections::HashSet<&str> =
        refs.iter().map(|f| f.entity_type.as_str()).collect();
    for k in ["event", "family", "citation", "media_link", "note_link"] {
        assert!(kinds.contains(k), "{k} in {kinds:?}");
    }
    // ignoring a finding hides it
    let first = refs[0].id.clone();
    quality::ignore(&mut s, &first).unwrap();
    assert!(!quality::check_all(&s, &Rules::default())
        .unwrap()
        .iter()
        .any(|f| f.id == first));
    // fixes
    let dated = s
        .rows("event")
        .unwrap()
        .into_iter()
        .find(|e| e["date_json"].is_string())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    s.transact("fix", |tx| {
        quality::apply_fix(
            tx,
            &Fix::ClearDate {
                event_id: dated.clone(),
            },
        )?;
        quality::apply_fix(
            tx,
            &Fix::DeleteEvent {
                event_id: "e-orphan".into(),
            },
        )
    })
    .unwrap();
    let ev = s.rows("event").unwrap();
    assert!(ev.iter().find(|e| e["id"] == dated.as_str()).unwrap()["date_json"].is_null());
    assert!(!ev.iter().any(|e| e["id"] == "e-orphan"));
}
