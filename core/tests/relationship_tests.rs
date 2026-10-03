use kintree_core::kinship_terms::{describe, describe_all, Lang};
use kintree_core::name::PersonName;
use kintree_core::relationship::*;
use kintree_core::store::{Store, Tx};
use std::collections::HashMap;

struct T {
    s: Store,
    id: HashMap<String, String>,
}

impl T {
    fn new() -> T {
        T {
            s: Store::open_memory().unwrap(),
            id: HashMap::new(),
        }
    }
    fn build(&mut self, f: impl FnOnce(&mut B)) {
        let id = &mut self.id;
        self.s
            .transact("build", |tx| {
                let mut b = B { tx, id };
                f(&mut b);
                Ok(())
            })
            .unwrap();
    }
    fn g(&self) -> Graph {
        Graph::load(&self.s).unwrap()
    }
    fn rel(&self, a: &str, b: &str, lang: Lang) -> String {
        let g = self.g();
        describe_all(&relationship(&g, &self.id[a], &self.id[b]), lang).join(" | ")
    }
}

struct B<'a, 'b> {
    tx: &'a mut Tx<'b>,
    id: &'a mut HashMap<String, String>,
}
impl B<'_, '_> {
    fn p(&mut self, name: &str, sex: &str) {
        let i = self
            .tx
            .create_person(&PersonName::new(name, "X"), sex)
            .unwrap();
        self.id.insert(name.into(), i);
    }
    /// family with partners and children
    fn fam(&mut self, h: &str, w: &str, kids: &[&str]) {
        let f = self
            .tx
            .create_family(
                Some(&self.id[h].clone()),
                Some(&self.id[w].clone()),
                "married",
            )
            .unwrap();
        for k in kids {
            self.tx.add_child(&f, &self.id[*k].clone(), "").unwrap();
        }
    }
    fn fam_kind(&mut self, h: &str, w: &str, kid: &str, kind: &str) {
        let f = self
            .tx
            .create_family(
                Some(&self.id[h].clone()),
                Some(&self.id[w].clone()),
                "married",
            )
            .unwrap();
        self.tx.add_child(&f, &self.id[kid].clone(), kind).unwrap();
    }
}

fn clan() -> T {
    let mut t = T::new();
    t.build(|b| {
        for (n, s) in [
            ("PGF", "M"),
            ("PGM", "F"),
            ("Dad", "M"),
            ("Aunt", "F"),
            ("Uncle", "M"),
            ("AuntHusb", "M"),
            ("UncleWife", "F"),
            ("MGF", "M"),
            ("MGM", "F"),
            ("Mom", "F"),
            ("MomSis", "F"),
            ("MomBro", "M"),
            ("MomSisHusb", "M"),
            ("Me", "M"),
            ("Sis", "F"),
            ("CousinA", "M"),
            ("CousinU", "F"),
            ("CousinM", "F"),
            ("Wife", "F"),
            ("WifeSis", "F"),
            ("WifeFather", "M"),
            ("WifeMother", "F"),
            ("WifeSisHusb", "M"),
            ("BroWife", "F"),
            ("SisHusb", "M"),
            ("Bro", "M"),
            ("Kid", "M"),
            ("KidWife", "F"),
            ("GKid", "F"),
            ("PGGF", "M"),
            ("PGGM", "F"),
        ] {
            b.p(n, s);
        }
        b.fam("PGGF", "PGGM", &["PGF"]);
        b.fam("PGF", "PGM", &["Dad", "Aunt", "Uncle"]);
        b.fam("MGF", "MGM", &["Mom", "MomSis", "MomBro"]);
        b.fam("Dad", "Mom", &["Me", "Sis", "Bro"]);
        b.fam("AuntHusb", "Aunt", &["CousinA"]);
        b.fam("Uncle", "UncleWife", &["CousinU"]);
        b.fam("MomSisHusb", "MomSis", &["CousinM"]);
        b.fam("Me", "Wife", &["Kid"]);
        b.fam("WifeFather", "WifeMother", &["Wife", "WifeSis"]);
        b.fam("WifeSisHusb", "WifeSis", &[]);
        b.fam("Bro", "BroWife", &[]);
        b.fam("SisHusb", "Sis", &[]);
        b.fam("Kid", "KidWife", &["GKid"]);
    });
    t
}

