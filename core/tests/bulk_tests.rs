use kintree_core::bulk::*;
use kintree_core::gedcom;
use kintree_core::store::Store;

fn spec(field: &str, find: &str, replace: &str) -> Spec {
    Spec {
        field: field.into(),
        find: find.into(),
        replace: replace.into(),
        case_sensitive: false,
        whole_field: false,
    }
}

#[test]
fn replace_in_handles_case_unicode_overlap_and_whole_field() {
    assert_eq!(
        replace_in("Ankara, ankara", &spec("place.name", "ANKARA", "Angora")).unwrap(),
        "Angora, Angora"
    );
    let mut cs = spec("place.name", "ankara", "X");
    cs.case_sensitive = true;
    assert_eq!(replace_in("Ankara ankara", &cs).unwrap(), "Ankara X");
    assert_eq!(
        replace_in(
            "İstanbul",
            &spec("place.name", "i̇stanbul", "Konstantiniyye")
        ),
        None,
        "combining form is not equal"
    );
    assert_eq!(
        replace_in("Şişli", &spec("place.name", "şişli", "Sisli")).unwrap(),
        "Sisli"
    );
    assert_eq!(
        replace_in("aaaa", &spec("place.name", "aa", "b")).unwrap(),
        "bb"
    );
    assert_eq!(replace_in("abc", &spec("place.name", "", "x")), None);
    assert_eq!(replace_in("abc", &spec("place.name", "zz", "x")), None);
    assert_eq!(
        replace_in("abc", &spec("place.name", "b", "b")),
        None,
        "no-op replacements are not changes"
    );
    let mut whole = spec("place.name", "Konya", "Konya Province");
    whole.whole_field = true;
    assert_eq!(replace_in("konya", &whole).unwrap(), "Konya Province");
    assert_eq!(replace_in("Konya City", &whole), None);
}

#[test]
fn preview_then_apply_changes_exactly_the_previewed_records_and_undoes() {
    let mut s = Store::open_memory().unwrap();
    gedcom::import(
        &mut s,
        b"0 HEAD\n1 GEDC\n2 VERS 5.5.1\n0 @I1@ INDI\n1 NAME Ali /Kaya/\n1 BIRT\n2 PLAC Angora\n0 @I2@ INDI\n1 NAME Veli /Kaja/\n0 @I3@ INDI\n1 NAME Ayse /Kaya/\n0 TRLR\n",
    )
    .unwrap();
    let sp = spec("person_name.surname", "kaya", "Kaplan");
    let pv = preview(&s, &sp).unwrap();
    assert_eq!(pv.len(), 2);
    assert!(pv.iter().all(|c| c.before == "Kaya" && c.after == "Kaplan"));
    let n = s.transact("Replace", |tx| apply(tx, &sp)).unwrap();
    assert_eq!(n, 2);
    let names: Vec<String> = s
        .rows("person_name")
        .unwrap()
        .iter()
        .map(|r| r["surname"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names.iter().filter(|n| *n == "Kaplan").count(), 2);
    assert!(names.contains(&"Kaja".to_string()));
    assert_eq!(
        s.search_persons("kaplan", 10).unwrap().len(),
        2,
        "search index follows"
    );
    assert!(preview(&s, &sp).unwrap().is_empty(), "idempotent");
    s.undo().unwrap();
    assert_eq!(preview(&s, &sp).unwrap().len(), 2);
    // empty place names are refused, unknown fields rejected
    let blank = spec("place.name", "Angora", "  ");
    assert_eq!(s.transact("x", |tx| apply(tx, &blank)).unwrap(), 0);
    assert!(preview(&s, &spec("person.sex", "M", "F")).is_err());
}
