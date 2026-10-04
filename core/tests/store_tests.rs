use kintree_core::date::GenDate;
use kintree_core::name::PersonName;
use kintree_core::store::Store;

fn d(s: &str) -> GenDate {
    GenDate::parse(s).unwrap()
}

#[test]
fn migrations_apply_and_are_idempotent() {
    let dir = std::env::temp_dir().join(format!("kt-{}", kintree_core::store::new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("p.db");
    {
        let s = Store::open(&p).unwrap();
        assert_eq!(s.schema_version(), 2);
    }
    let s = Store::open(&p).unwrap();
    assert_eq!(s.schema_version(), 2);
}

#[test]
fn undo_redo_roundtrip_and_redo_invalidation() {
    let mut s = Store::open_memory().unwrap();
    let id = s
        .transact("Add person", |tx| {
            tx.create_person(&PersonName::new("Ayşe", "Yılmaz"), "F")
        })
        .unwrap();
    assert_eq!(s.count("person").unwrap(), 1);
    assert_eq!(s.search_persons("ayse yil", 10).unwrap(), vec![id.clone()]);

    assert_eq!(s.undo().unwrap().as_deref(), Some("Add person"));
    assert_eq!(s.count("person").unwrap(), 0);
    assert_eq!(s.count("person_name").unwrap(), 0);
    assert!(s.search_persons("ayse", 10).unwrap().is_empty());

    assert!(s.can_redo());
    s.redo().unwrap();
    assert_eq!(s.count("person").unwrap(), 1);
    assert_eq!(s.search_persons("YILMAZ", 10).unwrap(), vec![id]);

    s.undo().unwrap();
    s.transact("Other", |tx| {
        tx.create_person(&PersonName::new("B", "C"), "M")
    })
    .unwrap();
    assert!(!s.can_redo(), "new command clears redo stack");
}

#[test]
fn groups_undo_as_one_step() {
    let mut s = Store::open_memory().unwrap();
    s.begin_group().unwrap();
    for n in ["A", "B", "C"] {
        s.transact("Add", |tx| tx.create_person(&PersonName::new(n, "X"), "U"))
            .unwrap();
    }
    s.end_group();
    s.transact("Solo", |tx| {
        tx.create_person(&PersonName::new("D", "X"), "U")
    })
    .unwrap();
    assert_eq!(s.count("person").unwrap(), 4);
    s.undo().unwrap();
    assert_eq!(s.count("person").unwrap(), 3);
    s.undo().unwrap();
    assert_eq!(s.count("person").unwrap(), 0);
    s.redo().unwrap();
    assert_eq!(s.count("person").unwrap(), 3);
}

#[test]
fn failed_transaction_rolls_back() {
    let mut s = Store::open_memory().unwrap();
    let r: Result<(), _> = s.transact("bad", |tx| {
        tx.create_person(&PersonName::new("A", "B"), "M")?;
        tx.delete("nonexistent_table", "x")?;
        Ok(())
    });
    assert!(r.is_err());
    assert_eq!(s.count("person").unwrap(), 0);
    assert!(!s.can_undo());
}

#[test]
fn delete_person_cascades_and_undo_restores_everything() {
    let mut s = Store::open_memory().unwrap();
    let (a, b, fam) = s
        .transact("setup", |tx| {
            let a = tx.create_person(&PersonName::new("Ali", "Kaya"), "M")?;
            let b = tx.create_person(&PersonName::new("Zeynep", "Kaya"), "F")?;
            let kid = tx.create_person(&PersonName::new("Can", "Kaya"), "M")?;
            let fam = tx.create_family(Some(&a), Some(&b), "married")?;
            tx.add_child(&fam, &kid, "biological")?;
            tx.add_event(
                "person",
                &a,
                "BIRT",
                Some(&GenDate::parse("3 MAR 1850").unwrap()),
                None,
            )?;
            Ok((a, b, fam))
        })
        .unwrap();
    assert_eq!(s.count("event").unwrap(), 1);
    s.transact("Delete", |tx| tx.delete_person(&a)).unwrap();
    assert_eq!(s.count("person").unwrap(), 2);
    assert_eq!(s.count("event").unwrap(), 0);
    let p1: Option<String> = s
        .conn()
        .query_row("SELECT partner1 FROM family WHERE id=?1", [&fam], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(p1, None);
    assert!(s.search_persons("ali", 5).unwrap().is_empty());

    s.undo().unwrap();
    assert_eq!(s.count("person").unwrap(), 3);
    assert_eq!(s.count("event").unwrap(), 1);
    let p1: Option<String> = s
        .conn()
        .query_row("SELECT partner1 FROM family WHERE id=?1", [&fam], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(p1.as_deref(), Some(a.as_str()));
    assert_eq!(s.search_persons("ali", 5).unwrap(), vec![a]);
    let _ = b;
}

#[test]
fn event_dates_are_indexed_by_sort_key() {
    let mut s = Store::open_memory().unwrap();
    s.transact("evs", |tx| {
        let p = tx.create_person(&PersonName::new("A", "B"), "M")?;
        for dt in ["1900", "BEF 1850", "3 MAR 1850", "@#DJULIAN@ 1 JAN 1700"] {
            tx.add_event("person", &p, "RESI", Some(&d(dt)), None)?;
        }
        Ok(())
    })
    .unwrap();
    let mut st = s
        .conn()
        .prepare("SELECT date_json FROM event ORDER BY date_sort")
        .unwrap();
    let order: Vec<String> = st
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(|j| {
            serde_json::from_str::<GenDate>(&j.unwrap())
                .unwrap()
                .to_gedcom()
        })
        .collect();
    assert_eq!(
        order,
        ["@#DJULIAN@ 1 JAN 1700", "BEF 1850", "3 MAR 1850", "1900"]
    );
}

#[test]
fn search_is_turkish_and_diacritic_insensitive() {
    let mut s = Store::open_memory().unwrap();
    let id = s
        .transact("add", |tx| {
            tx.create_person(&PersonName::new("İsmail Çağrı", "Işık"), "M")
        })
        .unwrap();
    for q in ["ismail", "cagri", "ISIK", "çağ"] {
        assert_eq!(
            s.search_persons(q, 5).unwrap(),
            vec![id.clone()],
            "query {q}"
        );
    }
}
