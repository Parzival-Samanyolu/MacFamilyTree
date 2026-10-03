//! Benchmarks recorded in docs/PERFORMANCE.md. Run: cargo test -p kintree-core --release --test bench -- --ignored --nocapture
use kintree_core::gedcom::{self, ExportOptions};
use kintree_core::store::Store;
use std::time::Instant;

fn run(n: usize) {
    let text = kintree_core::synth::generate_gedcom(n, 42);
    let dir = std::env::temp_dir().join(format!("kt-bench-{}", kintree_core::store::new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("p.db");
    let t = Instant::now();
    let mut s = Store::open(&path).unwrap();
    let rep = gedcom::import(&mut s, text.as_bytes()).unwrap();
    let import = t.elapsed();
    drop(s);
    let t = Instant::now();
    let s = Store::open(&path).unwrap();
    let open = t.elapsed();
    let t = Instant::now();
    let hits = s.search_persons("yilmaz ali", 50).unwrap();
    let search = t.elapsed();
    let t = Instant::now();
    let out = gedcom::export(&s, &ExportOptions::default()).unwrap();
    let export = t.elapsed();
    println!("persons={} families={} events={} gedcom={}KB | import {:?} | open {:?} | search({} hits) {:?} | export {:?} ({}KB)", rep.persons, rep.families, rep.events, text.len() / 1024, import, open, hits.len(), search, export, out.len() / 1024);
}

#[test]
#[ignore]
fn bench_sizes() {
    run(30);
    run(2_000);
    run(100_000);
}

#[test]
fn small_synth_roundtrips() {
    let text = kintree_core::synth::generate_gedcom(300, 7);
    let mut s = Store::open_memory().unwrap();
    let rep = gedcom::import(&mut s, text.as_bytes()).unwrap();
    assert!(rep.persons >= 290);
    assert!(
        rep.issues
            .iter()
            .all(|i| i.severity != gedcom::Severity::Error),
        "{:?}",
        rep.issues
    );
    let o1 = String::from_utf8(gedcom::export(&s, &ExportOptions::default()).unwrap()).unwrap();
    let mut s2 = Store::open_memory().unwrap();
    gedcom::import(&mut s2, o1.as_bytes()).unwrap();
    assert_eq!(
        o1,
        String::from_utf8(gedcom::export(&s2, &ExportOptions::default()).unwrap()).unwrap()
    );
}
