use kintree_core::gedcom;
use kintree_core::report::*;
use kintree_core::store::Store;

const HDR: &str = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n";

fn load(body: &str) -> Store {
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, format!("{HDR}{body}0 TRLR\n").as_bytes()).unwrap();
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

const FAMILY: &str = "\
0 @I1@ INDI\n1 NAME John /Smith/\n1 SEX M\n1 BIRT\n2 DATE 3 MAR 1850\n2 PLAC Konya, Turkey\n2 SOUR @S1@\n3 PAGE p. 12\n1 OCCU Farmer\n1 DEAT\n2 DATE 1 JAN 1920\n2 PLAC Ankara, Turkey\n1 BURI\n2 PLAC Ankara, Turkey\n1 FAMS @F1@\n1 FAMC @F0@\n\
0 @I2@ INDI\n1 NAME Mary /Jones/\n1 SEX F\n1 BIRT\n2 DATE ABT 1855\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME William /Smith/\n1 SEX M\n1 BIRT\n2 DATE 1875\n1 FAMC @F1@\n\
0 @I4@ INDI\n1 NAME Anna /Smith/\n1 SEX F\n1 BIRT\n2 DATE 1878\n1 FAMC @F1@\n\
0 @I5@ INDI\n1 NAME Old /Smith/\n1 SEX M\n1 FAMS @F0@\n\
0 @I6@ INDI\n1 NAME Olga /Brown/\n1 SEX F\n1 FAMS @F0@\n\
0 @F0@ FAM\n1 HUSB @I5@\n1 WIFE @I6@\n1 CHIL @I1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 CHIL @I4@\n1 MARR\n2 DATE 12 JUN 1874\n2 PLAC Konya, Turkey\n\
0 @S1@ SOUR\n1 TITL Parish Register\n1 AUTH Priest Ahmet\n1 PUBL Konya, 1850\n";

