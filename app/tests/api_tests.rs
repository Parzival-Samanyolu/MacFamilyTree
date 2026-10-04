use base64::{engine::general_purpose::STANDARD as B64, Engine};
use kintree_app::{dispatch, Session};
use serde_json::{json, Value};

fn call(s: &mut Session, cmd: &str, args: Value) -> Value {
    dispatch(s, cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {} {}", e.code, e.message))
}
fn err(s: &mut Session, cmd: &str, args: Value) -> String {
    dispatch(s, cmd, args)
        .err()
        .unwrap_or_else(|| panic!("{cmd} unexpectedly succeeded"))
        .code
}

fn fresh() -> Session {
    let mut s = Session::new();
    call(&mut s, "project.new", json!({}));
    s
}

#[test]
fn commands_require_an_open_project() {
    let mut s = Session::new();
    assert_eq!(err(&mut s, "person.list", json!({})), "no_project");
    assert_eq!(err(&mut s, "nope", json!({})), "unknown_command");
    assert_eq!(call(&mut s, "project.status", json!({}))["open"], false);
}

#[test]
fn build_a_family_with_quick_add_and_edit_it() {
    let mut s = fresh();
    let me = call(
        &mut s,
        "person.create",
        json!({"given": "Emre", "surname": "Kaya", "sex": "M"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    let dad = call(
        &mut s,
        "relative.add",
        json!({"person_id": me, "kind": "father", "given": "Ali"}),
    );
    let mom = call(
        &mut s,
        "relative.add",
        json!({"person_id": me, "kind": "mother", "given": "Fatma", "surname": "Demir"}),
    );
    assert_eq!(
        dad["family_id"], mom["family_id"],
        "both parents share one family"
    );
    let sis = call(
        &mut s,
        "relative.add",
        json!({"person_id": me, "kind": "sibling", "given": "Zeynep", "sex": "F"}),
    );
    let wife = call(
        &mut s,
        "relative.add",
        json!({"person_id": me, "kind": "partner", "given": "Elif", "sex": "F", "surname": "Yıldız"}),
    );
    let kid = call(
        &mut s,
        "relative.add",
        json!({"person_id": me, "kind": "child", "given": "Can", "sex": "M"}),
    );
    assert_eq!(kid["family_id"], wife["family_id"]);

    // surname inheritance: Dad inherits Emre's surname; sibling and child take the father's surname
    let d = call(&mut s, "person.get", json!({"id": dad["person_id"]}));
    assert_eq!(d["summary"]["surname"], "Kaya");
    let z = call(&mut s, "person.get", json!({"id": sis["person_id"]}));
    assert_eq!(z["summary"]["surname"], "Kaya");
    let c = call(&mut s, "person.get", json!({"id": kid["person_id"]}));
    assert_eq!(c["summary"]["surname"], "Kaya");
    assert_eq!(
        c["child_families"][0]["parents"].as_array().unwrap().len(),
        2
    );

    // a third parent is rejected
    assert_eq!(
        err(
            &mut s,
            "relative.add",
            json!({"person_id": me, "kind": "father", "given": "X"})
        ),
        "store"
    );

    // events with free-text dates and places
    call(
        &mut s,
        "event.put",
        json!({"owner_type": "person", "owner_id": me, "kind": "BIRT", "date_text": "3 Mart 1980", "place_text": "Konya, Turkey"}),
    );
    call(
        &mut s,
        "event.put",
        json!({"owner_type": "person", "owner_id": me, "kind": "OCCU", "value": "Engineer", "date_text": "from 2005"}),
    );
    let ev = call(
        &mut s,
        "event.put",
        json!({"owner_type": "person", "owner_id": me, "kind": "RESI", "date_text": "1990", "place_text": "Ankara, Turkey"}),
    );
    let p = call(&mut s, "person.get", json!({"id": me}));
    let events = p["events"].as_array().unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0]["kind"], "BIRT", "chronological order");
    assert_eq!(events[0]["date_gedcom"], "3 MAR 1980");
    assert_eq!(events[0]["place_text"], "Konya, Turkey");
    assert_eq!(events[1]["age"], 9, "age at event is computed");
    assert_eq!(p["summary"]["birth_year"], 1980);
    assert_eq!(p["summary"]["life"], "1980–");

    // edit + delete
    call(
        &mut s,
        "event.put",
        json!({"id": ev["id"], "date_text": "1991", "place_text": "Istanbul, Turkey"}),
    );
    let p = call(&mut s, "person.get", json!({"id": me}));
    assert!(p["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["date_gedcom"] == "1991" && e["place_text"] == "Istanbul, Turkey"));
    call(&mut s, "event.delete", json!({"id": ev["id"]}));
    assert_eq!(
        call(&mut s, "person.get", json!({"id": me}))["events"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    // search finds by event place and diacritic-insensitive names
    let hits = call(&mut s, "search", json!({"q": "konya emre"}));
    assert_eq!(hits.as_array().unwrap().len(), 1);
    assert_eq!(
        call(&mut s, "search", json!({"q": "yildiz"}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn undo_redo_through_the_api() {
    let mut s = fresh();
    let id = call(
        &mut s,
        "person.create",
        json!({"given": "A", "surname": "B"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &mut s,
        "event.put",
        json!({"owner_type": "person", "owner_id": id, "kind": "BIRT", "date_text": "1900"}),
    );
    let st = call(&mut s, "project.status", json!({}));
    assert_eq!(st["undo_label"], "Add event");
    assert_eq!(
        call(&mut s, "history.undo", json!({}))["label"],
        "Add event"
    );
    assert_eq!(
        call(&mut s, "person.get", json!({"id": id}))["events"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    call(&mut s, "history.redo", json!({}));
    assert_eq!(
        call(&mut s, "person.get", json!({"id": id}))["events"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    call(&mut s, "history.undo", json!({}));
    call(&mut s, "history.undo", json!({}));
    assert_eq!(call(&mut s, "project.status", json!({}))["persons"], 0);
    assert_eq!(call(&mut s, "project.status", json!({}))["can_undo"], false);
}

#[test]
fn sample_project_lists_pages_trees_and_relationships() {
    let mut s = Session::new();
    call(&mut s, "project.load_sample", json!({"persons": 300}));
    let page = call(&mut s, "person.list", json!({"limit": 25, "sort": "name"}));
    assert!(page["total"].as_i64().unwrap() >= 290);
    let items = page["items"].as_array().unwrap();
    assert_eq!(items.len(), 25);
    let names: Vec<String> = items
        .iter()
        .map(|i| {
            format!(
                "{} {}",
                i["surname"].as_str().unwrap().to_lowercase(),
                i["given"].as_str().unwrap().to_lowercase()
            )
        })
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "list is sorted by surname then given name");
    let page2 = call(&mut s, "person.list", json!({"limit": 25, "offset": 25}));
    assert_ne!(page2["items"][0]["id"], items[0]["id"]);

    // pick someone with ancestors and descendants
    let all = call(
        &mut s,
        "person.list",
        json!({"limit": 500, "sort": "added"}),
    );
    let mut root = None;
    for it in all["items"].as_array().unwrap() {
        let d = call(&mut s, "person.get", json!({"id": it["id"]}));
        if !d["child_families"].as_array().unwrap().is_empty()
            && !d["partner_families"].as_array().unwrap().is_empty()
            && !d["partner_families"][0]["children"]
                .as_array()
                .unwrap()
                .is_empty()
        {
            root = Some(it["id"].as_str().unwrap().to_string());
            break;
        }
    }
    let root = root.expect("sample has a middle-generation person");
    let t = call(
        &mut s,
        "tree.layout",
        json!({"root": root, "mode": "hourglass", "ancestors": 3, "descendants": 3}),
    );
    let nodes = t["layout"]["nodes"].as_array().unwrap();
    assert!(nodes.len() > 3);
    for n in nodes {
        assert!(
            t["people"][n["person_id"].as_str().unwrap()]["name"].is_string(),
            "every node has a summary"
        );
    }
    let lr = call(
        &mut s,
        "tree.layout",
        json!({"root": root, "mode": "ancestors", "direction": "lr"}),
    );
    assert!(lr["layout"]["width"].as_f64().unwrap() > 0.0);

    let ahn = call(
        &mut s,
        "chart.ahnentafel",
        json!({"root": root, "generations": 2}),
    );
    assert_eq!(ahn["items"][0]["n"], 1);

    let d = call(&mut s, "person.get", json!({"id": root}));
    let child = d["partner_families"][0]["children"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let r = call(
        &mut s,
        "relationship.calc",
        json!({"a": root, "b": child, "lang": "en"}),
    );
    let desc = r["descriptions"][0].as_str().unwrap();
    assert!(
        desc == "son" || desc == "daughter" || desc == "child",
        "{desc}"
    );
    let r_tr = call(
        &mut s,
        "relationship.calc",
        json!({"a": child, "b": root, "lang": "tr"}),
    );
    let tr = r_tr["descriptions"][0].as_str().unwrap();
    assert!(tr == "baba" || tr == "anne" || tr == "ebeveyn", "{tr}");
    assert_eq!(r["kind"], "blood");
}

#[test]
fn gedcom_import_export_roundtrip_via_api() {
    let mut s = Session::new();
    call(&mut s, "project.load_sample", json!({"persons": 60}));
    let exp = call(&mut s, "gedcom.export", json!({"version": "5.5.1"}));
    let bytes = B64.decode(exp["data"].as_str().unwrap()).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(
        text.starts_with("0 HEAD")
            && text.contains("2 VERS 5.5.1")
            && text.trim_end().ends_with("0 TRLR")
    );

    let mut s2 = Session::new();
    let r = call(
        &mut s2,
        "gedcom.import",
        json!({"data": B64.encode(&bytes)}),
    );
    assert_eq!(
        r["status"]["persons"],
        call(&mut s, "project.status", json!({}))["persons"]
    );
    assert!(r["report"]["issues"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["severity"] != "error"));
    let exp2 = call(&mut s2, "gedcom.export", json!({"version": "5.5.1"}));
    assert_eq!(exp["data"], exp2["data"], "export is stable across import");

    // privacy-filtered export hides living people (and only them)
    let lisa = call(
        &mut s,
        "person.create",
        json!({"given": "Lisa", "surname": "Livingston", "sex": "F"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &mut s,
        "event.put",
        json!({"owner_type": "person", "owner_id": lisa, "kind": "BIRT", "date_text": "1995", "place_text": "Secretville"}),
    );
    let get = |s: &mut Session, args: Value| {
        String::from_utf8(
            B64.decode(call(s, "gedcom.export", args)["data"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap()
    };
    let all = get(&mut s, json!({}));
    assert!(all.contains("Livingston") && all.contains("Secretville"));
    let excluded = get(&mut s, json!({"living": "exclude"}));
    assert!(!excluded.contains("Livingston") && !excluded.contains("Secretville"));
    let masked = get(&mut s, json!({"living": "mask"}));
    assert!(
        !masked.contains("Livingston")
            && !masked.contains("Secretville")
            && masked.contains("NAME Living")
    );
    assert!(
        excluded.contains("Smith") || excluded.contains("Kaya") || excluded.contains("Yılmaz"),
        "deceased people remain"
    );
}

#[test]
fn quality_duplicates_and_stats_endpoints() {
    let mut s = fresh();
    let a = call(
        &mut s,
        "person.create",
        json!({"given": "Mehmet", "surname": "Yılmaz", "sex": "M"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    let b = call(
        &mut s,
        "person.create",
        json!({"given": "Mehmed", "surname": "Yilmaz", "sex": "M"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    for id in [&a, &b] {
        call(
            &mut s,
            "event.put",
            json!({"owner_type": "person", "owner_id": id, "kind": "BIRT", "date_text": "1880", "place_text": "Konya"}),
        );
    }
    call(
        &mut s,
        "event.put",
        json!({"owner_type": "person", "owner_id": a, "kind": "DEAT", "date_text": "1870"}),
    );
    let q = call(&mut s, "quality.check", json!({}));
    let rules: Vec<&str> = q
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["rule"].as_str().unwrap())
        .collect();
    assert!(rules.contains(&"death_before_birth"));
    let f = q
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["rule"] == "death_before_birth")
        .unwrap();
    assert_eq!(f["person_id"], a.as_str());

    let d = call(&mut s, "duplicates.find", json!({"threshold": 0.6}));
    assert_eq!(d.as_array().unwrap().len(), 1);
    call(&mut s, "duplicates.merge", json!({"keep": a, "remove": b}));
    assert_eq!(call(&mut s, "project.status", json!({}))["persons"], 1);
    call(&mut s, "history.undo", json!({}));
    assert_eq!(call(&mut s, "project.status", json!({}))["persons"], 2);

    let st = call(&mut s, "stats.compute", json!({}));
    assert_eq!(st["counts"]["persons"], 2);
    assert_eq!(st["top_places"][0][0], "Konya");
}

#[test]
fn generic_records_and_settings() {
    let mut s = fresh();
    let src = call(
        &mut s,
        "rec.put",
        json!({"table": "source", "row": {"title": "Census 1880", "author": "Gov"}}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    let rec = call(&mut s, "rec.get", json!({"table": "source", "id": src}));
    assert_eq!(rec["title"], "Census 1880");
    assert_eq!(
        call(&mut s, "rec.list", json!({"table": "source"}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        err(&mut s, "rec.list", json!({"table": "history"})),
        "bad_args",
        "internal tables are not exposed"
    );
    call(&mut s, "rec.delete", json!({"table": "source", "id": src}));
    assert_eq!(
        call(&mut s, "rec.list", json!({"table": "source"}))
            .as_array()
            .unwrap()
            .len(),
        0
    );

    call(
        &mut s,
        "settings.set",
        json!({"key": "lang", "value": "tr"}),
    );
    assert_eq!(call(&mut s, "settings.get", json!({}))["lang"], "tr");
    let id = call(
        &mut s,
        "person.create",
        json!({"given": "A", "surname": "B"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &mut s,
        "event.put",
        json!({"owner_type": "person", "owner_id": id, "kind": "BIRT", "date_text": "3 Mar 1850"}),
    );
    let p = call(&mut s, "person.get", json!({"id": id}));
    assert_eq!(
        p["events"][0]["date_text"], "3 Mart 1850",
        "dates follow the language preference"
    );
}

#[test]
fn naming_cultures_for_children() {
    let mut s = fresh();
    call(
        &mut s,
        "settings.set",
        json!({"key": "naming_culture", "value": "patronymic"}),
    );
    let dad = call(
        &mut s,
        "person.create",
        json!({"given": "Jón", "surname": "", "sex": "M"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    let kid = call(
        &mut s,
        "relative.add",
        json!({"person_id": dad, "kind": "child", "given": "Anna", "sex": "F"}),
    );
    let d = call(&mut s, "person.get", json!({"id": kid["person_id"]}));
    assert_eq!(d["summary"]["surname"], "Jónsdóttir");

    call(
        &mut s,
        "settings.set",
        json!({"key": "naming_culture", "value": "spanish"}),
    );
    let f = call(
        &mut s,
        "person.create",
        json!({"given": "Luis", "surname": "Pérez Ruiz", "sex": "M"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    let w = call(
        &mut s,
        "relative.add",
        json!({"person_id": f, "kind": "partner", "given": "Ana", "surname": "García López", "sex": "F"}),
    );
    assert!(w["family_id"].is_string());
    let c = call(
        &mut s,
        "relative.add",
        json!({"person_id": f, "kind": "child", "given": "Sofía", "sex": "F"}),
    );
    assert_eq!(
        call(&mut s, "person.get", json!({"id": c["person_id"]}))["summary"]["surname"],
        "Pérez García"
    );
}

#[test]
fn dashboard_on_this_day_upcoming_and_quality_score() {
    let mut s = fresh();
    let a = call(
        &mut s,
        "person.create",
        json!({"given": "Ayşe", "surname": "Demir", "sex": "F"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &mut s,
        "event.put",
        json!({"owner_type": "person", "owner_id": a, "kind": "BIRT", "date_text": "10 Mar 1990"}),
    );
    let b = call(
        &mut s,
        "person.create",
        json!({"given": "Old", "surname": "Timer", "sex": "M"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &mut s,
        "event.put",
        json!({"owner_type": "person", "owner_id": b, "kind": "BIRT", "date_text": "10 Mar 1800"}),
    );
    call(
        &mut s,
        "event.put",
        json!({"owner_type": "person", "owner_id": b, "kind": "DEAT", "date_text": "11 Mar 1870"}),
    );
    call(
        &mut s,
        "person.update",
        json!({"id": a, "bookmarked": true}),
    );

    let d = call(
        &mut s,
        "dashboard.data",
        json!({"year": 2026, "month": 3, "day": 10}),
    );
    let otd = d["on_this_day"].as_array().unwrap();
    assert_eq!(otd.len(), 2, "both births fall on 10 March");
    assert!(otd
        .iter()
        .any(|x| x["years_ago"] == 36 && x["person"]["given"] == "Ayşe"));
    let up = d["upcoming"].as_array().unwrap();
    assert_eq!(
        up.len(),
        1,
        "only the living person has an upcoming birthday: {up:?}"
    );
    assert_eq!(up[0]["days"], 0);
    assert_eq!(up[0]["turning"], 36);
    assert_eq!(d["bookmarks"][0]["given"], "Ayşe");
    assert!(d["quality"]["score"].as_f64().unwrap() <= 100.0);
    assert!(d["random"]["id"].is_string());
    // a day later the birthday is no longer "upcoming" (next is a year away)
    let d2 = call(
        &mut s,
        "dashboard.data",
        json!({"year": 2026, "month": 3, "day": 11}),
    );
    assert!(d2["upcoming"].as_array().unwrap().is_empty());
    assert_eq!(
        d2["on_this_day"].as_array().unwrap().len(),
        1,
        "the death on 11 March"
    );
}

#[test]
fn date_parse_preview() {
    let mut s = fresh();
    let ok = call(&mut s, "date.parse", json!({"text": "abt 3 Mar 1850"}));
    assert_eq!(ok["valid"], true);
    assert_eq!(ok["gedcom"], "ABT 3 MAR 1850");
    assert_eq!(
        call(&mut s, "date.parse", json!({"text": "31 Feb 1850"}))["valid"],
        false
    );
    assert_eq!(
        call(&mut s, "date.parse", json!({"text": ""}))["valid"],
        true
    );
}

#[test]
fn notes_and_citations_are_atomic_and_undoable() {
    let mut s = fresh();
    let id = call(
        &mut s,
        "person.create",
        json!({"given": "A", "surname": "B"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &mut s,
        "note.add",
        json!({"target_type": "person", "target_id": id, "body": "Remember to check the **census**."}),
    );
    call(
        &mut s,
        "citation.add",
        json!({"target_type": "person", "target_id": id, "new_source_title": "1881 Census", "page": "folio 12", "quality": "3"}),
    );
    let p = call(&mut s, "person.get", json!({"id": id}));
    assert_eq!(p["notes"][0]["body"], "Remember to check the **census**.");
    assert_eq!(p["citations"][0]["source_title"], "1881 Census");
    assert_eq!(p["citations"][0]["page"], "folio 12");
    // re-use the same source for a second citation
    let src = p["citations"][0]["source_id"].as_str().unwrap().to_string();
    call(
        &mut s,
        "citation.add",
        json!({"target_type": "person", "target_id": id, "source_id": src, "page": "folio 13"}),
    );
    assert_eq!(
        call(&mut s, "rec.list", json!({"table": "source"}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        err(
            &mut s,
            "citation.add",
            json!({"target_type": "person", "target_id": id})
        ),
        "store"
    );

    let link = p["notes"][0]["link_id"].as_str().unwrap().to_string();
    call(&mut s, "note.remove", json!({"link_id": link}));
    assert!(call(&mut s, "person.get", json!({"id": id}))["notes"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(
        call(&mut s, "rec.list", json!({"table": "note"}))
            .as_array()
            .unwrap()
            .is_empty(),
        "orphaned note deleted"
    );
    call(&mut s, "history.undo", json!({}));
    assert_eq!(
        call(&mut s, "person.get", json!({"id": id}))["notes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn tree_cards_can_include_places() {
    let mut s = fresh();
    let id = call(
        &mut s,
        "person.create",
        json!({"given": "A", "surname": "B"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &mut s,
        "event.put",
        json!({"owner_type": "person", "owner_id": id, "kind": "BIRT", "date_text": "1900", "place_text": "Konya, Turkey"}),
    );
    let plain = call(
        &mut s,
        "tree.layout",
        json!({"root": id, "mode": "ancestors"}),
    );
    assert!(plain["people"][&id]["birth_place"].is_null());
    let with = call(
        &mut s,
        "tree.layout",
        json!({"root": id, "mode": "ancestors", "show_places": true}),
    );
    assert_eq!(with["people"][&id]["birth_place"], "Konya, Turkey");
}

#[test]
fn blank_surname_means_inherit_not_empty() {
    // The guided-start wizard sends "" for an untouched surname field; that must not override inheritance.
    let mut s = fresh();
    let me = call(
        &mut s,
        "person.create",
        json!({"given": "Emre", "surname": "Kaya", "sex": "M"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    let dad = call(
        &mut s,
        "relative.add",
        json!({"person_id": me, "kind": "father", "given": "Ali", "surname": ""}),
    );
    assert_eq!(
        call(&mut s, "person.get", json!({"id": dad["person_id"]}))["summary"]["surname"],
        "Kaya"
    );
    let sib = call(
        &mut s,
        "relative.add",
        json!({"person_id": me, "kind": "sibling", "given": "Zeynep", "surname": "   "}),
    );
    assert_eq!(
        call(&mut s, "person.get", json!({"id": sib["person_id"]}))["summary"]["surname"],
        "Kaya"
    );
}

#[test]
fn file_backed_projects_persist_back_up_and_restore() {
    let dir = std::env::temp_dir().join(format!("kt-proj-{}", kintree_core::store::new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("family.ktree");
    let mut s = Session::new();
    call(
        &mut s,
        "project.create",
        json!({"path": path.display().to_string()}),
    );
    assert_eq!(
        err(
            &mut s,
            "project.create",
            json!({"path": path.display().to_string()})
        ),
        "exists"
    );
    let id = call(
        &mut s,
        "person.create",
        json!({"given": "Persist", "surname": "Me", "sex": "F"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &mut s,
        "event.put",
        json!({"owner_type": "person", "owner_id": id, "kind": "BIRT", "date_text": "1900"}),
    );

    // manual backup is a complete, standalone copy
    let bak = dir.join("manual.bak");
    call(
        &mut s,
        "project.backup",
        json!({"path": bak.display().to_string()}),
    );
    assert_eq!(
        err(
            &mut s,
            "project.backup",
            json!({"path": bak.display().to_string()})
        ),
        "exists"
    );
    call(&mut s, "project.close", json!({}));

    // reopening keeps data (autosave) and creates a rolling backup next to the project
    let mut s2 = Session::new();
    call(
        &mut s2,
        "project.open",
        json!({"path": path.display().to_string()}),
    );
    assert_eq!(call(&mut s2, "project.status", json!({}))["persons"], 1);
    assert_eq!(
        call(&mut s2, "person.get", json!({"id": id}))["events"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let backups: Vec<_> = std::fs::read_dir(dir.join("backups")).unwrap().collect();
    assert_eq!(backups.len(), 1);
    // undo history survives a restart too
    assert_eq!(call(&mut s2, "project.status", json!({}))["can_undo"], true);

    // restore the manual backup into a new working copy after deleting the person
    call(&mut s2, "person.delete", json!({"id": id}));
    let restored = dir.join("restored.ktree");
    let mut s3 = Session::new();
    call(
        &mut s3,
        "project.restore",
        json!({"from": bak.display().to_string(), "to": restored.display().to_string()}),
    );
    assert_eq!(call(&mut s3, "project.status", json!({}))["persons"], 1);
    assert_eq!(
        err(
            &mut s3,
            "project.open",
            json!({"path": dir.join("nope.ktree").display().to_string()})
        ),
        "not_found"
    );
}

#[test]
fn upcoming_birthdays_handle_month_ends_and_leap_days() {
    let mut s = fresh();
    for (given, date) in [
        ("Jan31", "31 Jan 1980"),
        ("Leap", "29 Feb 1980"),
        ("Far", "15 Aug 1980"),
    ] {
        let id = call(
            &mut s,
            "person.create",
            json!({"given": given, "surname": "X", "sex": "F"}),
        )["id"]
            .as_str()
            .unwrap()
            .to_string();
        call(
            &mut s,
            "event.put",
            json!({"owner_type": "person", "owner_id": id, "kind": "BIRT", "date_text": date}),
        );
    }
    // 27 Jan 2026: the 31st is 4 days away (not "28 Jan"); 29 Feb falls on 28 Feb in 2026 (32 days away -> not listed)
    let d = call(
        &mut s,
        "dashboard.data",
        json!({"year": 2026, "month": 1, "day": 27}),
    );
    let up = d["upcoming"].as_array().unwrap();
    assert_eq!(up.len(), 1, "{up:?}");
    assert_eq!(up[0]["person"]["given"], "Jan31");
    assert_eq!(up[0]["days"], 4);
    // 25 Feb 2026: leap-day birthday is observed on 28 Feb (3 days)
    let d = call(
        &mut s,
        "dashboard.data",
        json!({"year": 2026, "month": 2, "day": 25}),
    );
    let leap = d["upcoming"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["person"]["given"] == "Leap")
        .expect("leap-day birthday listed");
    assert_eq!(leap["days"], 3);
}

fn family_session() -> (Session, String, String) {
    let mut s = fresh();
    let me = call(
        &mut s,
        "person.create",
        json!({"given": "Emre", "surname": "Kaya", "sex": "M"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    let dad = call(
        &mut s,
        "relative.add",
        json!({"person_id": me, "kind": "father", "given": "Ali"}),
    );
    let mom = call(
        &mut s,
        "relative.add",
        json!({"person_id": me, "kind": "mother", "given": "Fatma", "surname": "Demir"}),
    );
    for (id, date, place) in [
        (me.as_str(), "3 Mar 1980", "Konya, Türkiye"),
        (dad["person_id"].as_str().unwrap(), "1950", "Ankara"),
        (mom["person_id"].as_str().unwrap(), "1955", "Kars"),
    ] {
        call(
            &mut s,
            "event.put",
            json!({"owner_type": "person", "owner_id": id, "kind": "BIRT", "date_text": date, "place_text": place}),
        );
    }
    call(
        &mut s,
        "event.put",
        json!({"owner_type": "family", "owner_id": dad["family_id"], "kind": "MARR", "date_text": "12 Jun 1975"}),
    );
    (s, me, dad["family_id"].as_str().unwrap().to_string())
}

#[test]
fn report_endpoints_render_in_both_languages_and_templates_persist() {
    let (mut s, me, fam) = family_session();
    let en = call(
        &mut s,
        "report.generate",
        json!({"kind": "individual", "id": me, "lang": "en"}),
    );
    assert!(en["markdown"]
        .as_str()
        .unwrap()
        .contains("Emre Kaya was born on 3 March 1980 in Konya, Türkiye."));
    assert!(en["html"].as_str().unwrap().starts_with("<!doctype html>"));
    let tr = call(
        &mut s,
        "report.generate",
        json!({"kind": "individual", "id": me, "lang": "tr"}),
    );
    assert!(
        tr["markdown"]
            .as_str()
            .unwrap()
            .contains("3 Mart 1980 tarihinde Konya, Türkiye'de doğdu."),
        "{}",
        tr["markdown"]
    );
    for kind in ["ancestors", "descendants", "book"] {
        let r = call(
            &mut s,
            "report.generate",
            json!({"kind": kind, "id": me, "generations": 3}),
        );
        assert!(
            r["markdown"].as_str().unwrap().contains("Emre Kaya"),
            "{kind}"
        );
    }
    let f = call(
        &mut s,
        "report.generate",
        json!({"kind": "family", "id": fam}),
    );
    assert!(f["title"]
        .as_str()
        .unwrap()
        .starts_with("Family group sheet"));
    assert_eq!(
        err(&mut s, "report.generate", json!({"kind": "nope", "id": me})),
        "bad_args"
    );

    call(
        &mut s,
        "report.set_template",
        json!({"lang": "en", "key": "birth_full", "value": "Born {date} at {place}: {name}."}),
    );
    let en2 = call(
        &mut s,
        "report.generate",
        json!({"kind": "individual", "id": me, "lang": "en"}),
    );
    assert!(en2["markdown"]
        .as_str()
        .unwrap()
        .contains("Born on 3 March 1980 at Konya, Türkiye: Emre Kaya."));
    let tpls = call(&mut s, "report.templates", json!({}));
    let t = tpls
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["lang"] == "en" && x["key"] == "birth_full")
        .unwrap();
    assert_eq!(t["value"], "Born {date} at {place}: {name}.");
    call(
        &mut s,
        "report.set_template",
        json!({"lang": "en", "key": "birth_full", "value": ""}),
    );
    let en3 = call(
        &mut s,
        "report.generate",
        json!({"kind": "individual", "id": me, "lang": "en"}),
    );
    assert!(
        en3["markdown"]
            .as_str()
            .unwrap()
            .contains("was born on 3 March 1980"),
        "clearing restores the default"
    );
}

#[test]
fn timeline_calendar_and_exports_via_api() {
    let (mut s, me, _) = family_session();
    let t = call(&mut s, "timeline.get", json!({"scope": "person", "id": me}));
    assert_eq!(t["entries"][0]["kind"], "BIRT");
    let all = call(&mut s, "timeline.get", json!({"overlay": true}));
    assert!(all["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["history"] == true));
    let l = call(&mut s, "timeline.lifespans", json!({}));
    assert_eq!(l["total"], 3);
    let cal = call(&mut s, "calendar.month", json!({"year": 2026, "month": 3}));
    assert!(cal
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["text"] == "Emre Kaya" && e["person"]["given"] == "Emre"));
    let ics = String::from_utf8(
        B64.decode(
            call(&mut s, "export.ical", json!({}))["data"]
                .as_str()
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(ics.contains("BEGIN:VEVENT") && ics.contains("RRULE:FREQ=YEARLY"));
    let csv = String::from_utf8(
        B64.decode(
            call(&mut s, "export.csv", json!({}))["data"]
                .as_str()
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        csv.contains("Emre,Kaya,M,3 MAR 1980,\"Konya, Türkiye\""),
        "{csv}"
    );
    let json_text = String::from_utf8(
        B64.decode(
            call(&mut s, "export.json", json!({}))["data"]
                .as_str()
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        serde_json::from_str::<Value>(&json_text).unwrap()["tables"]["person"]
            .as_array()
            .unwrap()
            .len()
            == 3
    );

    // CSV round trip into a new project
    let mut s2 = Session::new();
    let r = call(
        &mut s2,
        "csv.import",
        json!({"data": B64.encode(csv.as_bytes())}),
    );
    assert_eq!(r["persons"], 3);
    assert_eq!(r["families"], 1);
    assert_eq!(r["status"]["persons"], 3);
}

#[test]
fn library_unsourced_facts_tasks_and_bibliography() {
    let (mut s, me, _) = family_session();
    let un = call(&mut s, "sources.unsourced", json!({}));
    assert_eq!(un["total"], 4, "3 births + 1 marriage");
    let birth = un["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["label"] == "Emre Kaya")
        .unwrap();
    call(
        &mut s,
        "citation.add",
        json!({"target_type": "event", "target_id": birth["event_id"], "new_source_title": "Birth certificate", "page": "No. 77"}),
    );
    assert_eq!(
        call(&mut s, "sources.unsourced", json!({}))["total"],
        3,
        "citing a fact removes it from the to-do list"
    );

    let t = call(
        &mut s,
        "task.save",
        json!({"title": "Order the 1950 census", "priority": 2, "person_id": me, "status": "open"}),
    );
    call(
        &mut s,
        "task.save",
        json!({"title": "Done thing", "status": "done"}),
    );
    let tasks = call(&mut s, "task.list", json!({}));
    assert_eq!(
        tasks[0]["title"], "Order the 1950 census",
        "open tasks first, by priority"
    );
    assert_eq!(tasks[0]["person"]["given"], "Emre");
    call(
        &mut s,
        "task.save",
        json!({"id": t["id"], "title": "Order the 1950 census", "status": "done"}),
    );
    assert!(call(&mut s, "task.list", json!({}))
        .as_array()
        .unwrap()
        .iter()
        .all(|x| x["status"] == "done"));
    assert_eq!(err(&mut s, "task.save", json!({"title": "  "})), "bad_args");

    call(
        &mut s,
        "rec.put",
        json!({"table": "source", "row": {"title": "Parish register", "author": "Priest Ahmet", "publication": "Konya, 1850"}}),
    );
    let b = call(&mut s, "report.generate", json!({"kind": "bibliography"}));
    let md = b["markdown"].as_str().unwrap();
    assert!(
        md.contains("Priest Ahmet. *Parish register*. Konya, 1850. (0 citations)"),
        "{md}"
    );
    assert!(md.contains("*Birth certificate*. (1 citation)"), "{md}");
}

#[test]
fn map_endpoints_geocode_filter_and_export() {
    let mut s = Session::new();
    let ged = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n0 @I1@ INDI\n1 NAME Ali /Kaya/\n1 BIRT\n2 DATE 1800\n2 PLAC Konya, Turkey\n1 DEAT\n2 DATE 1870\n2 PLAC Ankara, Turkey\n0 @I2@ INDI\n1 NAME Zed /Living/\n1 BIRT\n2 DATE 2000\n2 PLAC Adana, Turkey\n0 TRLR\n";
    call(&mut s, "gedcom.import", json!({"data": B64.encode(ged)}));
    assert_eq!(
        call(&mut s, "map.points", json!({}))
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let rep = call(&mut s, "geo.offline", json!({}));
    assert!(rep["geocoded"].as_i64().unwrap() >= 3, "{rep}");
    let pts = call(&mut s, "map.points", json!({}));
    assert_eq!(
        pts.as_array().unwrap().len(),
        2,
        "living person hidden by default"
    );
    let all = call(&mut s, "map.points", json!({"hide_living": false}));
    assert_eq!(all.as_array().unwrap().len(), 3);
    assert_eq!(
        call(&mut s, "map.arcs", json!({}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        call(&mut s, "map.heat", json!({}))
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let kml = call(&mut s, "map.export", json!({"format": "kml"}));
    assert_eq!(kml["ext"], "kml");

    let places = call(&mut s, "place.list", json!({}));
    let konya = places
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Konya")
        .unwrap()["id"]
        .clone();
    assert_eq!(
        err(
            &mut s,
            "place.set_coords",
            json!({"id": konya, "lat": 123.0, "lon": 1.0})
        ),
        "bad_args"
    );
    call(
        &mut s,
        "place.set_coords",
        json!({"id": konya, "lat": 10.5, "lon": 20.5}),
    );
    let pts = call(&mut s, "map.points", json!({"kinds": ["BIRT"]}));
    assert_eq!(pts[0]["lat"], 10.5);
    call(&mut s, "place.set_coords", json!({"id": konya}));
    let pts = call(&mut s, "map.points", json!({"kinds": ["BIRT"]}));
    assert_eq!(pts[0]["inherited"], true, "falls back to the country");
    call(&mut s, "history.undo", json!({}));
    assert_eq!(
        call(&mut s, "map.points", json!({"kinds": ["BIRT"]}))[0]["lat"],
        10.5
    );
    assert!(call(&mut s, "geo.lookup", json!({"query": "Izmir, Turkey"}))["lat"].is_f64());
}

#[test]
fn media_import_dedupe_link_edit_and_undo() {
    let mut s = fresh();
    let p = call(
        &mut s,
        "person.create",
        json!({"given": "Ali", "surname": "Kaya", "sex": "M"}),
    )["id"]
        .clone();
    let png = {
        let img = image::RgbImage::from_pixel(64, 32, image::Rgb([1, 2, 3]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        B64.encode(out.into_inner())
    };
    let a = call(
        &mut s,
        "media.import",
        json!({"name": "ali.png", "data": png, "target_type": "person", "target_id": p}),
    );
    let again = call(
        &mut s,
        "media.import",
        json!({"name": "dup.png", "data": png}),
    );
    assert_eq!(again["duplicate"], true);
    assert_eq!(again["id"], a["id"]);
    let id = a["id"].clone();
    let t = call(&mut s, "media.thumb", json!({"id": id}));
    assert_eq!(t["mime"], "image/jpeg");
    assert_eq!(
        call(&mut s, "media.file", json!({"id": id}))["name"],
        "ali.png"
    );
    call(
        &mut s,
        "media.update",
        json!({"id": id, "caption": "Wedding", "date_text": "3 Mar 1950", "place_text": "Konya, Turkey"}),
    );
    let g = call(&mut s, "media.get", json!({"id": id}));
    assert_eq!(g["item"]["caption"], "Wedding");
    assert_eq!(g["item"]["date"], "3 March 1950");
    assert_eq!(g["place"], "Konya, Turkey");
    assert_eq!(g["links"][0]["label"], "Ali Kaya");
    call(
        &mut s,
        "media.set_primary",
        json!({"person_id": p, "media_id": id}),
    );
    assert_eq!(
        call(&mut s, "person.get", json!({"id": p}))["person"]["primary_media"],
        id
    );
    call(&mut s, "media.delete", json!({"id": id}));
    assert_eq!(
        call(&mut s, "media.list", json!({}))
            .as_array()
            .unwrap()
            .len(),
        0
    );
    call(&mut s, "history.undo", json!({}));
    assert_eq!(
        call(&mut s, "media.list", json!({"q": "ali"}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        err(&mut s, "media.file", json!({"id": "nope"})),
        "not_found"
    );
    assert_eq!(
        err(&mut s, "media.import", json!({"name": "x", "data": "@@"})),
        "bad_args"
    );
}

#[test]
fn stories_save_render_and_validate() {
    let mut s = fresh();
    let p = call(
        &mut s,
        "person.create",
        json!({"given": "Ali", "surname": "Kaya", "sex": "M"}),
    )["id"]
        .clone();
    call(
        &mut s,
        "event.put",
        json!({"owner_type": "person", "owner_id": p, "kind": "BIRT", "date_text": "1850", "place_text": "Konya"}),
    );
    assert_eq!(
        err(&mut s, "story.save", json!({"title": " ", "blocks": []})),
        "bad_args"
    );
    assert_eq!(
        err(
            &mut s,
            "story.save",
            json!({"title": "x", "blocks": [{"type": "bogus"}]})
        ),
        "bad_args"
    );
    let id = call(
        &mut s,
        "story.save",
        json!({"title": "Roots", "blocks": [
        {"type": "heading", "text": "Where we began"},
        {"type": "text", "text": "A long time ago."},
        {"type": "person", "id": p}]}),
    )["id"]
        .clone();
    assert_eq!(call(&mut s, "story.list", json!({}))[0]["blocks"], 3);
    let r = call(&mut s, "story.render", json!({"id": id, "lang": "en"}));
    let html = r["html"].as_str().unwrap();
    assert!(
        html.contains("Where we began") && html.contains("Ali Kaya was born in 1850 in Konya."),
        "{html}"
    );
    let tr = call(&mut s, "story.render", json!({"id": id, "lang": "tr"}));
    assert!(tr["html"].as_str().unwrap().contains("lang=\"tr\""));
    call(&mut s, "story.delete", json!({"id": id}));
    assert_eq!(err(&mut s, "story.get", json!({"id": id})), "not_found");
}
