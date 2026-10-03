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
            && d["partner_families"][0]["children"]
                .as_array()
                .unwrap()
                .len()
                >= 1
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
