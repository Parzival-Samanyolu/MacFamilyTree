use kintree_core::gedcom;
use kintree_core::stats::compute;
use kintree_core::store::Store;

#[test]
fn stats_on_known_tree() {
    let ged = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n\
0 @I1@ INDI\n1 NAME Ali /Kaya/\n1 SEX M\n1 BIRT\n2 DATE 15 MAR 1800\n2 PLAC Konya\n1 DEAT\n2 DATE 15 MAR 1870\n1 OCCU Farmer\n1 FAMS @F1@\n1 SOUR @S1@\n\
0 @I2@ INDI\n1 NAME Ayşe /Kaya/\n1 SEX F\n1 BIRT\n2 DATE 1 JAN 1805\n1 DEAT\n2 DATE 1 JAN 1900\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Ali /Kaya/\n1 SEX M\n1 BIRT\n2 DATE 1 JUN 1830\n2 PLAC Konya\n1 OCCU Farmer\n1 FAMC @F1@\n\
0 @I4@ INDI\n1 NAME Zeynep /Demir/\n1 SEX F\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 CHIL @I4@\n1 MARR\n2 DATE 1825\n\
0 @S1@ SOUR\n1 TITL T\n0 TRLR\n";
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, ged.as_bytes()).unwrap();
    let st = compute(&s).unwrap();
    assert_eq!(
        (st.counts.persons, st.counts.families, st.counts.sources),
        (4, 1, 1)
    );
    let sex = |l: &str| st.sex.iter().find(|b| b.label == l).unwrap().count;
    assert_eq!((sex("M"), sex("F")), (2, 2));
    // ages at death: Ali 70 -> bin 70-79, Ayşe 95 -> 90-99
    assert_eq!(st.age_at_death[7].count, 1);
    assert_eq!(st.age_at_death[9].count, 1);
    assert_eq!(st.age_at_death[7].ids.len(), 1, "drill-down ids present");
    assert_eq!(st.lifespan_by_century.len(), 1);
    let (century, avg, n) = st.lifespan_by_century[0];
    assert_eq!((century, n), (1800, 2));
    assert!((avg - 82.5).abs() < 0.1, "{avg}");
    assert_eq!(st.children_per_family[2].count, 1);
    assert_eq!(st.top_given_names[0], ("Ali".into(), 2));
    assert_eq!(st.top_surnames[0], ("Kaya".into(), 3));
    assert_eq!(st.top_occupations[0], ("Farmer".into(), 2));
    assert_eq!(st.top_places[0], ("Konya".into(), 2));
    assert_eq!(st.generation_depth, 2);
    assert_eq!(st.birth_months[2].count, 1);
    assert_eq!(st.longest_lived[0].1.round(), 95.0);
    assert!(
        (st.avg_marriage_age.unwrap() - 22.5).abs() < 0.2,
        "{:?}",
        st.avg_marriage_age
    );
    assert!((st.source_coverage_pct - 25.0).abs() < 1e-9);
    assert!(st.avg_completeness > 30.0 && st.avg_completeness < 90.0);
    assert_eq!(st.largest_families[0].1, 2);
}

#[test]
fn stats_on_empty_project() {
    let s = Store::open_memory().unwrap();
    let st = compute(&s).unwrap();
    assert_eq!(st.counts.persons, 0);
    assert_eq!(st.generation_depth, 0);
    assert_eq!(st.source_coverage_pct, 0.0);
}