#[test]
fn english_narrative_is_exact() {
    let s = load(FAMILY);
    let d = individual_summary(&s, &pid(&s, "John"), &Options::default()).unwrap();
    let paras: Vec<&String> = d
        .blocks
        .iter()
        .filter_map(|b| {
            if let Block::Para(p) = b {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(paras[0], "John Smith was born on 3 March 1850 in Konya, Turkey.[1] He was the child of Old Smith and Olga Brown.");
    assert_eq!(paras[1], "He worked as Farmer.");
    assert_eq!(paras[2], "He married Mary Jones on 12 June 1874 in Konya, Turkey. They had 2 children: William (1875), Anna (1878).");
    assert_eq!(
        paras[3],
        "He died on 1 January 1920 in Ankara, Turkey (aged 69). He was buried in Ankara, Turkey."
    );
    assert_eq!(
        d.footnotes,
        vec!["Priest Ahmet, *Parish Register* (Konya, 1850), p. 12.".to_string()]
    );
    assert_eq!(d.title, "John Smith");
}

#[test]
fn turkish_narrative_uses_harmony_and_no_gendered_pronouns() {
    let s = load(FAMILY);
    let o = Options {
        lang: ReportLang::Tr,
        ..Default::default()
    };
    let d = individual_summary(&s, &pid(&s, "John"), &o).unwrap();
    let paras: Vec<&String> = d
        .blocks
        .iter()
        .filter_map(|b| {
            if let Block::Para(p) = b {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(paras[0], "John Smith, 3 Mart 1850 tarihinde Konya, Turkey'de doğdu.[1] Old Smith ile Olga Brown çiftinin çocuğudur.");
    assert!(
        paras[2].starts_with("Mary Jones ile 12 Haziran 1874 tarihinde Konya, Turkey'de evlendi."),
        "{}",
        paras[2]
    );
    assert!(paras[2].contains("2 çocukları oldu: William (1875), Anna (1878)."));
    assert!(
        paras[3].starts_with("1 Ocak 1920 tarihinde Ankara, Turkey'de vefat etti (69 yaşında)."),
        "{}",
        paras[3]
    );
    assert_eq!(d.lang, "tr");
}

#[test]
fn turkish_locative_follows_vowel_harmony_and_assimilation() {
    for (w, want) in [
        ("Konya", "Konya'da"),
        ("Ankara", "Ankara'da"),
        ("İzmir", "İzmir'de"),
        ("Edirne", "Edirne'de"),
        ("Kars", "Kars'ta"),
        ("Sivas", "Sivas'ta"),
        ("Muş", "Muş'ta"),
        ("Çorum", "Çorum'da"),
        ("Rize", "Rize'de"),
        ("Trabzon", "Trabzon'da"),
        ("Diyarbakır", "Diyarbakır'da"),
        ("Bursa", "Bursa'da"),
        ("Mersin", "Mersin'de"),
        ("Van", "Van'da"),
        ("Köln", "Köln'de"),
        ("Berlin", "Berlin'de"),
        ("Paris", "Paris'te"),
        ("Isparta", "Isparta'da"),
        ("Ordu", "Ordu'da"),
        ("Bolu", "Bolu'da"),
    ] {
        assert_eq!(locative_tr(w), want);
    }
}

#[test]
fn dates_with_different_precision_read_naturally() {
    let s = load("0 @I1@ INDI\n1 NAME A /B/\n1 SEX F\n1 BIRT\n2 DATE ABT 1850\n1 DEAT\n2 DATE BET 1900 AND 1910\n");
    let d = individual_summary(&s, &pid(&s, "A"), &Options::default()).unwrap();
    let text = to_markdown(&d);
    assert!(text.contains("A B was born around 1850."), "{text}");
    assert!(text.contains("She died between 1900 and 1910."), "{text}");
    let s = load("0 @I1@ INDI\n1 NAME A /B/\n1 BIRT\n2 DATE MAR 1850\n1 RESI\n2 PLAC Berlin\n2 DATE FROM 1900 TO 1910\n");
    let text = to_markdown(&individual_summary(&s, &pid(&s, "A"), &Options::default()).unwrap());
    assert!(text.contains("was born in March 1850."), "{text}");
    assert!(
        text.contains("They lived in Berlin from 1900 to 1910."),
        "{text}"
    );
}

#[test]
fn templates_are_user_editable_and_per_language() {
    let s = load(FAMILY);
    let mut o = Options::default();
    o.templates.insert(
        "birth_full".into(),
        "Born: {date}, {place} — {name}.".into(),
    );
    let d = individual_summary(&s, &pid(&s, "John"), &o).unwrap();
    let Block::Para(p) = &d.blocks[1] else {
        panic!()
    };
    assert!(
        p.starts_with("Born: on 3 March 1850, Konya, Turkey — John Smith."),
        "{p}"
    );
    // a language-specific override does not leak into the other language
    let mut o2 = Options::default();
    o2.templates.insert("tr.birth_full".into(), "X".into());
    let d = individual_summary(&s, &pid(&s, "John"), &o2).unwrap();
    assert!(matches!(&d.blocks[1], Block::Para(p) if p.starts_with("John Smith was born")));
}

#[test]
fn ancestor_and_descendant_reports_are_numbered_and_indexed() {
    let s = load(FAMILY);
    let anc = ancestor_report(&s, &pid(&s, "William"), &Options::default()).unwrap();
    let heads: Vec<String> = anc
        .blocks
        .iter()
        .filter_map(|b| {
            if let Block::Heading { level: 3, text, .. } = b {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(heads[0], "1. William Smith (1875–)");
    assert!(heads.iter().any(|h| h.starts_with("2. John Smith")));
    assert!(heads.iter().any(|h| h.starts_with("3. Mary Jones")));
    assert!(heads.iter().any(|h| h.starts_with("4. Old Smith")));
    assert_eq!(anc.title, "Ancestors of William Smith");
    assert!(anc
        .index
        .windows(2)
        .all(|w| kintree_core::name::fold(&w[0].0) <= kintree_core::name::fold(&w[1].0)));

    let desc = descendant_report(&s, &pid(&s, "Old"), &Options::default()).unwrap();
    let heads: Vec<String> = desc
        .blocks
        .iter()
        .filter_map(|b| {
            if let Block::Heading { level: 3, text, .. } = b {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(heads[0].starts_with("1 Old Smith"));
    assert!(heads.iter().any(|h| h.starts_with("1.1 John Smith")));
    assert!(heads.iter().any(|h| h.starts_with("1.1.1 William Smith")));
    assert!(heads.iter().any(|h| h.starts_with("1.1.2 Anna Smith")));
}

#[test]
fn family_group_sheet_lists_partners_and_children() {
    let s = load(FAMILY);
    let fam = s
        .rows("family")
        .unwrap()
        .into_iter()
        .find(|f| f["partner1"].as_str() == Some(&pid(&s, "John")))
        .unwrap();
    let d = family_group_sheet(&s, fam["id"].as_str().unwrap(), &Options::default()).unwrap();
    assert_eq!(d.title, "Family group sheet: John Smith & Mary Jones");
    let md = to_markdown(&d);
    assert!(md.contains("| Born | 3 Mar 1850 | Konya, Turkey |"), "{md}"); // tables use the compact form
    assert!(md.contains("| William Smith | 1875 |"));
    assert!(md.contains("| Anna Smith | 1878 |"));
}

#[test]
fn privacy_masks_or_excludes_living_people() {
    // Lisa (born 1995) is a third child of John & Mary; her details must never leak into anyone else's narrative.
    let tree = FAMILY
        .replace("1 CHIL @I4@\n1 MARR", "1 CHIL @I4@\n1 CHIL @I9@\n1 MARR")
        .replace("0 @S1@ SOUR", "0 @I9@ INDI\n1 NAME Lisa /Livingston/\n1 SEX F\n1 BIRT\n2 DATE 1995\n2 PLAC Secretville\n1 FAMC @F1@\n0 @S1@ SOUR");
    let s = load(&tree);
    let john = pid(&s, "John");
    let all = to_markdown(&individual_summary(&s, &john, &Options::default()).unwrap());
    assert!(all.contains("Lisa Livingston (1995)"), "{all}");

    for (privacy, children) in [
        (Privacy::Mask, "had 3 children"),
        (Privacy::Exclude, "had 2 children"),
    ] {
        let o = Options {
            privacy,
            ..Default::default()
        };
        let md = to_markdown(&individual_summary(&s, &john, &o).unwrap());
        for secret in ["Lisa", "Livingston", "1995", "Secretville"] {
            assert!(!md.contains(secret), "{privacy:?} leaked {secret}: {md}");
        }
        assert!(md.contains(children), "{privacy:?}: {md}");
    }
    let masked = to_markdown(
        &individual_summary(
            &s,
            &john,
            &Options {
                privacy: Privacy::Mask,
                ..Default::default()
            },
        )
        .unwrap(),
    );
    assert!(
        masked.contains("Living"),
        "masked children are shown as 'Living'"
    );

    // a living person's own report: unavailable when excluded, details-free when masked
    let lisa = pid(&s, "Lisa");
    let excluded = individual_summary(
        &s,
        &lisa,
        &Options {
            privacy: Privacy::Exclude,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(excluded.blocks.is_empty());
    let own = to_markdown(
        &individual_summary(
            &s,
            &lisa,
            &Options {
                privacy: Privacy::Mask,
                ..Default::default()
            },
        )
        .unwrap(),
    );
    assert!(
        !own.contains("Secretville") && !own.contains("1995"),
        "{own}"
    );

    // family sheets and other report types obey the same rules
    let fam = s
        .rows("family")
        .unwrap()
        .into_iter()
        .find(|f| f["partner1"].as_str() == Some(&john))
        .unwrap();
    let sheet = to_markdown(
        &family_group_sheet(
            &s,
            fam["id"].as_str().unwrap(),
            &Options {
                privacy: Privacy::Exclude,
                ..Default::default()
            },
        )
        .unwrap(),
    );
    assert!(
        !sheet.contains("Lisa") && !sheet.contains("1995"),
        "{sheet}"
    );
    let book = to_markdown(
        &book(
            &s,
            &john,
            &Options {
                privacy: Privacy::Mask,
                ..Default::default()
            },
        )
        .unwrap(),
    );
    assert!(
        !book.contains("Secretville") && !book.contains("Livingston"),
        "{book}"
    );
}

#[test]
fn html_output_is_escaped_and_footnotes_link() {
    let s = load("0 @I1@ INDI\n1 NAME <script>alert(1)</script> /O'Brien & Sons/\n1 SEX M\n1 BIRT\n2 DATE 1850\n2 SOUR @S1@\n0 @S1@ SOUR\n1 TITL A \"quoted\" <title>\n");
    let id = s.rows("person").unwrap()[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let d = individual_summary(&s, &id, &Options::default()).unwrap();
    let html = to_html(&d);
    assert!(!html.contains("<script>"), "{html}");
    assert!(html.contains("&lt;script&gt;"));
    assert!(html.contains("<sup><a href=\"#fn1\">1</a></sup>"));
    assert!(html.contains("<li id=\"fn1\">"));
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("lang=\"en\""));
}

#[test]
fn book_numbers_footnotes_continuously_and_has_one_index() {
    let s = load(FAMILY);
    let d = book(&s, &pid(&s, "John"), &Options::default()).unwrap();
    assert!(d.title.starts_with("Family book: John Smith"));
    // every footnote marker used in the text resolves to an existing footnote
    let html = to_html(&d);
    for n in 1..=d.footnotes.len() {
        assert!(html.contains(&format!("id=\"fn{}\"", n)));
    }
    let mut names: Vec<&String> = d.index.iter().map(|x| &x.0).collect();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len(), "no duplicate index entries");
    assert!(d.blocks.iter().any(
        |b| matches!(b, Block::Heading { level: 2, text, .. } if text.starts_with("Descendants of"))
    ));
}
