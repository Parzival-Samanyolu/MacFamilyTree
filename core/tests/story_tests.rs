use kintree_core::gedcom;
use kintree_core::media;
use kintree_core::report::{self, Options, Privacy};
use kintree_core::store::Store;
use kintree_core::story::*;

fn project() -> (Store, String, String) {
    let mut s = Store::open_memory().unwrap();
    gedcom::import(
        &mut s,
        b"0 HEAD\n1 GEDC\n2 VERS 5.5.1\n0 @I1@ INDI\n1 NAME Ali /Kaya/\n1 SEX M\n1 BIRT\n2 DATE 3 MAR 1850\n2 PLAC Konya\n1 DEAT\n2 DATE 1920\n0 @I2@ INDI\n1 NAME Zeynep /Kaya/\n1 SEX F\n1 BIRT\n2 DATE 2001\n0 TRLR\n",
    )
    .unwrap();
    let id = |g: &str| {
        s.rows("person_name")
            .unwrap()
            .into_iter()
            .find(|n| n["given"] == g)
            .unwrap()["person_id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let (a, z) = (id("Ali"), id("Zeynep"));
    (s, a, z)
}

fn png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(8, 8, image::Rgb([9, 9, 9]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

#[test]
fn save_get_list_delete_and_undo() {
    let (mut s, a, _) = project();
    let blocks = vec![
        StoryBlock::Heading {
            text: "Origins".into(),
        },
        StoryBlock::Person { id: a },
    ];
    let id = save(&mut s, None, "  The Kaya line ", &blocks).unwrap();
    assert_eq!(list(&s).unwrap()[0]["title"], "The Kaya line");
    assert_eq!(
        get(&s, &id).unwrap().unwrap()["blocks"][0]["type"],
        "heading"
    );
    save(&mut s, Some(&id), "Renamed", &blocks[..1]).unwrap();
    assert_eq!(list(&s).unwrap().len(), 1);
    assert_eq!(list(&s).unwrap()[0]["blocks"], 1);
    assert!(save(&mut s, None, "   ", &[]).is_err());
    delete(&mut s, &id).unwrap();
    assert!(list(&s).unwrap().is_empty());
    s.undo().unwrap();
    assert_eq!(list(&s).unwrap().len(), 1);
}

#[test]
fn render_builds_headings_text_quotes_people_timeline_and_images() {
    let (mut s, a, _) = project();
    let m = media::import(&mut s, "ali.png", &png(), Some(("person", &a))).unwrap();
    let id = save(
        &mut s,
        None,
        "Ali's story",
        &[
            StoryBlock::Heading {
                text: "Early life".into(),
            },
            StoryBlock::Text {
                text: "First paragraph.\n\nSecond <b>paragraph</b>.".into(),
            },
            StoryBlock::Quote {
                text: "Work hard".into(),
                by: Some("Ali".into()),
            },
            StoryBlock::Person { id: a.clone() },
            StoryBlock::Timeline { id: a },
            StoryBlock::Media {
                id: m.id,
                caption: Some("Ali, c. 1900".into()),
            },
        ],
    )
    .unwrap();
    let doc = render(&s, &id, &Options::default()).unwrap();
    let html = report::to_html(&doc);
    assert!(
        html.contains("<h1>Ali&#39;s story</h1>") || html.contains("<h1>Ali's story</h1>"),
        "{html}"
    );
    assert!(html.contains("<h2>Early life</h2>"));
    assert!(
        html.contains("<p>First paragraph.</p>")
            && html.contains("Second &lt;b&gt;paragraph&lt;/b&gt;."),
        "{html}"
    );
    assert!(html.contains("“Work hard” — Ali"));
    assert!(
        html.contains("Ali Kaya was born on 3 March 1850 in Konya."),
        "{html}"
    );
    assert!(html.contains("<h3 id="), "person heading demoted");
    assert!(html.contains("<table>") && html.contains("Birth: Ali Kaya"));
    assert!(
        html.contains("<img src=\"data:image/png;base64,")
            && html.contains("<figcaption>Ali, c. 1900</figcaption>")
    );
    let md = report::to_markdown(&doc);
    assert!(md.contains("*[Ali, c. 1900]*") && !md.contains("base64"));
}

#[test]
fn privacy_hides_living_people_and_their_pictures() {
    let (mut s, _, z) = project();
    let m = media::import(&mut s, "z.png", &png(), Some(("person", &z))).unwrap();
    let id = save(
        &mut s,
        None,
        "Family today",
        &[
            StoryBlock::Person { id: z.clone() },
            StoryBlock::Media {
                id: m.id,
                caption: None,
            },
            StoryBlock::Timeline { id: z },
        ],
    )
    .unwrap();
    let open = report::to_html(&render(&s, &id, &Options::default()).unwrap());
    assert!(open.contains("Zeynep") && open.contains("<img"));
    let o = Options {
        privacy: Privacy::Exclude,
        ..Default::default()
    };
    let hidden = report::to_html(&render(&s, &id, &o).unwrap());
    assert!(
        !hidden.contains("2001") && !hidden.contains("<img"),
        "{hidden}"
    );
}

#[test]
fn render_unknown_story_and_corrupt_blocks_are_safe() {
    let (mut s, _, _) = project();
    assert!(render(&s, "nope", &Options::default()).is_err());
    s.transact("bad", |tx| {
        let mut r = serde_json::Map::new();
        r.insert("id".into(), "s1".into());
        r.insert("title".into(), "Broken".into());
        r.insert("blocks".into(), "not json".into());
        tx.put_row("story", r)?;
        Ok(())
    })
    .unwrap();
    let d = render(&s, "s1", &Options::default()).unwrap();
    assert_eq!(d.blocks.len(), 1);
}
