use kintree_core::date::Locale;
use kintree_core::gedcom;
use kintree_core::store::Store;
use kintree_core::tabular::*;
use kintree_core::timeline::*;

fn load(body: &str) -> Store {
    let mut s = Store::open_memory().unwrap();
    gedcom::import(
        &mut s,
        format!("0 HEAD\n1 GEDC\n2 VERS 5.5.1\n{body}0 TRLR\n").as_bytes(),
    )
    .unwrap();
    s
}
fn pid(s: &Store, given: &str) -> String {
    s.rows("person_name")
        .unwrap()
        .into_iter()
        .find(|n| n["given"] == given)
        .unwrap()["person_id"]
        .as_str()
        .unwrap()
        .to_string()
}

const TREE: &str = "\
0 @I1@ INDI\n1 NAME Ali /Kaya/\n1 SEX M\n1 BIRT\n2 DATE 10 MAR 1800\n2 PLAC Konya\n1 DEAT\n2 DATE 11 MAR 1870\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Ayşe /Demir/\n1 SEX F\n1 BIRT\n2 DATE 1805\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Can /Kaya/\n1 SEX M\n1 BIRT\n2 DATE 15 JUN 1990\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 MARR\n2 DATE 12 JUN 1825\n";

#[test]
fn timeline_scopes_order_and_overlay() {
    let s = load(TREE);
    let all = timeline(&s, Scope::All, false, Locale::En).unwrap();
    assert!(all.windows(2).all(|w| w[0].sort <= w[1].sort));
    assert_eq!(all[0].text, "Birth: Ali Kaya (Konya)");
    assert!(all.iter().any(|e| e.kind == "MARR"
        && e.text == "Marriage: Ali Kaya & Ayşe Demir"
        && e.family_id.is_some()));
    let ali = timeline(&s, Scope::Person(&pid(&s, "Ali")), false, Locale::En).unwrap();
    assert_eq!(ali.len(), 3, "birth, marriage (as partner), death");
    let fam_id = s.rows("family").unwrap()[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        timeline(&s, Scope::Family(&fam_id), false, Locale::En)
            .unwrap()
            .len()
            >= 5
    );
    assert_eq!(
        timeline(&s, Scope::Surname("kaya"), false, Locale::En)
            .unwrap()
            .iter()
            .filter(|e| e.kind == "BIRT")
            .count(),
        2
    );
    let with = timeline(&s, Scope::All, true, Locale::En).unwrap();
    let h: Vec<&Entry> = with.iter().filter(|e| e.history).collect();
    assert!(
        h.iter().any(|e| e.text.contains("Waterloo")),
        "1815 lies within the tree's era"
    );
    assert!(
        !h.iter().any(|e| e.text.contains("French Revolution")),
        "1789 is before the first event (1800)"
    );
    assert!(h
        .iter()
        .all(|e| e.sort >= all[0].sort - 400 && e.sort <= all.last().unwrap().sort + 400));
    assert!(timeline(&s, Scope::All, false, Locale::En)
        .unwrap()
        .iter()
        .all(|e| !e.history));
}

#[test]
fn overlay_is_user_editable() {
    let mut s = load(TREE);
    s.transact("x", |tx| {
        let mut r = serde_json::Map::new();
        r.insert("id".into(), "history_events".into());
        r.insert(
            "value".into(),
            r#"[{"year":1850,"text":"My ancestors move"}]"#.into(),
        );
        tx.put_row("setting", r)?;
        Ok(())
    })
    .unwrap();
    let t = timeline(&s, Scope::All, true, Locale::En).unwrap();
    let h: Vec<&str> = t
        .iter()
        .filter(|e| e.history)
        .map(|e| e.text.as_str())
        .collect();
    assert_eq!(h, vec!["My ancestors move"]);
}

#[test]
fn lifespans_handle_living_unknown_and_surname_filter() {
    let s = load(TREE);
    let l = lifespans(&s, None, 2026, 110).unwrap();
    let ali = l.iter().find(|x| x.name == "Ali Kaya").unwrap();
    assert_eq!(
        (ali.start_year, ali.end_year, ali.estimated_end),
        (1800, 1870, false)
    );
    let can = l.iter().find(|x| x.name == "Can Kaya").unwrap();
    assert_eq!(
        (can.start_year, can.end_year, can.estimated_end),
        (1990, 2026, true),
        "living runs to the current year"
    );
    let ayse = l.iter().find(|x| x.name.starts_with("Ay")).unwrap();
    assert_eq!(
        (ayse.end_year, ayse.estimated_end),
        (1875, true),
        "unknown death is estimated and flagged"
    );
    assert_eq!(lifespans(&s, Some("KAYA"), 2026, 110).unwrap().len(), 2);
}

#[test]
fn calendar_month_lists_anniversaries_with_privacy_rules() {
    let s = load(TREE);
    let june = calendar_month(&s, 2026, 6, false, 2026, 110).unwrap();
    assert!(june
        .iter()
        .any(|e| e.kind == "BIRT" && e.text == "Can Kaya" && e.day == 15 && e.years == 36));
    assert!(
        !june.iter().any(|e| e.kind == "MARR"),
        "couple has a death event, so no anniversary by default"
    );
    let june_all = calendar_month(&s, 2026, 6, true, 2026, 110).unwrap();
    assert!(june_all
        .iter()
        .any(|e| e.kind == "MARR" && e.day == 12 && e.years == 201));
    let march = calendar_month(&s, 2026, 3, false, 2026, 110).unwrap();
    assert!(
        march.iter().any(|e| e.kind == "DEAT" && e.day == 11),
        "death anniversaries are always listed"
    );
    assert!(
        !march.iter().any(|e| e.kind == "BIRT"),
        "deceased birthdays hidden by default"
    );
}

