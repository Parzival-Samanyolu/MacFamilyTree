use kintree_core::gedcom;
use kintree_core::quality::*;
use kintree_core::store::Store;

fn load(ged: &str) -> Store {
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, ged.as_bytes()).unwrap();
    s
}

const HDR: &str = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n";

fn rules_hit(s: &Store) -> Vec<String> {
    let mut v: Vec<String> = check_all(s, &Rules::default())
        .unwrap()
        .into_iter()
        .map(|f| f.rule)
        .collect();
    v.sort();
    v.dedup();
    v
}

#[test]
fn clean_tree_has_no_errors_or_warnings() {
    let s = load(&format!(
        "{HDR}0 @I1@ INDI\n1 NAME Dad /A/\n1 SEX M\n1 BIRT\n2 DATE 1 JAN 1900\n1 DEAT\n2 DATE 1 JAN 1970\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Mom /A/\n1 SEX F\n1 BIRT\n2 DATE 1 JAN 1902\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Kid /A/\n1 SEX M\n1 BIRT\n2 DATE 5 MAY 1930\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 MARR\n2 DATE 1925\n0 TRLR\n"
    ));
    let f = check_all(&s, &Rules::default()).unwrap();
    assert!(f.iter().all(|x| x.severity == Severity::Info), "{f:#?}");
}

#[test]
fn detects_chronology_problems() {
    let s = load(&format!(
        "{HDR}\
0 @I1@ INDI\n1 NAME Dead /First/\n1 SEX M\n1 BIRT\n2 DATE 1900\n1 DEAT\n2 DATE 1890\n\
0 @I2@ INDI\n1 NAME Old /Timer/\n1 SEX F\n1 BIRT\n2 DATE 1700\n1 DEAT\n2 DATE 1850\n1 RESI\n2 DATE 1860\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Young /Mother/\n1 SEX F\n1 BIRT\n2 DATE 1 JAN 1900\n1 DEAT\n2 DATE 1 JAN 1901\n1 FAMS @F2@\n\
0 @I4@ INDI\n1 NAME Late /Child/\n1 SEX M\n1 BIRT\n2 DATE 1 JAN 1910\n1 FAMC @F2@\n\
0 @I5@ INDI\n1 NAME Before /Parent/\n1 SEX M\n1 BIRT\n2 DATE 1850\n1 FAMC @F3@\n\
0 @I6@ INDI\n1 NAME Parent /Late/\n1 SEX M\n1 BIRT\n2 DATE 1900\n1 FAMS @F3@\n\
0 @I7@ INDI\n1 NAME Child /Mother/\n1 SEX F\n1 BIRT\n2 DATE 1 JAN 1900\n1 FAMS @F4@\n\
0 @I8@ INDI\n1 NAME Tiny /Kid/\n1 SEX M\n1 BIRT\n2 DATE 1 JAN 1905\n1 FAMC @F4@\n\
0 @F1@ FAM\n1 HUSB @I2@\n0 @F2@ FAM\n1 WIFE @I3@\n1 CHIL @I4@\n\
0 @F3@ FAM\n1 HUSB @I6@\n1 CHIL @I5@\n\
0 @F4@ FAM\n1 WIFE @I7@\n1 CHIL @I8@\n1 MARR\n2 DATE 1905\n\
0 TRLR\n"
    ));
    let hit = rules_hit(&s);
    for r in [
        "death_before_birth",
        "lifespan",
        "event_after_death",
        "born_after_parent_death",
        "child_before_parent",
        "parent_too_young",
        "marriage_too_young",
    ] {
        assert!(hit.contains(&r.to_string()), "missing {r}: {hit:?}");
    }
}

