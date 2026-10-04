use kintree_core::gedcom;
use kintree_core::query::*;
use kintree_core::store::Store;

const DATA: &str = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n\
0 @I1@ INDI\n1 NAME Ali /Kaya/\n1 SEX M\n1 BIRT\n2 DATE 1850\n2 PLAC Konya, Turkey\n1 OCCU Weaver\n1 DEAT\n2 DATE 1920\n2 PLAC Ankara, Turkey\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Ayse /Demir/\n1 SEX F\n1 BIRT\n2 DATE 1855\n1 DEAT\n2 DATE 1930\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Can /Kaja/\n1 SEX M\n1 BIRT\n2 DATE 1880\n2 PLAC Konya, Turkey\n1 OCCU Teacher\n2 SOUR @S1@\n1 FAMC @F1@\n1 DEAT\n2 DATE 1950\n\
0 @I4@ INDI\n1 NAME Zeynep /Yildiz/\n1 SEX F\n1 BIRT\n2 DATE 2002\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n\
0 @S1@ SOUR\n1 TITL Register\n0 TRLR\n";

fn store() -> Store {
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, DATA.as_bytes()).unwrap();
    s
}
fn names(s: &Store, c: &Criteria) -> Vec<String> {
    run(s, c, 110, 2026)
        .unwrap()
        .into_iter()
        .map(|id| {
            s.rows_where("person_name", "person_id", &id).unwrap()[0]["given"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect()
}
fn crit(f: impl FnOnce(&mut Criteria)) -> Criteria {
    let mut c = Criteria::default();
    f(&mut c);
    c
}

#[test]
fn empty_criteria_list_everyone_sorted_by_surname() {
    assert_eq!(
        names(&store(), &Criteria::default()),
        ["Ayse", "Can", "Ali", "Zeynep"]
    );
}

#[test]
fn name_place_and_year_filters_combine() {
    let s = store();
    assert_eq!(names(&s, &crit(|c| c.surname = "kaya".into())), ["Ali"]);
    assert_eq!(
        names(&s, &crit(|c| c.given = "AYŞE".into())),
        ["Ayse"],
        "diacritics and case are ignored"
    );
    assert_eq!(
        names(&s, &crit(|c| c.place = "konya".into())),
        ["Can", "Ali"]
    );
    assert_eq!(
        names(
            &s,
            &crit(|c| (c.born_from, c.born_to) = (Some(1851), Some(1900)))
        ),
        ["Ayse", "Can"]
    );
    assert_eq!(names(&s, &crit(|c| c.died_from = Some(1940))), ["Can"]);
    assert_eq!(
        names(
            &s,
            &crit(|c| (c.place, c.born_to) = ("Konya".into(), Some(1860)))
        ),
        ["Ali"]
    );
    assert!(names(&s, &crit(|c| c.born_from = Some(3000))).is_empty());
}

#[test]
fn phonetic_surname_matching_finds_spelling_variants() {
    let s = store();
    assert_eq!(
        names(
            &s,
            &crit(|c| (c.surname, c.phonetic) = ("Kaia".into(), true))
        ),
        ["Can", "Ali"]
    );
    assert!(names(&s, &crit(|c| c.surname = "Kaia".into())).is_empty());
}

#[test]
fn sex_status_events_and_flags() {
    let s = store();
    assert_eq!(names(&s, &crit(|c| c.sex = "F".into())), ["Ayse", "Zeynep"]);
    assert_eq!(names(&s, &crit(|c| c.status = "living".into())), ["Zeynep"]);
    assert_eq!(
        names(&s, &crit(|c| c.status = "deceased".into())),
        ["Ayse", "Can", "Ali"]
    );
    assert_eq!(
        names(&s, &crit(|c| c.event_kind = "OCCU".into())),
        ["Can", "Ali"]
    );
    assert_eq!(
        names(
            &s,
            &crit(|c| (c.event_kind, c.event_text) = ("OCCU".into(), "teach".into()))
        ),
        ["Can"]
    );
    assert_eq!(names(&s, &crit(|c| c.has_sources = Some(true))), ["Can"]);
    assert_eq!(names(&s, &crit(|c| c.has_parents = Some(true))), ["Can"]);
    assert_eq!(
        names(&s, &crit(|c| c.has_children = Some(true))),
        ["Ayse", "Ali"]
    );
    assert_eq!(
        names(&s, &crit(|c| c.has_media = Some(true))),
        Vec::<String>::new()
    );
    assert_eq!(names(&s, &crit(|c| c.bookmarked = Some(false))).len(), 4);
}

#[test]
fn saved_searches_are_stored_replaced_and_deleted() {
    let mut s = store();
    assert!(saved(&s).unwrap().is_empty());
    let c1 = crit(|c| c.place = "Konya".into());
    save(&mut s, "Konya folks", &c1).unwrap();
    save(&mut s, "Alive", &crit(|c| c.status = "living".into())).unwrap();
    save(&mut s, "Konya folks", &crit(|c| c.place = "Ankara".into())).unwrap();
    let list = saved(&s).unwrap();
    assert_eq!(
        list.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
        ["Alive", "Konya folks"]
    );
    assert_eq!(list[1].criteria.place, "Ankara");
    delete(&mut s, "Alive").unwrap();
    assert_eq!(saved(&s).unwrap().len(), 1);
    s.undo().unwrap();
    assert_eq!(saved(&s).unwrap().len(), 2);
    let json = serde_json::to_string(&c1).unwrap();
    assert_eq!(serde_json::from_str::<Criteria>(&json).unwrap(), c1);
    assert_eq!(
        serde_json::from_str::<Criteria>("{}").unwrap(),
        Criteria::default()
    );
}