#[test]
fn ical_is_valid_folded_and_escaped() {
    let s = load("0 @I1@ INDI\n1 NAME Ayşe, \"the; Great\" /Çağlayanoğlu-Yıldızhanzade-Karakaşoğulları/\n1 BIRT\n2 DATE 15 JUN 1990\n0 @I2@ INDI\n1 NAME Dead /Person/\n1 BIRT\n2 DATE 1 JAN 1900\n1 DEAT\n2 DATE 1 JAN 1950\n");
    let ics = kintree_core::timeline::ical(&s, false, 2026, 110).unwrap();
    assert!(ics.starts_with("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n"));
    assert!(ics.trim_end().ends_with("END:VCALENDAR"));
    assert_eq!(
        ics.matches("BEGIN:VEVENT").count(),
        1,
        "only the living person's birthday"
    );
    assert!(ics.contains("DTSTART;VALUE=DATE:19900615"));
    assert!(ics.contains("RRULE:FREQ=YEARLY"));
    for line in ics.split("\r\n") {
        assert!(
            line.len() <= 75,
            "unfolded line of {} octets: {line}",
            line.len()
        );
    }
    let unfolded = ics.replace("\r\n ", "");
    assert!(unfolded.contains(r#"Ayşe\, "the\; Great" "#), "{unfolded}");
    let all = kintree_core::timeline::ical(&s, true, 2026, 110).unwrap();
    assert_eq!(all.matches("BEGIN:VEVENT").count(), 2);
}

#[test]
fn csv_export_import_roundtrip_rebuilds_families() {
    let s = load(TREE);
    let csv = persons_csv(&s).unwrap();
    assert!(csv.starts_with("id,given,surname,sex,birth_date,birth_place,death_date,death_place,occupation,father_id,mother_id,partner_ids\r\n"));
    assert!(csv.contains("Can,Kaya,M,15 JUN 1990"));
    let mut s2 = Store::open_memory().unwrap();
    let rep = s2
        .transact_untracked(|tx| import_persons_csv(tx, &csv))
        .unwrap();
    assert_eq!((rep.persons, rep.families), (3, 1));
    assert!(rep.warnings.is_empty(), "{:?}", rep.warnings);
    let can = pid(&s2, "Can");
    let g = kintree_core::relationship::Graph::load(&s2).unwrap();
    assert_eq!(g.people[&can].parents.len(), 2);
    // birth data survives
    let again = persons_csv(&s2).unwrap();
    assert!(
        again.contains("Ali,Kaya,M,10 MAR 1800,Konya,11 MAR 1870"),
        "{again}"
    );
}

#[test]
fn csv_parser_handles_quotes_delimiters_and_boms() {
    let rows =
        parse_csv("\u{FEFF}a;b;c\r\n\"x;1\";\"he said \"\"hi\"\"\";3\r\n\"multi\nline\";,;z\n\n");
    assert_eq!(
        rows,
        vec![
            vec!["a", "b", "c"],
            vec!["x;1", "he said \"hi\"", "3"],
            vec!["multi\nline", ",", "z"]
        ]
    );
    assert_eq!(parse_csv("a,b\n1,2"), vec![vec!["a", "b"], vec!["1", "2"]]);
    assert!(parse_csv("").is_empty());
}

#[test]
fn csv_import_accepts_turkish_headers_and_reports_problems() {
    let mut s = Store::open_memory().unwrap();
    let csv = "Ad;Soyad;Cinsiyet;Doğum Tarihi;Doğum Yeri;Meslek\nAyşe;Yılmaz;K;3 Mart 1950;Konya;Öğretmen\n;;;;;\nMehmet;Kaya;E;sometime;Ankara;\n";
    let rep = s
        .transact_untracked(|tx| import_persons_csv(tx, csv))
        .unwrap();
    assert_eq!(rep.persons, 2);
    assert!(rep.warnings.iter().any(|w| w.contains("no name")));
    assert!(rep.warnings.iter().any(|w| w.contains("kept as text")));
    let facts = kintree_core::facts::Facts::load(&s).unwrap();
    let a = facts.persons.values().find(|p| p.given == "Ayşe").unwrap();
    assert_eq!(a.sex, kintree_core::relationship::Sex::Female);
    assert_eq!(
        facts
            .birth(&a.id)
            .unwrap()
            .date
            .as_ref()
            .unwrap()
            .to_gedcom(),
        "3 MAR 1950"
    );
    let bad = s
        .transact_untracked(|tx| import_persons_csv(tx, "foo,bar\n1,2\n"))
        .unwrap();
    assert_eq!(bad.persons, 0);
    assert!(bad.warnings[0].contains("no given/surname"));
}

#[test]
fn csv_export_neutralises_spreadsheet_formulas() {
    let s = load("0 @I1@ INDI\n1 NAME =HYPERLINK(\"http://evil\") /+cmd|calc/\n");
    let csv = persons_csv(&s).unwrap();
    assert!(!csv.contains(",=HYPERLINK"), "{csv}");
    assert!(csv.contains("'=HYPERLINK"));
}

#[test]
fn json_export_contains_every_table_and_no_internal_columns() {
    let s = load(TREE);
    let j = project_json(&s).unwrap();
    assert_eq!(j["format"], "kintree-json");
    assert_eq!(j["tables"]["person"].as_array().unwrap().len(), 3);
    assert_eq!(j["tables"]["family"].as_array().unwrap().len(), 1);
    assert!(j["tables"]["person"][0].get("_rowid").is_none());
    assert!(j["tables"]["event"].as_array().unwrap().len() >= 5);
    assert!(j["tables"].get("history").is_none());
}