#[test]
fn english_terms() {
    let t = clan();
    let r = |a, b| t.rel(a, b, Lang::En);
    assert_eq!(r("Me", "Dad"), "father");
    assert_eq!(r("Dad", "Me"), "son");
    assert_eq!(r("Me", "Sis"), "sister");
    assert_eq!(r("Me", "PGM"), "grandmother");
    assert_eq!(r("Me", "PGGF"), "great-grandfather");
    assert_eq!(r("PGGF", "GKid"), "great-great-great-granddaughter");
    assert_eq!(r("Me", "Aunt"), "aunt");
    assert_eq!(r("Me", "MomBro"), "uncle");
    assert_eq!(r("Aunt", "Me"), "nephew");
    assert_eq!(r("Uncle", "Sis"), "niece");
    assert_eq!(r("Me", "CousinA"), "1st cousin");
    assert_eq!(r("Me", "GKid"), "granddaughter");
    assert_eq!(r("Kid", "CousinU"), "1st cousin once removed");
    assert_eq!(r("GKid", "CousinU"), "1st cousin twice removed");
    assert_eq!(r("Me", "Wife"), "wife");
    assert_eq!(r("Wife", "Dad"), "father-in-law");
    assert_eq!(r("Me", "WifeFather"), "father-in-law");
    assert_eq!(r("Me", "WifeSis"), "sister-in-law");
    assert_eq!(r("Me", "SisHusb"), "brother-in-law");
    assert_eq!(r("Me", "KidWife"), "daughter-in-law");
    assert_eq!(r("Me", "Me"), "self");
    assert_eq!(r("Me", "WifeSisHusb"), "brother-in-law");
}

#[test]
fn turkish_terms_distinguish_paternal_and_maternal_kin() {
    let t = clan();
    let r = |a, b| t.rel(a, b, Lang::Tr);
    assert_eq!(r("Me", "Dad"), "baba");
    assert_eq!(r("Me", "Mom"), "anne");
    assert_eq!(r("Me", "Uncle"), "amca");
    assert_eq!(r("Me", "Aunt"), "hala");
    assert_eq!(r("Me", "MomBro"), "dayı");
    assert_eq!(r("Me", "MomSis"), "teyze");
    assert_eq!(r("Me", "PGM"), "babaanne");
    assert_eq!(r("Me", "MGM"), "anneanne");
    assert_eq!(r("Me", "PGF"), "dede");
    assert_eq!(r("Me", "MGF"), "dede");
    assert_eq!(r("Me", "Sis"), "kız kardeş");
    assert_eq!(r("Me", "Bro"), "erkek kardeş");
    assert_eq!(r("Me", "CousinU"), "amca kızı");
    assert_eq!(r("Me", "CousinA"), "hala oğlu");
    assert_eq!(r("Me", "CousinM"), "teyze kızı");
    assert_eq!(r("Uncle", "Me"), "yeğen");
    assert_eq!(r("Me", "Wife"), "karı");
    assert_eq!(r("Me", "WifeFather"), "kayınpeder");
    assert_eq!(r("Me", "WifeMother"), "kayınvalide");
    assert_eq!(r("Me", "WifeSis"), "baldız");
    assert_eq!(r("Me", "BroWife"), "yenge");
    assert_eq!(r("Me", "SisHusb"), "enişte");
    assert_eq!(r("Me", "KidWife"), "gelin");
    assert_eq!(r("Me", "WifeSisHusb"), "bacanak");
    assert_eq!(r("Me", "PGGF"), "büyük dede");
    assert_eq!(r("Me", "AuntHusb"), "enişte");
}

#[test]
fn other_languages_core_terms() {
    let t = clan();
    for (lang, father, grandmother, uncle) in [
        (Lang::De, "Vater", "Großmutter", "Onkel"),
        (Lang::Es, "padre", "abuela", "tío"),
        (Lang::Fr, "père", "grand-mère", "oncle"),
        (Lang::Ru, "отец", "бабушка", "дядя"),
        (Lang::Ar, "أب", "جدة", "عم"),
    ] {
        assert_eq!(t.rel("Me", "Dad", lang), father, "{lang:?}");
        assert_eq!(t.rel("Me", "PGM", lang), grandmother, "{lang:?}");
        assert_eq!(t.rel("Me", "Uncle", lang), uncle, "{lang:?}");
    }
    assert_eq!(t.rel("Me", "MomBro", Lang::Ar), "خال");
    assert_eq!(t.rel("Me", "CousinU", Lang::Ru), "двоюродная сестра");
    assert_eq!(t.rel("Wife", "Dad", Lang::Ru), "свёкор");
    assert_eq!(t.rel("Me", "WifeFather", Lang::Ru), "тесть");
}

