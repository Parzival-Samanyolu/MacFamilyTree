use kintree_core::gedcom;
use kintree_core::geo::*;
use kintree_core::places::full_names;
use kintree_core::store::Store;

fn load(body: &str) -> Store {
    let mut s = Store::open_memory().unwrap();
    gedcom::import(
        &mut s,
        format!("0 HEAD\n1 GEDC\n2 VERS 5.5.1\n{body}0 TRLR\n").as_bytes(),
    )
    .unwrap();
    s
}

const TREE: &str = "\
0 @I1@ INDI\n1 NAME Ali /Kaya/\n1 SEX M\n1 BIRT\n2 DATE 10 MAR 1800\n2 PLAC Konya, Turkey\n1 DEAT\n2 DATE 11 MAR 1870\n2 PLAC Istanbul, Turkey\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Ayşe /Demir/\n1 SEX F\n1 BIRT\n2 DATE 1805\n2 PLAC Kadıköy, Istanbul, Turkey\n1 DEAT\n2 DATE 1880\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Can /Kaya/\n1 SEX M\n1 BIRT\n2 DATE 15 JUN 1830\n2 PLAC Adana, Turkey\n1 DEAT\n2 DATE 1900\n2 PLAC Nowhereville\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n";

fn geocoded() -> Store {
    let mut s = load(TREE);
    let names = full_names(&s).unwrap();
    s.transact("geocode", |tx| geocode_offline(tx, &names))
        .unwrap();
    s
}

#[test]
fn gazetteer_loads_and_resolves_aliases_and_diacritics() {
    let g = Gazetteer::embedded();
    assert!(g.len() > 250);
    let parts = |s: &str| s.split(", ").map(String::from).collect::<Vec<_>>();
    let (la, lo, i) = g.lookup(&parts("Constantinople, Turkey")).unwrap();
    assert_eq!(i, 0);
    assert!((la - 41.01).abs() < 0.01 && (lo - 28.98).abs() < 0.01);
    assert_eq!(g.lookup(&parts("istanbul")).unwrap().2, 0);
    assert!(g.lookup(&parts("Nowhereville")).is_none());
}

#[test]
fn offline_geocoding_fills_known_places_only() {
    let s = geocoded();
    let rows = s.rows("place").unwrap();
    let get = |n: &str| rows.iter().find(|r| r["name"] == n).unwrap().clone();
    assert!(get("Konya")["lat"].as_f64().is_some());
    assert_eq!(get("Konya")["geocode_status"], "offline");
    assert!(get("Nowhereville")["lat"].is_null());
    // Kadıköy has no gazetteer row of its own but inherits from İstanbul.
    assert!(get("Kadıköy")["lat"].is_null());
    let co = effective_coords(&s).unwrap();
    let k = co[get("Kadıköy")["id"].as_str().unwrap()];
    assert!(k.2, "inherited");
    assert!((k.0 - 41.01).abs() < 0.01);
}

#[test]
fn geocoding_is_idempotent_and_keeps_manual_coordinates() {
    let mut s = geocoded();
    let konya = s
        .rows("place")
        .unwrap()
        .into_iter()
        .find(|r| r["name"] == "Konya")
        .unwrap();
    let id = konya["id"].as_str().unwrap().to_string();
    s.transact("manual", |tx| {
        let mut r = tx.get("place", &id)?.unwrap();
        r.insert("lat".into(), 1.0.into());
        r.insert("lon".into(), 2.0.into());
        tx.put_row("place", r)?;
        Ok(())
    })
    .unwrap();
    let names = full_names(&s).unwrap();
    let rep = s.transact("g2", |tx| geocode_offline(tx, &names)).unwrap();
    assert_eq!(rep.geocoded, 0);
    assert!(rep.unresolved.iter().any(|u| u.contains("Nowhereville")));
    assert_eq!(
        s.rows("place")
            .unwrap()
            .into_iter()
            .find(|r| r["id"] == id.as_str())
            .unwrap()["lat"],
        1.0
    );
}

#[test]
fn points_filter_by_kind_and_years() {
    let s = geocoded();
    let all = points(&s, &Filter::default()).unwrap();
    assert!(all.iter().all(|p| valid_coords(p.lat, p.lon)));
    assert!(all.windows(2).all(|w| w[0].year <= w[1].year));
    let births = points(
        &s,
        &Filter {
            kinds: vec!["BIRT".into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(births.len(), 3);
    let early = points(
        &s,
        &Filter {
            year_to: Some(1810),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(early.iter().all(|p| p.year.unwrap() <= 1810));
    assert_eq!(early.len(), 2);
}

#[test]
fn route_heat_and_arcs() {
    let s = geocoded();
    let ali = s
        .rows("person_name")
        .unwrap()
        .into_iter()
        .find(|n| n["given"] == "Ali")
        .unwrap()["person_id"]
        .as_str()
        .unwrap()
        .to_string();
    let r = route(&s, &ali, &Filter::default()).unwrap();
    assert_eq!(
        r.iter().map(|p| p.kind.as_str()).collect::<Vec<_>>(),
        ["BIRT", "DEAT"]
    );
    let h = heat(&s, &Filter::default()).unwrap();
    let total: usize = h.iter().map(|c| c.count).sum();
    assert_eq!(total, points(&s, &Filter::default()).unwrap().len());
    assert!(h.windows(2).all(|w| w[0].count >= w[1].count));
    let life = arcs(&s, &Filter::default(), false).unwrap();
    assert_eq!(life.len(), 1);
    assert_eq!(life[0].from_place, "Konya, Turkey");
    let gen = arcs(&s, &Filter::default(), true).unwrap();
    assert!(gen.iter().any(|a| a.to_place.starts_with("Adana")));
}

#[test]
fn exports_are_well_formed() {
    let s = geocoded();
    let pts = points(&s, &Filter::default()).unwrap();
    let gj = to_geojson(&pts);
    assert_eq!(gj["features"].as_array().unwrap().len(), pts.len());
    assert_eq!(gj["features"][0]["geometry"]["coordinates"][0], pts[0].lon);
    let kml = to_kml(&pts, "A & B <test>");
    assert!(kml.contains("A &amp; B &lt;test&gt;"));
    assert_eq!(kml.matches("<Placemark>").count(), pts.len());
    assert!(haversine_km((41.01, 28.98), (39.93, 32.86)) > 300.0);
}

#[test]
fn country_aliases_disambiguate_cities() {
    let g = Gazetteer::embedded();
    let parts: Vec<String> = ["Konya", "Türkiye"].iter().map(|s| s.to_string()).collect();
    assert_eq!(g.lookup(&parts).map(|r| r.2), Some(0));
}
