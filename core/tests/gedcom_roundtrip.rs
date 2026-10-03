use kintree_core::gedcom::{self, charset, tree, Charset, ExportOptions, LivingPolicy, Version};
use kintree_core::store::Store;
use std::path::PathBuf;

fn corpus(dir: &str) -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../samples/gedcom")
        .join(dir);
    let mut v: Vec<PathBuf> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().map(|e| e == "ged").unwrap_or(false))
        .collect();
    v.sort();
    v
}

fn export_text(s: &Store, o: &ExportOptions) -> String {
    String::from_utf8(gedcom::export(s, o).unwrap()).unwrap()
}

/// Multiset of `PATH=value` entries, ignoring xref spelling, header/submitter/trailer and record order.
fn flatten(text: &str) -> Vec<String> {
    let mut issues = vec![];
    let recs = tree::parse(text, &mut issues);
    let mut out = vec![];
    fn walk(n: &tree::Node, path: &str, out: &mut Vec<String>) {
        let p = if path.is_empty() {
            n.tag.clone()
        } else {
            format!("{}.{}", path, n.tag)
        };
        let v = if n.is_pointer() {
            "@@".to_string()
        } else {
            n.value.clone()
        };
        out.push(format!("{}={}", p, v));
        for c in &n.children {
            walk(c, &p, out);
        }
    }
    for r in recs
        .iter()
        .filter(|r| !matches!(r.tag.as_str(), "HEAD" | "TRLR" | "SUBM"))
    {
        walk(r, "", &mut out);
    }
    out.sort();
    out
}

fn decode(bytes: &[u8]) -> String {
    charset::decode(bytes, &mut |_| {}).0
}

#[test]
fn clean_corpus_is_lossless() {
    let files = corpus("clean");
    assert!(
        files.len() >= 15,
        "corpus must have at least 15 clean files, found {}",
        files.len()
    );
    for f in files {
        let bytes = std::fs::read(&f).unwrap();
        let mut s = Store::open_memory().unwrap();
        let rep = gedcom::import(&mut s, &bytes).unwrap();
        let opts = ExportOptions {
            version: if rep.version.starts_with('7') {
                Version::V70
            } else {
                Version::V551
            },
            ..Default::default()
        };
        let out1 = export_text(&s, &opts);
        let src = flatten(&decode(&bytes));
        let got = flatten(&out1);
        assert_eq!(
            src,
            got,
            "tag-level mismatch for {} (issues: {:?})",
            f.display(),
            rep.issues
        );

        // import → export → import produces an identical graph (byte-identical re-export).
        let mut s2 = Store::open_memory().unwrap();
        gedcom::import(&mut s2, out1.as_bytes()).unwrap();
        let out2 = export_text(&s2, &opts);
        assert_eq!(out1, out2, "re-export differs for {}", f.display());
        for t in [
            "person", "family", "event", "source", "note", "media", "place", "raw_tag",
        ] {
            assert_eq!(
                s.count(t).unwrap(),
                s2.count(t).unwrap(),
                "{} count differs for {}",
                t,
                f.display()
            );
        }
    }
}

#[test]
fn torture_corpus_survives_and_is_idempotent() {
    for f in corpus("torture") {
        let bytes = std::fs::read(&f).unwrap();
        let mut s = Store::open_memory().unwrap();
        let rep = gedcom::import(&mut s, &bytes).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let _ = rep;
        let out1 = export_text(&s, &ExportOptions::default());
        let mut s2 = Store::open_memory().unwrap();
        gedcom::import(&mut s2, out1.as_bytes()).unwrap();
        let out2 = export_text(&s2, &ExportOptions::default());
        assert_eq!(out1, out2, "not idempotent: {}", f.display());
    }
}

#[test]
fn malformed_file_reports_detailed_issues() {
    let bytes = std::fs::read(corpus("torture")[0].clone()).unwrap();
    let mut s = Store::open_memory().unwrap();
    let rep = gedcom::import(&mut s, &bytes).unwrap();
    let msgs: Vec<&str> = rep.issues.iter().map(|i| i.message.as_str()).collect();
    for needle in [
        "malformed line",
        "level jumps",
        "duplicate xref",
        "dangling pointer",
        "unparseable date",
        "link added",
    ] {
        assert!(
            msgs.iter().any(|m| m.contains(needle)),
            "missing issue containing {needle:?}: {msgs:#?}"
        );
    }
    assert!(rep
        .issues
        .iter()
        .all(|i| i.line > 0 || i.severity != gedcom::Severity::Error));
}

