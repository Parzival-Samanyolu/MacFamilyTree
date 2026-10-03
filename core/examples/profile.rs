use kintree_core::gedcom::{self, ExportOptions};
use kintree_core::store::Store;
use std::time::Instant;
fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20000);
    let text = kintree_core::synth::generate_gedcom(n, 42);
    let t = Instant::now();
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, text.as_bytes()).unwrap();
    eprintln!("import {:?}", t.elapsed());
    for tbl in [
        "person",
        "person_name",
        "event",
        "family",
        "family_child",
        "place",
        "raw_tag",
        "note",
        "citation",
    ] {
        let t = Instant::now();
        let r = s.rows(tbl).unwrap();
        eprintln!("rows({tbl}) {} -> {:?}", r.len(), t.elapsed());
    }
    let t = Instant::now();
    let o = gedcom::export(&s, &ExportOptions::default()).unwrap();
    eprintln!("export {:?} {}KB", t.elapsed(), o.len() / 1024);
    let t = Instant::now();
    let mut issues = vec![];
    let recs = gedcom::tree::parse(std::str::from_utf8(&o).unwrap(), &mut issues);
    eprintln!("parse only {:?} recs={}", t.elapsed(), recs.len());
}