#[test]
fn half_siblings_adoption_and_step() {
    let mut t = T::new();
    t.build(|b| {
        for (n, s) in [
            ("A", "M"),
            ("B", "F"),
            ("C", "F"),
            ("X", "M"),
            ("Y", "F"),
            ("Ad", "M"),
            ("StepKid", "F"),
        ] {
            b.p(n, s);
        }
        b.fam("A", "B", &["X"]);
        b.fam("A", "C", &["Y"]);
        b.fam_kind("A", "B", "Ad", "adopted");
        b.fam("StepKid", "C", &[]); // StepKid is C's partner, not relevant
    });
    assert_eq!(t.rel("X", "Y", Lang::En), "half-sister");
    assert_eq!(t.rel("X", "Y", Lang::Tr), "baba bir kız kardeş");
    assert_eq!(t.rel("X", "Y", Lang::De), "Halbschwester");
    assert_eq!(t.rel("X", "Y", Lang::Ru), "единокровная сестра");
    assert_eq!(t.rel("A", "Ad", Lang::En), "adoptive son");
    // step relations: C is A's partner; Y... X is not C's child
    assert_eq!(t.rel("C", "X", Lang::En), "stepson");
    assert_eq!(t.rel("X", "C", Lang::En), "stepmother");
}

#[test]
fn pedigree_collapse_reports_multiple_paths() {
    // Uncle R marries niece D (daughter of his brother Q); their child P is related to Q in two ways.
    let mut t = T::new();
    t.build(|b| {
        for (n, s) in [
            ("G", "M"),
            ("GW", "F"),
            ("Q", "M"),
            ("QW", "F"),
            ("R", "M"),
            ("D", "F"),
            ("P", "M"),
        ] {
            b.p(n, s);
        }
        b.fam("G", "GW", &["Q", "R"]);
        b.fam("Q", "QW", &["D"]);
        b.fam("R", "D", &["P"]);
    });
    assert_eq!(t.rel("P", "Q", Lang::En), "grandfather | uncle");
    let g = t.g();
    let shapes: Vec<(usize, usize)> = relationship(&g, &t.id["P"], &t.id["Q"])
        .iter()
        .filter_map(|r| {
            if let Kind::Blood(b) = &r.kind {
                Some((b.up, b.down))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(shapes, vec![(2, 0), (2, 1)]);
    // Double first cousins share a couple of lowest ancestors -> one relation, full (not half).
    let mut t2 = T::new();
    t2.build(|b| {
        for (n, s) in [
            ("G1", "M"),
            ("G2", "F"),
            ("B1", "M"),
            ("B2", "M"),
            ("S1", "F"),
            ("S2", "F"),
            ("K1", "M"),
            ("K2", "F"),
        ] {
            b.p(n, s);
        }
        b.fam("G1", "G2", &["B1", "B2", "S1", "S2"]);
        b.fam("B1", "S1", &["K1"]);
        b.fam("B2", "S2", &["K2"]);
    });
    let g2 = t2.g();
    let rels = relationship(&g2, &t2.id["K1"], &t2.id["K2"]);
    assert_eq!(rels.len(), 1);
    let Kind::Blood(b) = &rels[0].kind else {
        panic!()
    };
    assert_eq!(
        (b.up, b.down, b.ancestors.len(), b.half()),
        (2, 2, 2, false)
    );
}

#[test]
fn inbreeding_coefficients() {
    let mut t = T::new();
    t.build(|b| {
        for (n, s) in [
            ("A", "M"),
            ("B", "F"),
            ("C", "M"),
            ("D", "F"),
            ("E", "F"),
            ("F_", "M"),
            ("G", "M"),
            ("H", "F"),
            ("I", "F"),
            ("A2", "M"),
            ("C2", "F"),
            ("X", "M"),
            ("Y", "F"),
            ("Z", "M"),
            ("U", "M"),
        ] {
            b.p(n, s);
        }
        // full siblings C, D produce child E: F = 0.25
        b.fam("A", "B", &["C", "D"]);
        b.fam("C", "D", &["E"]);
        // first cousins: G,H grandkids
        b.fam("A2", "C2", &["X", "Y"]);
        b.fam("X", "E", &["G"]);
        b.fam("Y", "F_", &["H"]);
        b.fam("G", "H", &["I"]);
        // unrelated
        b.fam("U", "Z", &[]);
    });
    // Rebuild a cleaner first-cousin case independent of the above.
    let g = t.g();
    let mut k = Kinship::new(&g);
    let e = k.inbreeding(&t.id["E"]);
    assert!((e - 0.25).abs() < 1e-12, "sibling mating F={e}");
    assert_eq!(k.inbreeding(&t.id["C"]), 0.0);

    let mut t2 = T::new();
    t2.build(|b| {
        for (n, s) in [
            ("A", "M"),
            ("B", "F"),
            ("C", "M"),
            ("D", "F"),
            ("P", "F"),
            ("Q", "M"),
            ("G", "M"),
            ("H", "F"),
            ("I", "F"),
        ] {
            b.p(n, s);
        }
        b.fam("A", "B", &["C", "D"]);
        b.fam("C", "P", &["G"]);
        b.fam("Q", "D", &["H"]);
        b.fam("G", "H", &["I"]);
    });
    let g2 = t2.g();
    let mut k2 = Kinship::new(&g2);
    let f = k2.inbreeding(&t2.id["I"]);
    assert!((f - 0.0625).abs() < 1e-12, "first cousins F={f}");
    // relatedness of siblings = 0.5, first cousins = 0.125
    assert!((k2.relatedness(&t2.id["C"], &t2.id["D"]) - 0.5).abs() < 1e-12);
    assert!((k2.relatedness(&t2.id["G"], &t2.id["H"]) - 0.125).abs() < 1e-12);
}

#[test]
fn half_sibling_mating_inbreeding_is_one_eighth() {
    let mut t = T::new();
    t.build(|b| {
        for (n, s) in [
            ("A", "M"),
            ("B", "F"),
            ("C", "F"),
            ("X", "M"),
            ("Y", "F"),
            ("K", "M"),
        ] {
            b.p(n, s);
        }
        b.fam("A", "B", &["X"]);
        b.fam("A", "C", &["Y"]);
        b.fam("X", "Y", &["K"]);
    });
    let g = t.g();
    let f = Kinship::new(&g).inbreeding(&t.id["K"]);
    assert!((f - 0.125).abs() < 1e-12, "{f}");
}

#[test]
fn ahnentafel_numbering() {
    let t = clan();
    let g = t.g();
    let ahn = ahnentafel(&g, &t.id["Me"], 3);
    let m: HashMap<u64, &str> = ahn.iter().map(|(n, id)| (*n, id.as_str())).collect();
    assert_eq!(m[&1], t.id["Me"]);
    assert_eq!(m[&2], t.id["Dad"]);
    assert_eq!(m[&3], t.id["Mom"]);
    assert_eq!(m[&4], t.id["PGF"]);
    assert_eq!(m[&5], t.id["PGM"]);
    assert_eq!(m[&6], t.id["MGF"]);
    assert_eq!(m[&7], t.id["MGM"]);
    assert_eq!(m[&8], t.id["PGGF"]);
    assert!(!m.contains_key(&16), "generation limit respected");
    assert!(g.find_cycles().is_empty());
}

#[test]
fn real_cycle_is_detected() {
    // A is parent of B, B is parent of A.
    let mut t = T::new();
    t.build(|b| {
        b.p("A", "M");
        b.p("B", "M");
        b.p("W1", "F");
        b.p("W2", "F");
        b.fam("A", "W1", &["B"]);
        b.fam("B", "W2", &["A"]);
    });
    let c = t.g().find_cycles();
    assert!(!c.is_empty());
    // relationship queries on cyclic data must terminate
    let g = t.g();
    let _ = relationship(&g, &t.id["A"], &t.id["B"]);
    let _ = Kinship::new(&g).inbreeding(&t.id["A"]);
}

#[test]
fn descendant_numbering_systems() {
    let mut t = T::new();
    t.build(|b| {
        for (n, s) in [
            ("R", "M"),
            ("W", "F"),
            ("C1", "M"),
            ("C2", "F"),
            ("G1", "M"),
            ("G2", "F"),
            ("CW", "F"),
        ] {
            b.p(n, s);
        }
        b.fam("R", "W", &["C1", "C2"]);
        b.fam("C1", "CW", &["G1", "G2"]);
    });
    let g = t.g();
    let births = HashMap::new();
    let dab = number_descendants(&g, &births, &t.id["R"], 5, DescendantNumbering::DAboville);
    let nums: Vec<&str> = dab.iter().map(|x| x.0.as_str()).collect();
    assert_eq!(nums, ["1", "1.1", "1.2", "1.1.1", "1.1.2"]);
    let henry = number_descendants(&g, &births, &t.id["R"], 5, DescendantNumbering::Henry);
    let nums: Vec<&str> = henry.iter().map(|x| x.0.as_str()).collect();
    assert_eq!(nums, ["1", "11", "12", "111", "112"]);
    let _ = describe;
}
