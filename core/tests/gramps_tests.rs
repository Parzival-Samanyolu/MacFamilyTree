use flate2::write::GzEncoder;
use flate2::Compression;
use kintree_core::gedcom::{self, ExportOptions};
use kintree_core::gramps;
use kintree_core::store::Store;
use std::io::Write;

const SAMPLE: &str = include_str!("../../samples/gramps/kaya.gramps");

fn person(s: &Store, given: &str) -> String {
    s.rows("person_name")
        .unwrap()
        .into_iter()
        .find(|n| n["given"] == given)
        .unwrap_or_else(|| panic!("no {given}"))["person_id"]
        .as_str()
        .unwrap()
        .to_string()
}
fn events(s: &Store, owner: &str) -> Vec<kintree_core::store::Row> {
    s.rows_where("event", "owner_id", owner).unwrap()
}

#[test]
fn detects_gramps_files() {
    assert!(gramps::looks_like_gramps(SAMPLE.as_bytes()));
    assert!(gramps::looks_like_gramps(&[0x1F, 0x8B, 8]));
    assert!(!gramps::looks_like_gramps(b"0 HEAD\n1 GEDC\n"));
}

#[test]
fn imports_people_families_events_places_sources_and_notes() {
    let ged = gramps::to_gedcom(SAMPLE.as_bytes()).unwrap();
    let mut s = Store::open_memory().unwrap();
    let rep = gedcom::import(&mut s, ged.as_bytes()).unwrap();
    assert_eq!((rep.persons, rep.families), (3, 1), "{ged}");
    let ali = person(&s, "Ali");
    let names = s.rows_where("person_name", "person_id", &ali).unwrap();
    assert_eq!(
        (names[0]["surname"].as_str(), names[0]["nickname"].as_str()),
        (Some("Kaya"), Some("Aliş"))
    );
    let can = person(&s, "Can");
    assert_eq!(
        s.rows_where("person_name", "person_id", &can)
            .unwrap()
            .len(),
        2,
        "alternate name kept"
    );
    assert_eq!(
        s.rows("person")
            .unwrap()
            .iter()
            .find(|p| p["id"] == ali.as_str())
            .unwrap()["sex"],
        "M"
    );

    let ev = events(&s, &ali);
    let kinds: Vec<&str> = ev.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    for k in ["BIRT", "DEAT", "EVEN", "RESI"] {
        assert!(kinds.contains(&k), "{k} in {kinds:?}");
    }
    let birth = ev.iter().find(|e| e["kind"] == "BIRT").unwrap();
    let place = s
        .rows("place")
        .unwrap()
        .into_iter()
        .find(|p| p["id"] == birth["place_id"])
        .unwrap();
    assert_eq!(place["name"], "Konya");
    assert_eq!(place["lat"], 37.87);
    let names = kintree_core::places::full_names(&s).unwrap();
    assert_eq!(names[place["id"].as_str().unwrap()], "Konya, Turkey");
    let death = ev.iter().find(|e| e["kind"] == "DEAT").unwrap();
    assert!(
        death["date_json"].as_str().unwrap().contains("About"),
        "{:?}",
        death["date_json"]
    );
    let custom = ev.iter().find(|e| e["kind"] == "EVEN").unwrap();
    assert_eq!(custom["custom_kind"], "Apprenticeship");
    assert!(custom["date_json"].as_str().unwrap().contains("1865"));
    let flood = ev.iter().find(|e| e["kind"] == "RESI").unwrap();
    assert!(
        flood["date_json"].as_str().unwrap().contains("flood"),
        "free-text dates are kept as phrases"
    );

    // family: partners, child, marriage range
    let fam = &s.rows("family").unwrap()[0];
    assert_eq!(fam["partner1"], ali.as_str());
    assert_eq!(
        s.rows("family_child").unwrap()[0]["person_id"],
        can.as_str()
    );
    let marr = events(&s, fam["id"].as_str().unwrap());
    assert_eq!(marr[0]["kind"], "MARR");
    // source, citation page, note
    assert_eq!(
        s.rows("source").unwrap()[0]["title"],
        "Konya parish register"
    );
    assert!(s
        .rows("citation")
        .unwrap()
        .iter()
        .any(|c| c["page"] == "Folio 12"));
    assert!(s.rows("note").unwrap().iter().any(|n| n["body"]
        .as_str()
        .unwrap()
        .contains("Weaver and\n storyteller.")));
    // and it exports as valid GEDCOM again
    let out = gedcom::export(&s, &ExportOptions::default()).unwrap();
    assert!(String::from_utf8(out)
        .unwrap()
        .contains("Konya parish register"));
}

#[test]
fn gzip_compressed_files_and_bad_input() {
    let mut z = GzEncoder::new(Vec::new(), Compression::fast());
    z.write_all(SAMPLE.as_bytes()).unwrap();
    let gz = z.finish().unwrap();
    assert_eq!(
        gramps::to_gedcom(&gz).unwrap(),
        gramps::to_gedcom(SAMPLE.as_bytes()).unwrap()
    );
    assert!(gramps::to_gedcom(b"<html/>").is_err());
    assert!(gramps::to_gedcom(b"<database xmlns=\"x\"><events></database>").is_err());
    assert!(gramps::to_gedcom(&[0x1F, 0x8B, 1, 2, 3]).is_err());
    let minimal =
        gramps::to_gedcom(b"<database xmlns=\"http://gramps-project.org/xml/1.7.1/\"/>").unwrap();
    assert!(minimal.starts_with("0 HEAD") && minimal.trim_end().ends_with("0 TRLR"));
}

#[test]
fn negative_dates_and_coordinates_in_the_southern_hemisphere() {
    let xml = r#"<database xmlns="http://gramps-project.org/xml/1.7.1/">
<events><event handle="_e"><type>Birth</type><dateval val="-0044-03-15" type="before"/><place hlink="_p"/></event></events>
<people><person handle="_i"><gender>U</gender><name><first>Gaius</first><surname>Julius</surname></name><eventref hlink="_e"/></person></people>
<places><placeobj handle="_p"><pname value="Somewhere"/><coord lat="-33.9" long="-18.4"/></placeobj></places></database>"#;
    let ged = gramps::to_gedcom(xml.as_bytes()).unwrap();
    assert!(ged.contains("2 DATE BEF 15 MAR 44 BC"), "{ged}");
    assert!(
        ged.contains("LATI S33.9") && ged.contains("LONG W18.4"),
        "{ged}"
    );
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, ged.as_bytes()).unwrap();
    assert_eq!(s.rows("place").unwrap()[0]["lat"], -33.9);
}
