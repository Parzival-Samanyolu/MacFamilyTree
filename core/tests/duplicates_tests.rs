use kintree_core::duplicates::*;
use kintree_core::gedcom::{self, ExportOptions};
use kintree_core::store::Store;

const HDR: &str = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n";

fn load(ged: &str) -> Store {
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, ged.as_bytes()).unwrap();
    s
}

fn ids(s: &Store, given: &str) -> Vec<String> {
    s.rows("person_name")
        .unwrap()
        .into_iter()
        .filter(|n| n["given"] == given)
        .map(|n| n["person_id"].as_str().unwrap().to_string())
        .collect()
}

const TREE: &str = "\
0 @I1@ INDI\n1 NAME Ahmet /Yılmaz/\n1 SEX M\n1 BIRT\n2 DATE 3 MAR 1850\n2 PLAC Konya, Turkey\n1 SOUR @S1@\n2 PAGE 1\n1 FAMS @F1@\n1 FAMC @F0@\n\
0 @I2@ INDI\n1 NAME Ahmed /Yilmaz/\n1 SEX M\n1 BIRT\n2 DATE 1850\n2 PLAC Konya, Turkey\n1 DEAT\n2 DATE 1920\n1 NOTE only on duplicate\n1 FAMS @F2@\n\
0 @I3@ INDI\n1 NAME Ayşe /Kaya/\n1 SEX F\n1 FAMS @F1@\n\
0 @I4@ INDI\n1 NAME Ayse /Kaya/\n1 SEX F\n1 FAMS @F2@\n\
0 @I5@ INDI\n1 NAME Mehmet /Yılmaz/\n1 SEX M\n1 BIRT\n2 DATE 1880\n1 FAMC @F1@\n\
0 @I6@ INDI\n1 NAME Mehmet /Yilmaz/\n1 SEX M\n1 BIRT\n2 DATE 1880\n1 FAMC @F2@\n\
0 @I7@ INDI\n1 NAME John /Smith/\n1 SEX M\n1 BIRT\n2 DATE 1850\n\
0 @I8@ INDI\n1 NAME Ahmet /Yılmaz/\n1 SEX F\n1 BIRT\n2 DATE 1850\n\
0 @I9@ INDI\n1 NAME Dede /Yılmaz/\n1 SEX M\n1 FAMS @F0@\n\
0 @F0@ FAM\n1 HUSB @I9@\n1 CHIL @I1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I3@\n1 CHIL @I5@\n1 MARR\n2 DATE 1875\n\
0 @F2@ FAM\n1 HUSB @I2@\n1 WIFE @I4@\n1 CHIL @I6@\n1 MARR\n2 DATE 1875\n\
0 @S1@ SOUR\n1 TITL Parish book\n0 TRLR\n";

#[test]
fn finds_turkish_diacritic_variants_and_rejects_others() {
    let s = load(&format!("{HDR}{TREE}"));
    let c = find_duplicates(&s, 0.7, 50).unwrap();
    let pairs: Vec<(String, String)> = c.iter().map(|x| (x.a.clone(), x.b.clone())).collect();
    let has = |g: &str| {
        let v = ids(&s, g);
        pairs.iter().any(|(a, b)| v.contains(a) && v.contains(b))
    };
    assert!(has("Ahmet") || has("Mehmet"), "{c:#?}");
    // Mehmet pair: same names ignoring diacritics, same birth year
    let mm = ids(&s, "Mehmet");
    assert!(pairs.iter().any(|(a, b)| mm.contains(a) && mm.contains(b)));
    // John Smith and the female Ahmet are never suggested; opposite-sex "Ahmet" excluded
    let john = &ids(&s, "John")[0];
    assert!(pairs.iter().all(|(a, b)| a != john && b != john));
    let am = ids(&s, "Ahmet");
    let female = s
        .rows("person")
        .unwrap()
        .into_iter()
        .find(|p| p["sex"] == "F" && am.contains(&p["id"].as_str().unwrap().to_string()))
        .unwrap();
    let fid = female["id"].as_str().unwrap();
    assert!(pairs.iter().all(|(a, b)| a != fid && b != fid));
    // ordered by score
    assert!(c.windows(2).all(|w| w[0].score >= w[1].score));
}

#[test]
fn not_a_duplicate_is_remembered() {
    let mut s = load(&format!("{HDR}{TREE}"));
    let before = find_duplicates(&s, 0.7, 50).unwrap();
    assert!(!before.is_empty());
    mark_not_duplicate(&mut s, &before[0].b, &before[0].a).unwrap();
    let after = find_duplicates(&s, 0.7, 50).unwrap();
    assert_eq!(after.len(), before.len() - 1);
    assert!(!after
        .iter()
        .any(|c| c.a == before[0].a && c.b == before[0].b));
}