#[test]
fn detects_structural_problems_and_applies_fixes_with_undo() {
    let mut s = load(&format!(
        "{HDR}\
0 @I1@ INDI\n1 NAME Solo /Orphan/\n1 SEX U\n1 BIRT\n2 DATE 1900\n2 PLAC Konya\n1 BIRT\n2 DATE 1900\n2 PLAC Konya\n1 BIRT\n2 DATE 1901\n\
1 EVEN\n2 TYPE x\n2 DATE FROM 1900 TO 1890\n1 EVEN\n2 TYPE y\n2 DATE next full moon\n\
0 @F1@ FAM\n0 TRLR\n"
    ));
    let hit = rules_hit(&s);
    for r in [
        "orphan_person",
        "duplicate_event",
        "multiple_births",
        "invalid_date",
        "empty_family",
    ] {
        assert!(hit.contains(&r.to_string()), "missing {r}: {hit:?}");
    }
    let findings = check_all(&s, &Rules::default()).unwrap();
    assert!(findings
        .iter()
        .any(|f| f.rule == "invalid_date" && f.severity == Severity::Error));
    let dup = findings
        .iter()
        .find(|f| f.rule == "duplicate_event")
        .unwrap()
        .clone();
    let before = s.count("event").unwrap();
    s.transact("Fix", |tx| apply_fix(tx, dup.fix.as_ref().unwrap()))
        .unwrap();
    assert_eq!(s.count("event").unwrap(), before - 1);
    assert!(!check_all(&s, &Rules::default())
        .unwrap()
        .iter()
        .any(|f| f.rule == "duplicate_event"));
    s.undo().unwrap();
    assert_eq!(s.count("event").unwrap(), before);
    assert!(check_all(&s, &Rules::default())
        .unwrap()
        .iter()
        .any(|f| f.rule == "duplicate_event"));
}

#[test]
fn ignore_list_and_rule_toggles() {
    let mut s = load(&format!("{HDR}0 @I1@ INDI\n1 NAME A /B/\n0 TRLR\n"));
    let f = check_all(&s, &Rules::default()).unwrap();
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].rule, "orphan_person");
    ignore(&mut s, &f[0].id).unwrap();
    assert!(check_all(&s, &Rules::default()).unwrap().is_empty());
    let mut r = Rules::default();
    r.enabled.remove("orphan_person");
    let s2 = load(&format!("{HDR}0 @I1@ INDI\n1 NAME A /B/\n0 TRLR\n"));
    assert!(check_all(&s2, &r).unwrap().is_empty());
}

#[test]
fn detects_loops_and_dangling_references() {
    let mut s = load(&format!(
        "{HDR}0 @I1@ INDI\n1 NAME A /B/\n1 SEX M\n1 FAMS @F1@\n1 FAMC @F2@\n0 @I2@ INDI\n1 NAME C /D/\n1 SEX M\n1 FAMS @F2@\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 CHIL @I2@\n0 @F2@ FAM\n1 HUSB @I2@\n1 CHIL @I1@\n0 TRLR\n"
    ));
    assert!(rules_hit(&s).contains(&"ancestor_loop".to_string()));
    // Inject a dangling reference directly, as a corrupted file would.
    s.transact("corrupt", |tx| {
        let mut r = serde_json::Map::new();
        r.insert("id".into(), "dangling-1".into());
        r.insert("owner_type".into(), "person".into());
        r.insert("owner_id".into(), "no-such-person".into());
        r.insert("kind".into(), "BIRT".into());
        tx.put_row("event", r)?;
        Ok(())
    })
    .unwrap();
    assert!(rules_hit(&s).contains(&"dangling_reference".to_string()));
}

#[test]
fn sibling_spacing_allows_twins() {
    let s = load(&format!(
        "{HDR}0 @I1@ INDI\n1 NAME Mom /M/\n1 SEX F\n1 BIRT\n2 DATE 1880\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME A /M/\n1 BIRT\n2 DATE 1 JAN 1900\n1 FAMC @F1@\n\
0 @I3@ INDI\n1 NAME B /M/\n1 BIRT\n2 DATE 1 JAN 1900\n1 FAMC @F1@\n\
0 @I4@ INDI\n1 NAME C /M/\n1 BIRT\n2 DATE 1 APR 1900\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 WIFE @I1@\n1 CHIL @I2@\n1 CHIL @I3@\n1 CHIL @I4@\n0 TRLR\n"
    ));
    let f = check_all(&s, &Rules::default()).unwrap();
    let sp: Vec<_> = f.iter().filter(|x| x.rule == "sibling_spacing").collect();
    assert_eq!(sp.len(), 1, "{sp:#?}");
}
