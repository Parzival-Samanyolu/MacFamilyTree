use kintree_core::gedcom::{self, ExportOptions};
use kintree_core::store::Store;

const A: &[u8] = b"0 HEAD\n1 GEDC\n2 VERS 5.5.1\n0 @I1@ INDI\n1 NAME Ali /Kaya/\n1 FAMS @F1@\n0 @I2@ INDI\n1 NAME Ayse /Demir/\n1 FAMS @F1@\n0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n0 TRLR\n";
const B: &[u8] = b"0 HEAD\n1 GEDC\n2 VERS 5.5.1\n0 @I1@ INDI\n1 NAME Veli /Yildiz/\n1 FAMS @F1@\n0 @I2@ INDI\n1 NAME Zeynep /Yildiz/\n1 FAMS @F1@\n0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n0 TRLR\n";

#[test]
fn importing_a_second_file_keeps_exported_xrefs_unique_and_links_intact() {
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, A).unwrap();
    gedcom::import(&mut s, B).unwrap();
    assert_eq!(s.rows("person").unwrap().len(), 4);
    let out = String::from_utf8(gedcom::export(&s, &ExportOptions::default()).unwrap()).unwrap();
    let defs: Vec<&str> = out.lines().filter(|l| l.starts_with("0 @")).collect();
    let mut uniq = defs.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(
        defs.len(),
        uniq.len(),
        "duplicate record ids in export:\n{out}"
    );
    // re-import: still two separate couples
    let mut t = Store::open_memory().unwrap();
    gedcom::import(&mut t, out.as_bytes()).unwrap();
    assert_eq!(t.rows("person").unwrap().len(), 4);
    assert_eq!(t.rows("family").unwrap().len(), 2);
    for f in t.rows("family").unwrap() {
        let p1 = t
            .rows_where("person_name", "person_id", f["partner1"].as_str().unwrap())
            .unwrap();
        let p2 = t
            .rows_where("person_name", "person_id", f["partner2"].as_str().unwrap())
            .unwrap();
        assert_eq!(
            p1[0]["surname"] == "Kaya",
            p2[0]["surname"] == "Demir",
            "couples were crossed"
        );
    }
}
