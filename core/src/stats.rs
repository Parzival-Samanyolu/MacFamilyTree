//! Tree statistics with drill-down (every bucket carries the person ids behind it).

use crate::date::jdn_to_gregorian;
use crate::facts::{years_between, Facts};
use crate::relationship::Sex;
use crate::store::{Result, Store};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Bucket {
    pub label: String,
    pub count: usize,
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Counts {
    pub persons: usize,
    pub families: usize,
    pub events: usize,
    pub places: usize,
    pub sources: usize,
    pub media: usize,
    pub notes: usize,
    pub citations: usize,
    pub repositories: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Stats {
    pub counts: Counts,
    pub sex: Vec<Bucket>,
    pub age_at_death: Vec<Bucket>,
    /// (century start year, average lifespan, sample size)
    pub lifespan_by_century: Vec<(i32, f64, usize)>,
    pub avg_marriage_age: Option<f64>,
    pub children_per_family: Vec<Bucket>,
    pub top_given_names: Vec<(String, usize)>,
    pub top_surnames: Vec<(String, usize)>,
    pub top_occupations: Vec<(String, usize)>,
    pub top_places: Vec<(String, usize)>,
    pub birth_months: Vec<Bucket>,
    pub generation_depth: usize,
    pub avg_completeness: f64,
    /// Percentage of persons whose own record or events carry at least one citation.
    pub source_coverage_pct: f64,
    pub longest_lived: Vec<(String, f64)>,
    pub largest_families: Vec<(String, usize)>,
}

fn top(map: HashMap<String, usize>, n: usize) -> Vec<(String, usize)> {
    let mut v: Vec<(String, usize)> = map.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.truncate(n);
    v
}

/// Per-person completeness 0..100, over the checks that apply to that person.
pub fn completeness(f: &Facts, cited: &HashSet<String>, id: &str) -> f64 {
    let p = &f.persons[id];
    let mut checks: Vec<bool> = vec![
        !p.given.is_empty(),
        !p.surname.is_empty(),
        p.sex != Sex::Unknown,
        f.birth(id).is_some(),
        f.birth(id).and_then(|e| e.place.as_ref()).is_some(),
        !f.graph.people[id].parents.is_empty(),
        cited.contains(id),
    ];
    if f.has_death_event(id) {
        checks.push(f.death(id).is_some());
        checks.push(f.death(id).and_then(|e| e.place.as_ref()).is_some());
    }
    checks.iter().filter(|c| **c).count() as f64 * 100.0 / checks.len() as f64
}

pub fn compute(store: &Store) -> Result<Stats> {
    let f = Facts::load(store)?;
    let mut s = Stats {
        counts: Counts {
            persons: f.persons.len(),
            families: f.families.len(),
            events: f.events.len(),
            places: f.places.len(),
            sources: store.count("source")? as usize,
            media: store.count("media")? as usize,
            notes: store.count("note")? as usize,
            citations: store.count("citation")? as usize,
            repositories: store.count("repository")? as usize,
        },
        ..Default::default()
    };

    // Sex
    let mut sex: HashMap<&str, Vec<String>> = HashMap::new();
    for id in &f.order {
        let k = match f.persons[id].sex {
            Sex::Male => "M",
            Sex::Female => "F",
            Sex::Unknown => "U",
        };
        sex.entry(k).or_default().push(id.clone());
    }
    for k in ["M", "F", "U"] {
        let ids = sex.remove(k).unwrap_or_default();
        s.sex.push(Bucket {
            label: k.into(),
            count: ids.len(),
            ids,
        });
    }

    // Lifespans
    let mut ages: Vec<(String, f64, i32)> = vec![];
    for id in &f.order {
        if let (Some(b), Some(d)) = (f.birth(id), f.death(id)) {
            let age = years_between(b.start.unwrap(), d.start.unwrap());
            if (0.0..=125.0).contains(&age) {
                ages.push((id.clone(), age, jdn_to_gregorian(b.start.unwrap()).0));
            }
        }
    }
    let mut bins: Vec<Bucket> = (0..13)
        .map(|i| Bucket {
            label: format!("{}-{}", i * 10, i * 10 + 9),
            ..Default::default()
        })
        .collect();
    for (id, age, _) in &ages {
        let b = &mut bins[(*age / 10.0) as usize];
        b.count += 1;
        b.ids.push(id.clone());
    }
    s.age_at_death = bins;
    let mut by_century: HashMap<i32, (f64, usize)> = HashMap::new();
    for (_, age, y) in &ages {
        let c = y.div_euclid(100) * 100;
        let e = by_century.entry(c).or_default();
        e.0 += age;
        e.1 += 1;
    }
    let mut bc: Vec<(i32, f64, usize)> = by_century
        .into_iter()
        .map(|(c, (sum, n))| (c, sum / n as f64, n))
        .collect();
    bc.sort_by_key(|x| x.0);
    s.lifespan_by_century = bc;
    ages.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
    s.longest_lived = ages
        .iter()
        .take(10)
        .map(|(id, a, _)| (id.clone(), *a))
        .collect();

    // Marriage age
    let mut m_ages = vec![];
    for fam in &f.families {
        if let Some(m) = fam
            .events
            .iter()
            .map(|&i| &f.events[i])
            .find(|e| e.kind == "MARR" && e.start.is_some())
        {
            for p in &fam.partners {
                if let Some(b) = f.birth(p) {
                    let a = years_between(b.start.unwrap(), m.start.unwrap());
                    if (10.0..=90.0).contains(&a) {
                        m_ages.push(a);
                    }
                }
            }
        }
    }
    s.avg_marriage_age = if m_ages.is_empty() {
        None
    } else {
        Some(m_ages.iter().sum::<f64>() / m_ages.len() as f64)
    };

    // Children per family & largest families
    let mut cpf: Vec<Bucket> = (0..=10)
        .map(|i| Bucket {
            label: if i == 10 { "10+".into() } else { i.to_string() },
            ..Default::default()
        })
        .collect();
    for fam in &f.families {
        let b = &mut cpf[fam.children.len().min(10)];
        b.count += 1;
        b.ids.push(fam.id.clone());
    }
    s.children_per_family = cpf;
    let mut big: Vec<(String, usize)> = f
        .families
        .iter()
        .map(|x| (x.id.clone(), x.children.len()))
        .collect();
    big.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    big.truncate(10);
    s.largest_families = big;

    // Names, occupations, places
    let mut given: HashMap<String, usize> = HashMap::new();
    let mut sur: HashMap<String, usize> = HashMap::new();
    for id in &f.order {
        let p = &f.persons[id];
        if let Some(g) = p.given.split_whitespace().next() {
            *given.entry(g.to_string()).or_default() += 1;
        }
        if !p.surname.is_empty() {
            *sur.entry(p.surname.clone()).or_default() += 1;
        }
    }
    s.top_given_names = top(given, 10);
    s.top_surnames = top(sur, 10);
    let mut occ: HashMap<String, usize> = HashMap::new();
    let mut plc: HashMap<String, usize> = HashMap::new();
    for e in &f.events {
        if e.kind == "OCCU" {
            if let Some(v) = &e.value {
                *occ.entry(v.clone()).or_default() += 1;
            }
        }
        if let Some(p) = f.place_name(e) {
            *plc.entry(p.clone()).or_default() += 1;
        }
    }
    s.top_occupations = top(occ, 10);
    s.top_places = top(plc, 10);

    // Birth months
    let names = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut months: Vec<Bucket> = names
        .iter()
        .map(|n| Bucket {
            label: n.to_string(),
            ..Default::default()
        })
        .collect();
    for id in &f.order {
        if let Some(e) = f.birth(id) {
            if e.start == e.end {
                let m = jdn_to_gregorian(e.start.unwrap()).1 as usize - 1;
                months[m].count += 1;
                months[m].ids.push(id.clone());
            }
        }
    }
    s.birth_months = months;

    // Generation depth (longest ancestor chain), cycle-safe
    fn depth(
        f: &Facts,
        id: &str,
        memo: &mut HashMap<String, usize>,
        stack: &mut HashSet<String>,
    ) -> usize {
        if let Some(d) = memo.get(id) {
            return *d;
        }
        if !stack.insert(id.to_string()) {
            return 0;
        }
        let d = f.graph.people[id]
            .parents
            .iter()
            .map(|(p, _, _)| depth(f, p, memo, stack) + 1)
            .max()
            .unwrap_or(0);
        stack.remove(id);
        memo.insert(id.to_string(), d);
        d
    }
    let mut memo = HashMap::new();
    let mut stack = HashSet::new();
    s.generation_depth = f
        .order
        .iter()
        .map(|id| depth(&f, id, &mut memo, &mut stack))
        .max()
        .unwrap_or(0)
        + usize::from(!f.order.is_empty());

    // Sources & completeness
    let mut cited: HashSet<String> = HashSet::new();
    let ev_owner: HashMap<&str, &str> = f
        .events
        .iter()
        .filter(|e| e.owner_type == "person")
        .map(|e| (e.id.as_str(), e.owner_id.as_str()))
        .collect();
    for c in store.rows("citation")? {
        let (tt, tid) = (
            c["target_type"].as_str().unwrap_or(""),
            c["target_id"].as_str().unwrap_or(""),
        );
        match tt {
            "person" => {
                cited.insert(tid.to_string());
            }
            "event" => {
                if let Some(o) = ev_owner.get(tid) {
                    cited.insert(o.to_string());
                }
            }
            _ => {}
        }
    }
    if !f.order.is_empty() {
        s.source_coverage_pct = f.order.iter().filter(|id| cited.contains(*id)).count() as f64
            * 100.0
            / f.order.len() as f64;
        s.avg_completeness = f
            .order
            .iter()
            .map(|id| completeness(&f, &cited, id))
            .sum::<f64>()
            / f.order.len() as f64;
    }
    Ok(s)
}
