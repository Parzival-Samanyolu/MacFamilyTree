use kintree_core::gedcom::{self, ExportOptions};
use kintree_core::media;
use kintree_core::pkg::*;
use kintree_core::report::{Options, Privacy, ReportLang};
use kintree_core::site;
use kintree_core::store::Store;

const TREE: &[u8] = b"0 HEAD\n1 GEDC\n2 VERS 5.5.1\n\
0 @I1@ INDI\n1 NAME Ali /Kaya/\n1 SEX M\n1 BIRT\n2 DATE 3 MAR 1850\n2 PLAC Konya\n1 DEAT\n2 DATE 1920\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Ay\xc5\x9fe /Demir <b>/\n1 SEX F\n1 BIRT\n2 DATE 1855\n1 DEAT\n2 DATE 1930\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Zeynep /Kaya/\n1 SEX F\n1 BIRT\n2 DATE 2001\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n0 TRLR\n";

fn project() -> Store {
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, TREE).unwrap();
    s
}
fn png(shade: u8) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(8, 8, image::Rgb([shade, 9, 9]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}
fn pid(s: &Store, g: &str) -> String {
    s.rows("person_name")
        .unwrap()
        .into_iter()
        .find(|n| n["given"] == g)
        .unwrap()["person_id"]
        .as_str()
        .unwrap()
        .to_string()
}
fn text(files: &[(String, Vec<u8>)], name: &str) -> String {
    String::from_utf8(
        files
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| {
                panic!(
                    "missing {name}: {:?}",
                    files.iter().map(|f| &f.0).collect::<Vec<_>>()
                )
            })
            .1
            .clone(),
    )
    .unwrap()
}

#[test]
fn zip_roundtrip_skips_directories_duplicates_and_unsafe_paths() {
    let z = build_zip(&[
        ("a.txt".into(), b"one".to_vec()),
        ("a.txt".into(), b"dup".to_vec()),
        ("d/b.txt".into(), b"two".to_vec()),
    ])
    .unwrap();
    let r = read_zip(&z).unwrap();
    assert_eq!(
        r,
        vec![
            ("a.txt".to_string(), b"one".to_vec()),
            ("d/b.txt".to_string(), b"two".to_vec())
        ]
    );
    let evil = build_zip(&[
        ("../evil.txt".into(), b"x".to_vec()),
        ("/abs.txt".into(), b"x".to_vec()),
        ("ok.txt".into(), b"y".to_vec()),
    ])
    .unwrap();
    assert_eq!(read_zip(&evil).unwrap().len(), 1);
    assert!(read_zip(b"not a zip").is_err());
}

#[test]
fn gedzip_roundtrip_reattaches_media_by_file_name() {
    let mut s = project();
    let ali = pid(&s, "Ali");
    let img = png(5);
    media::import(&mut s, "photos/ali.png", &img, Some(("person", &ali))).unwrap();
    let z = export_gedzip(&s, &ExportOptions::default()).unwrap();
    let names: Vec<String> = read_zip(&z).unwrap().into_iter().map(|f| f.0).collect();
    assert!(
        names.contains(&"gedcom.ged".to_string()) && names.contains(&"ali.png".to_string()),
        "{names:?}"
    );

    let (ged, files) = split_gedzip(&z).unwrap();
    let mut t = Store::open_memory().unwrap();
    gedcom::import(&mut t, &ged).unwrap();
    let before = media::list(
        &t,
        &media::Filter {
            missing: true,
            ..Default::default()
        },
        kintree_core::date::Locale::En,
    )
    .unwrap();
    assert_eq!(before.len(), 1, "imported media starts without bytes");
    let rep = attach_media(&mut t, &files).unwrap();
    assert_eq!((rep.media_attached, rep.media_unmatched), (1, 0));
    let id = t.rows("media").unwrap()[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(media::file(&t, &id).unwrap().unwrap().2, img);
    assert!(split_gedzip(&build_zip(&[("x.txt".into(), b"1".to_vec())]).unwrap()).is_err());
}

#[test]
fn website_has_index_person_pages_links_search_and_escaping() {
    let mut s = project();
    let ali = pid(&s, "Ali");
    media::import(&mut s, "ali.png", &png(1), Some(("person", &ali))).unwrap();
    let files = site::build(&s, "Kaya <Family>", &Options::default()).unwrap();
    let names: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
    assert!(
        names.contains(&"index.html")
            && names.contains(&"style.css")
            && names.contains(&"people/p1.html"),
        "{names:?}"
    );
    assert!(names
        .iter()
        .any(|n| n.starts_with("media/") && n.ends_with(".jpg")));
    let idx = text(&files, "index.html");
    assert!(
        idx.contains("Kaya &lt;Family&gt;") && idx.contains("3 people") && idx.contains("id=\"q\"")
    );
    assert!(idx.contains("href=\"people/p1.html\""));
    assert!(!idx.contains("<b>"), "names are escaped");
    // Every person page links back to the index and its stylesheet, and its relatives resolve to existing pages.
    for (n, d) in files.iter().filter(|f| f.0.starts_with("people/")) {
        let h = String::from_utf8(d.clone()).unwrap();
        assert!(h.contains("href=\"../style.css\"") && h.contains("href=\"../index.html\""));
        for l in h
            .split("href=\"")
            .skip(1)
            .map(|x| x.split('"').next().unwrap())
        {
            if l.starts_with("p") && l.ends_with(".html") {
                assert!(
                    names.contains(&format!("people/{l}").as_str()),
                    "{n} links to missing {l}"
                );
            }
        }
    }
    let ali_page = files
        .iter()
        .filter(|f| f.0.starts_with("people/"))
        .map(|f| String::from_utf8(f.1.clone()).unwrap())
        .find(|h| h.contains("Ali Kaya was born"))
        .unwrap();
    assert!(
        ali_page.contains("Children")
            && ali_page.contains("Zeynep Kaya")
            && ali_page.contains("class=\"gallery\"")
    );
}

#[test]
fn website_privacy_excludes_or_masks_living_people() {
    let s = project();
    let ex = site::build(
        &s,
        "T",
        &Options {
            privacy: Privacy::Exclude,
            ..Default::default()
        },
    )
    .unwrap();
    let all: String = ex
        .iter()
        .map(|f| String::from_utf8_lossy(&f.1).to_string())
        .collect();
    assert!(!all.contains("Zeynep") && !all.contains("2001"), "excluded");
    assert_eq!(ex.iter().filter(|f| f.0.starts_with("people/")).count(), 2);
    let mk = site::build(
        &s,
        "T",
        &Options {
            privacy: Privacy::Mask,
            lang: ReportLang::Tr,
            ..Default::default()
        },
    )
    .unwrap();
    let all: String = mk
        .iter()
        .map(|f| String::from_utf8_lossy(&f.1).to_string())
        .collect();
    assert!(
        !all.contains("Zeynep") && !all.contains("2001") && all.contains("Yaşıyor"),
        "masked"
    );
    assert!(text(&mk, "index.html").contains("lang=\"tr\""));
}