#[test]
fn charsets_decode_to_same_content() {
    let get = |name: &str| {
        let p = corpus("clean")
            .into_iter()
            .find(|p| p.file_name().unwrap().to_str().unwrap().starts_with(name))
            .unwrap();
        let mut s = Store::open_memory().unwrap();
        let rep = gedcom::import(&mut s, &std::fs::read(p).unwrap()).unwrap();
        (
            rep.charset.unwrap(),
            export_text(&s, &ExportOptions::default()),
        )
    };
    let (cs, t) = get("04_");
    assert_eq!(cs, Charset::Utf16Le);
    assert!(t.contains("Zoë /Müller/"));
    let (cs, t) = get("05_");
    assert_eq!(cs, Charset::Latin1);
    assert!(t.contains("Jörg /Müller/") && t.contains("Köln"));
    let (cs, t) = get("06_");
    assert_eq!(cs, Charset::Ansel);
    assert!(t.contains("José /Müller/") && t.contains("Kraków"));
}

#[test]
fn gedcom7_export_has_no_char_and_native_calendar_names() {
    let p = corpus("clean")
        .into_iter()
        .find(|p| p.to_str().unwrap().contains("13_dates"))
        .unwrap();
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, &std::fs::read(p).unwrap()).unwrap();
    let o = ExportOptions {
        version: Version::V70,
        ..Default::default()
    };
    let t = export_text(&s, &o);
    assert!(t.contains("2 VERS 7.0") && !t.contains("1 CHAR"));
    assert!(
        t.contains("DATE JULIAN 5 OCT 1582")
            && t.contains("DATE FRENCH_R 18 BRUM 8")
            && !t.contains("@#D")
    );
    // 7.0 output must re-import to the same dates.
    let mut s2 = Store::open_memory().unwrap();
    gedcom::import(&mut s2, t.as_bytes()).unwrap();
    assert_eq!(
        flatten(&export_text(&s2, &ExportOptions::default())),
        flatten(&export_text(&s, &ExportOptions::default()))
    );
}

#[test]
fn utf16_export_roundtrips() {
    let p = corpus("clean")
        .into_iter()
        .find(|p| p.to_str().unwrap().contains("03_unicode"))
        .unwrap();
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, &std::fs::read(p).unwrap()).unwrap();
    let bytes = gedcom::export(
        &s,
        &ExportOptions {
            charset: Charset::Utf16Le,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
    let mut s2 = Store::open_memory().unwrap();
    gedcom::import(&mut s2, &bytes).unwrap();
    assert_eq!(
        flatten(&export_text(&s, &ExportOptions::default())),
        flatten(&export_text(&s2, &ExportOptions::default()))
    );
}

#[test]
fn living_privacy_mask_and_exclude() {
    let src = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n\
0 @I1@ INDI\n1 NAME Old /Timer/\n1 SEX M\n1 BIRT\n2 DATE 1800\n1 DEAT Y\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Young /Person/\n1 SEX F\n1 BIRT\n2 DATE 1990\n2 PLAC Secret City\n1 NOTE private note\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Kid /Person/\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 MARR\n2 DATE 2010\n0 TRLR\n";
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, src.as_bytes()).unwrap();
    let all = export_text(&s, &ExportOptions::default());
    assert!(all.contains("Young /Person/") && all.contains("Secret City"));

    let masked = export_text(
        &s,
        &ExportOptions {
            living: LivingPolicy::Mask,
            current_year: 2026,
            ..Default::default()
        },
    );
    assert!(masked.contains("Old /Timer/"));
    assert!(
        !masked.contains("Young")
            && !masked.contains("Secret City")
            && !masked.contains("private note")
            && !masked.contains("Kid")
    );
    assert!(masked.contains("NAME Living"));
    assert!(
        !masked.contains("2010"),
        "marriage with living partner must be hidden"
    );

    let excl = export_text(
        &s,
        &ExportOptions {
            living: LivingPolicy::Exclude,
            current_year: 2026,
            ..Default::default()
        },
    );
    assert!(
        excl.contains("Old /Timer/")
            && !excl.contains("Living")
            && !excl.contains("Young")
            && !excl.contains("Kid")
    );
    // no dangling pointers in the filtered output
    let mut s2 = Store::open_memory().unwrap();
    let rep = gedcom::import(&mut s2, excl.as_bytes()).unwrap();
    assert!(
        !rep.issues.iter().any(|i| i.message.contains("dangling")),
        "{:?}",
        rep.issues
    );
}