#[test]
fn merge_rewires_everything_and_undo_restores() {
    let mut s = load(&format!("{HDR}{TREE}"));
    let mut ah = ids(&s, "Ahmet");
    ah.extend(ids(&s, "Ahmed"));
    let rows = s.rows("person").unwrap();
    let male: Vec<String> = ah
        .iter()
        .filter(|id| {
            rows.iter()
                .any(|p| p["id"].as_str() == Some(id.as_str()) && p["sex"] == "M")
        })
        .cloned()
        .collect();
    assert_eq!(male.len(), 2);
    let (keep, remove) = (male[0].clone(), male[1].clone());
    let counts_before: Vec<i64> = [
        "person",
        "person_name",
        "event",
        "family",
        "family_child",
        "note_link",
        "citation",
    ]
    .iter()
    .map(|t| s.count(t).unwrap())
    .collect();
    let export_before =
        String::from_utf8(gedcom::export(&s, &ExportOptions::default()).unwrap()).unwrap();

    s.transact("Merge", |tx| merge_persons(tx, &keep, &remove))
        .unwrap();
    assert_eq!(s.count("person").unwrap(), counts_before[0] - 1);
    // no row may still reference the removed person
    let rf = |t: &str, col: &str| {
        s.rows(t)
            .unwrap()
            .iter()
            .any(|r| r[col].as_str() == Some(remove.as_str()))
    };
    assert!(
        !rf("person_name", "person_id")
            && !rf("event", "owner_id")
            && !rf("family", "partner1")
            && !rf("family_child", "person_id")
    );
    assert!(!rf("note_link", "target_id") && !rf("citation", "target_id"));
    // keep gained: both families as partner, the death event and the note
    let fams = s
        .rows("family")
        .unwrap()
        .iter()
        .filter(|f| f["partner1"].as_str() == Some(keep.as_str()))
        .count();
    assert_eq!(fams, 2);
    let evs: Vec<String> = s
        .rows("event")
        .unwrap()
        .into_iter()
        .filter(|e| e["owner_id"].as_str() == Some(keep.as_str()))
        .map(|e| e["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        evs.iter().filter(|k| *k == "BIRT").count(),
        2,
        "birth dates differ (3 MAR 1850 vs 1850) so both are kept: {evs:?}"
    );
    assert!(evs.contains(&"DEAT".to_string()));
    assert!(s
        .rows("note_link")
        .unwrap()
        .iter()
        .any(|l| l["target_id"].as_str() == Some(keep.as_str())));
    // search index no longer finds the removed id
    let hits = s.search_persons("ahmed", 10).unwrap();
    assert!(!hits.contains(&remove));
    assert!(
        hits.contains(&keep),
        "alternate name (Ahmed) searchable on merged person"
    );

    s.undo().unwrap();
    let counts_after: Vec<i64> = [
        "person",
        "person_name",
        "event",
        "family",
        "family_child",
        "note_link",
        "citation",
    ]
    .iter()
    .map(|t| s.count(t).unwrap())
    .collect();
    assert_eq!(counts_before, counts_after);
    assert_eq!(
        export_before,
        String::from_utf8(gedcom::export(&s, &ExportOptions::default()).unwrap()).unwrap()
    );
}

#[test]
fn merging_identical_records_collapses_names_events_and_child_links() {
    let mut s = load(&format!(
        "{HDR}0 @I1@ INDI\n1 NAME Same /Person/\n1 BIRT\n2 DATE 1900\n2 SOUR @S1@\n1 FAMC @F1@\n\
0 @I2@ INDI\n1 NAME Same /Person/\n1 BIRT\n2 DATE 1900\n2 SOUR @S1@\n3 PAGE 9\n1 FAMC @F1@\n\
0 @I3@ INDI\n1 NAME Dad /Person/\n1 FAMS @F1@\n0 @F1@ FAM\n1 HUSB @I3@\n1 CHIL @I1@\n1 CHIL @I2@\n\
0 @S1@ SOUR\n1 TITL T\n0 TRLR\n"
    ));
    let v = ids(&s, "Same");
    s.transact("Merge", |tx| merge_persons(tx, &v[0], &v[1]))
        .unwrap();
    assert_eq!(
        s.count("person_name").unwrap(),
        2,
        "duplicate name collapsed"
    );
    assert_eq!(s.count("event").unwrap(), 1, "identical birth collapsed");
    assert_eq!(
        s.count("family_child").unwrap(),
        1,
        "no duplicate child link"
    );
    assert_eq!(
        s.count("citation").unwrap(),
        2,
        "citations of the collapsed event move to the survivor"
    );
}
