//! Deterministic synthetic family-tree generator (GEDCOM text) for demos, tests and benchmarks.

const GIVEN_M: &[&str] = &[
    "Ali", "Mehmet", "Ahmet", "John", "William", "Hans", "Pierre", "Mustafa", "Ibrahim", "Robert",
    "Jan", "Carlos", "Ivan", "Emre", "Hasan",
];
const GIVEN_F: &[&str] = &[
    "Ayşe",
    "Fatma",
    "Zeynep",
    "Mary",
    "Anna",
    "Marie",
    "Elif",
    "Hatice",
    "Elizabeth",
    "Greta",
    "Maria",
    "Olga",
    "Selin",
    "Emine",
    "Sarah",
];
const SURNAMES: &[&str] = &[
    "Yılmaz", "Kaya", "Demir", "Çelik", "Şahin", "Smith", "Jones", "Müller", "Dupont", "García",
    "Ivanov", "Öztürk", "Aydın", "Brown", "Rossi",
];
const PLACES: &[&str] = &[
    "Konya, Konya Province, Turkey",
    "Istanbul, Turkey",
    "Ankara, Turkey",
    "İzmir, Turkey",
    "Berlin, Germany",
    "Paris, France",
    "London, England",
    "Madrid, Spain",
    "Moscow, Russia",
    "New York, USA",
];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a>(&mut self, a: &'a [&'a str]) -> &'a str {
        a[self.below(a.len())]
    }
}

struct P {
    sex: char,
    given: String,
    surname: String,
    birth: i32,
    famc: Option<usize>,
    fams: Vec<usize>,
}
struct F {
    husb: usize,
    wife: usize,
    children: Vec<usize>,
    marr: i32,
}

/// Generate roughly `target` persons spread over generations, with sources, places and notes.
pub fn generate_gedcom(target: usize, seed: u64) -> String {
    let mut rng = Rng(seed ^ 0x9E3779B97F4A7C15);
    let mut persons: Vec<P> = Vec::new();
    let mut fams: Vec<F> = Vec::new();
    let new_person = |rng: &mut Rng,
                      persons: &mut Vec<P>,
                      sex: char,
                      surname: &str,
                      birth: i32,
                      famc: Option<usize>|
     -> usize {
        let given = if sex == 'M' {
            rng.pick(GIVEN_M)
        } else {
            rng.pick(GIVEN_F)
        }
        .to_string();
        persons.push(P {
            sex,
            given,
            surname: surname.to_string(),
            birth,
            famc,
            fams: vec![],
        });
        persons.len() - 1
    };
    // Founders: each founder couple starts a lineage in 1650..1700.
    let mut frontier: Vec<usize> = Vec::new();
    while persons.len() < target {
        if frontier.is_empty() {
            let sn = rng.pick(SURNAMES).to_string();
            let b = 1650 + rng.below(60) as i32;
            let h = new_person(&mut rng, &mut persons, 'M', &sn, b, None);
            let w_sn = rng.pick(SURNAMES).to_string();
            let wb = b + rng.below(5) as i32;
            let w = new_person(&mut rng, &mut persons, 'F', &w_sn, wb, None);
            frontier.push(add_family(&mut fams, &mut persons, h, w, b + 22));
            continue;
        }
        let fi = frontier.remove(rng.below(frontier.len()));
        let (husb, marr) = (fams[fi].husb, fams[fi].marr);
        let nkids = 1 + rng.below(4);
        let sn = persons[husb].surname.clone();
        for k in 0..nkids {
            if persons.len() >= target {
                break;
            }
            let birth = marr + 1 + (k as i32) * 2 + rng.below(2) as i32;
            let sex = if rng.below(2) == 0 { 'M' } else { 'F' };
            let kid = new_person(&mut rng, &mut persons, sex, &sn, birth, Some(fi));
            fams[fi].children.push(kid);
            // ~70% of children marry (if old enough in the model's timeline) and start new families.
            if birth < 1960 && rng.below(10) < 7 && persons.len() + 1 < target {
                let spouse_sex = if sex == 'M' { 'F' } else { 'M' };
                let spouse_sn = rng.pick(SURNAMES).to_string();
                let sb = birth + rng.below(6) as i32 - 2;
                let spouse = new_person(&mut rng, &mut persons, spouse_sex, &spouse_sn, sb, None);
                let (h, w) = if sex == 'M' {
                    (kid, spouse)
                } else {
                    (spouse, kid)
                };
                let nf = add_family(
                    &mut fams,
                    &mut persons,
                    h,
                    w,
                    birth + 20 + rng.below(8) as i32,
                );
                frontier.push(nf);
            }
        }
    }
    let mut out = String::with_capacity(target * 220);
    out.push_str("0 HEAD\n1 SOUR KinTree-Synth\n2 VERS 1\n1 GEDC\n2 VERS 5.5.1\n2 FORM LINEAGE-LINKED\n1 CHAR UTF-8\n1 SUBM @U1@\n0 @U1@ SUBM\n1 NAME Synthetic\n");
    for (i, p) in persons.iter().enumerate() {
        out.push_str(&format!(
            "0 @I{}@ INDI\n1 NAME {} /{}/\n2 GIVN {}\n2 SURN {}\n1 SEX {}\n",
            i + 1,
            p.given,
            p.surname,
            p.given,
            p.surname,
            p.sex
        ));
        let place = PLACES[(i * 7 + p.birth as usize) % PLACES.len()];
        out.push_str(&format!(
            "1 BIRT\n2 DATE {} {} {}\n2 PLAC {}\n",
            1 + (i % 28),
            ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"]
                [i % 12],
            p.birth,
            place
        ));
        if p.birth < 1930 {
            out.push_str(&format!(
                "1 DEAT\n2 DATE {}\n2 PLAC {}\n",
                p.birth + 40 + (i % 45) as i32,
                PLACES[(i * 3) % PLACES.len()]
            ));
        }
        if i % 5 == 0 {
            out.push_str(&format!(
                "1 OCCU {}\n",
                ["Farmer", "Teacher", "Merchant", "Smith", "Clerk"][(i / 5) % 5]
            ));
        }
        if i % 40 == 0 {
            out.push_str(
                "1 NOTE A synthetic note.\n2 CONT With a second line.\n1 SOUR @S1@\n2 PAGE 1\n",
            );
        }
        if let Some(f) = p.famc {
            out.push_str(&format!("1 FAMC @F{}@\n", f + 1));
        }
        for f in &p.fams {
            out.push_str(&format!("1 FAMS @F{}@\n", f + 1));
        }
    }
    for (i, f) in fams.iter().enumerate() {
        out.push_str(&format!(
            "0 @F{}@ FAM\n1 HUSB @I{}@\n1 WIFE @I{}@\n",
            i + 1,
            f.husb + 1,
            f.wife + 1
        ));
        for c in &f.children {
            out.push_str(&format!("1 CHIL @I{}@\n", c + 1));
        }
        out.push_str(&format!("1 MARR\n2 DATE {}\n", f.marr));
    }
    out.push_str("0 @S1@ SOUR\n1 TITL Synthetic Register\n0 TRLR\n");
    out
}

fn add_family(fams: &mut Vec<F>, persons: &mut [P], h: usize, w: usize, marr: i32) -> usize {
    fams.push(F {
        husb: h,
        wife: w,
        children: vec![],
        marr,
    });
    let idx = fams.len() - 1;
    persons[h].fams.push(idx);
    persons[w].fams.push(idx);
    idx
}
