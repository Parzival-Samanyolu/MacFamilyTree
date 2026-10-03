//! Family graph, relationship calculation, inbreeding coefficient and ancestor/descendant numbering.

use crate::store::{Result, Store};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sex {
    Male,
    Female,
    Unknown,
}

impl Sex {
    pub fn from_gedcom(s: &str) -> Sex {
        match s {
            "M" => Sex::Male,
            "F" => Sex::Female,
            _ => Sex::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkKind {
    Biological,
    Adopted,
    Foster,
    Step,
}

impl LinkKind {
    pub fn from_pedi(s: &str) -> LinkKind {
        match s.to_ascii_lowercase().as_str() {
            "adopted" | "adop" => LinkKind::Adopted,
            "foster" => LinkKind::Foster,
            "step" => LinkKind::Step,
            _ => LinkKind::Biological,
        }
    }
    fn is_blood(self) -> bool {
        matches!(self, LinkKind::Biological)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Person {
    pub sex: Option<Sex>,
    /// (parent id, kind of link, family id)
    pub parents: Vec<(String, LinkKind, String)>,
    pub children: Vec<(String, LinkKind, String)>,
    pub spouses: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Graph {
    pub people: HashMap<String, Person>,
    /// Person ids in database order (stable iteration).
    pub order: Vec<String>,
}

impl Graph {
    pub fn load(store: &Store) -> Result<Graph> {
        let mut g = Graph::default();
        for p in store.rows("person")? {
            let id = p["id"].as_str().unwrap_or("").to_string();
            g.order.push(id.clone());
            g.people.insert(
                id,
                Person {
                    sex: Some(Sex::from_gedcom(p["sex"].as_str().unwrap_or(""))),
                    ..Default::default()
                },
            );
        }
        let mut kids: HashMap<String, Vec<(String, LinkKind)>> = HashMap::new();
        for c in store.rows("family_child")? {
            kids.entry(c["family_id"].as_str().unwrap_or("").to_string())
                .or_default()
                .push((
                    c["person_id"].as_str().unwrap_or("").to_string(),
                    LinkKind::from_pedi(c["rel_type"].as_str().unwrap_or("")),
                ));
        }
        for f in store.rows("family")? {
            let fid = f["id"].as_str().unwrap_or("").to_string();
            let partners: Vec<String> = ["partner1", "partner2"]
                .iter()
                .filter_map(|k| f[*k].as_str().map(String::from))
                .filter(|p| g.people.contains_key(p))
                .collect();
            if partners.len() == 2 {
                let (a, b) = (partners[0].clone(), partners[1].clone());
                g.people.get_mut(&a).unwrap().spouses.push(b.clone());
                g.people.get_mut(&b).unwrap().spouses.push(a);
            }
            for (child, kind) in kids.get(&fid).into_iter().flatten() {
                if !g.people.contains_key(child) {
                    continue;
                }
                for p in &partners {
                    if p == child {
                        continue; // a person cannot be their own parent
                    }
                    g.people
                        .get_mut(child)
                        .unwrap()
                        .parents
                        .push((p.clone(), *kind, fid.clone()));
                    g.people
                        .get_mut(p)
                        .unwrap()
                        .children
                        .push((child.clone(), *kind, fid.clone()));
                }
            }
        }
        Ok(g)
    }

    pub fn sex(&self, id: &str) -> Sex {
        self.people
            .get(id)
            .and_then(|p| p.sex)
            .unwrap_or(Sex::Unknown)
    }

    pub fn parents(&self, id: &str) -> Vec<&(String, LinkKind, String)> {
        self.people
            .get(id)
            .map(|p| p.parents.iter().collect())
            .unwrap_or_default()
    }

    pub fn father(&self, id: &str) -> Option<String> {
        self.parent_of_sex(id, Sex::Male)
    }
    pub fn mother(&self, id: &str) -> Option<String> {
        self.parent_of_sex(id, Sex::Female)
    }
    fn parent_of_sex(&self, id: &str, sex: Sex) -> Option<String> {
        let ps = &self.people.get(id)?.parents;
        // Prefer the biological parent of that sex; otherwise any.
        ps.iter()
            .filter(|(p, k, _)| self.sex(p) == sex && k.is_blood())
            .map(|x| x.0.clone())
            .next()
            .or_else(|| {
                ps.iter()
                    .find(|(p, _, _)| self.sex(p) == sex)
                    .map(|x| x.0.clone())
            })
    }

    /// Distance (generations) to every ancestor, with minimal path length; includes `id` at 0.
    pub fn ancestor_depths(&self, id: &str, max: usize) -> HashMap<String, usize> {
        let mut d = HashMap::new();
        let mut q = VecDeque::new();
        d.insert(id.to_string(), 0);
        q.push_back(id.to_string());
        while let Some(x) = q.pop_front() {
            let dx = d[&x];
            if dx >= max {
                continue;
            }
            for (p, _, _) in self.parents(&x) {
                if !d.contains_key(p) {
                    d.insert(p.clone(), dx + 1);
                    q.push_back(p.clone());
                }
            }
        }
        d
    }

    /// All ancestors reachable at *any* path length: id -> sorted set of distinct generation distances.
    pub fn ancestor_distances(&self, id: &str, max: usize) -> HashMap<String, Vec<usize>> {
        let mut out: HashMap<String, HashSet<usize>> = HashMap::new();
        let mut frontier: Vec<(String, usize)> = vec![(id.to_string(), 0)];
        while let Some((x, d)) = frontier.pop() {
            if !out.entry(x.clone()).or_default().insert(d) && d != 0 {
                continue;
            }
            if d >= max {
                continue;
            }
            for (p, _, _) in self.parents(&x) {
                frontier.push((p.clone(), d + 1));
            }
        }
        out.into_iter()
            .map(|(k, v)| {
                let mut v: Vec<usize> = v.into_iter().collect();
                v.sort();
                (k, v)
            })
            .collect()
    }

    pub fn is_ancestor_of(&self, anc: &str, of: &str) -> bool {
        anc != of && self.ancestor_depths(of, usize::MAX).contains_key(anc)
    }

    /// Persons that are their own ancestor (data loops). Returns one member per cycle found.
    pub fn find_cycles(&self) -> Vec<String> {
        #[derive(Clone, Copy, PartialEq)]
        enum C {
            White,
            Grey,
            Black,
        }
        let mut color: HashMap<&str, C> =
            self.people.keys().map(|k| (k.as_str(), C::White)).collect();
        let mut found = Vec::new();
        for start in &self.order {
            if color[start.as_str()] != C::White {
                continue;
            }
            // iterative DFS over parent edges
            let mut stack: Vec<(&str, usize)> = vec![(start.as_str(), 0)];
            color.insert(start.as_str(), C::Grey);
            while let Some(&(node, idx)) = stack.last() {
                let parents = &self.people[node].parents;
                if idx < parents.len() {
                    stack.last_mut().unwrap().1 += 1;
                    let p = parents[idx].0.as_str();
                    match color[p] {
                        C::White => {
                            color.insert(p, C::Grey);
                            stack.push((p, 0));
                        }
                        C::Grey => found.push(p.to_string()),
                        C::Black => {}
                    }
                } else {
                    color.insert(node, C::Black);
                    stack.pop();
                }
            }
        }
        found.sort();
        found.dedup();
        found
    }
}

// ---------------- relationship calculation ----------------

/// A blood relationship: `b` is `down` generations below the lowest common ancestor(s), `a` is `up` generations below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blood {
    pub up: usize,
    pub down: usize,
    /// Lowest common ancestors for this (up, down). Two = full relation (a couple), one = half relation.
    pub ancestors: Vec<String>,
    /// Sex of the parent of `a` through whom the relation passes (the "side": father's or mother's).
    pub side: Sex,
    /// Link kind of the weakest link on the path (adopted / foster / step make the relation non-biological).
    pub link: LinkKind,
    /// Sex of the (first) lowest common ancestor.
    pub anc_sex: Sex,
    /// Sex of `b`'s ancestor directly below the common ancestor (b's own sex when `down == 1`).
    pub b_below: Sex,
}

impl Blood {
    pub fn half(&self) -> bool {
        self.ancestors.len() == 1 && self.up >= 1 && self.down >= 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Same,
    Blood(Blood),
    /// `b` is the partner of `a`.
    Spouse,
    /// `b` is a blood relative of `a`'s spouse (`via`).
    SpouseRelative {
        via: String,
        blood: Blood,
    },
    /// `b` is the spouse of a blood relative `via` of `a`.
    RelativeSpouse {
        via: String,
        blood: Blood,
    },
    /// `b` is the spouse of a blood relative of `a`'s spouse (e.g. spouse's brother's wife).
    SpouseRelativeSpouse {
        spouse: String,
        via: String,
        blood: Blood,
    },
    /// `b` is a child of `a`'s partner but not `a`'s own child.
    StepChild,
    /// `b` is the partner of `a`'s parent but not `a`'s parent.
    StepParent,
    Unrelated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    pub kind: Kind,
    pub a_sex: Sex,
    pub b_sex: Sex,
}

/// One path of exactly `steps` parent links from `from` up to `to_anc`:
/// (weakest link kind, sex of the first parent on the path, sex of the person directly below `to_anc`).
/// Memoised, so it is linear in graph size even with pedigree collapse or corrupt (cyclic) data.
fn path_link(g: &Graph, from: &str, to_anc: &str, steps: usize) -> Option<(LinkKind, Sex, Sex)> {
    type Memo = HashMap<(String, usize), Option<(LinkKind, Sex)>>;
    fn worse(a: LinkKind, b: LinkKind) -> LinkKind {
        if !a.is_blood() {
            a
        } else {
            b
        }
    }
    fn sub(
        g: &Graph,
        cur: &str,
        target: &str,
        left: usize,
        memo: &mut Memo,
    ) -> Option<(LinkKind, Sex)> {
        let key = (cur.to_string(), left);
        if let Some(r) = memo.get(&key) {
            return *r;
        }
        memo.insert(key.clone(), None); // cycle guard
        let mut best: Option<(LinkKind, Sex)> = None;
        for (p, k, _) in g.parents(cur) {
            let r = if left == 1 {
                (p == target).then(|| (LinkKind::Biological, g.sex(cur)))
            } else {
                sub(g, p, target, left - 1, memo)
            };
            if let Some((w, below)) = r {
                let cand = (worse(*k, w), below);
                if best
                    .map(|b| !b.0.is_blood() && cand.0.is_blood())
                    .unwrap_or(true)
                {
                    best = Some(cand);
                }
            }
        }
        memo.insert(key, best);
        best
    }
    if steps == 0 {
        return (from == to_anc).then_some((LinkKind::Biological, Sex::Unknown, Sex::Unknown));
    }
    let mut memo = Memo::new();
    let mut best: Option<(LinkKind, Sex, Sex)> = None;
    for (p, k, _) in g.parents(from) {
        let r = if steps == 1 {
            (p == to_anc).then(|| (LinkKind::Biological, g.sex(from)))
        } else {
            sub(g, p, to_anc, steps - 1, &mut memo)
        };
        if let Some((w, below)) = r {
            let cand = (worse(*k, w), g.sex(p), below);
            if best
                .map(|b| !b.0.is_blood() && cand.0.is_blood())
                .unwrap_or(true)
            {
                best = Some(cand);
            }
        }
    }
    best
}

/// All distinct (up, down) blood relations between `a` and `b`, shortest first.
pub fn blood_relations(g: &Graph, a: &str, b: &str) -> Vec<Blood> {
    if !g.people.contains_key(a) || !g.people.contains_key(b) || a == b {
        return vec![];
    }
    let da = g.ancestor_distances(a, 40);
    let db = g.ancestor_distances(b, 40);
    let common: Vec<&String> = da.keys().filter(|k| db.contains_key(*k)).collect();
    // A route (c, up, down) is redundant when both of its paths run through another common ancestor c2,
    // i.e. it is merely c2's relationship extended further up the same chain.
    let mut up_cache: HashMap<&String, HashMap<String, Vec<usize>>> = HashMap::new();
    for c2 in &common {
        up_cache.insert(*c2, g.ancestor_distances(c2, 40));
    }
    let redundant = |c: &String, u: usize, d: usize| -> bool {
        common.iter().any(|c2| {
            if *c2 == c {
                return false;
            }
            let Some(ks) = up_cache[*c2].get(c) else {
                return false;
            };
            ks.iter().any(|&k| {
                k >= 1
                    && u >= k
                    && d >= k
                    && da[*c2].contains(&(u - k))
                    && db[*c2].contains(&(d - k))
            })
        })
    };
    let mut by_shape: HashMap<(usize, usize), Vec<String>> = HashMap::new();
    for c in &common {
        for &u in &da[*c] {
            for &d in &db[*c] {
                if !redundant(c, u, d) {
                    by_shape.entry((u, d)).or_default().push((*c).clone());
                }
            }
        }
    }
    let mut out: Vec<Blood> = by_shape
        .into_iter()
        .map(|((up, down), mut ancestors)| {
            ancestors.sort();
            ancestors.dedup();
            let (l1, side, _) = ancestors
                .first()
                .and_then(|c| path_link(g, a, c, up))
                .unwrap_or((LinkKind::Biological, Sex::Unknown, Sex::Unknown));
            let (l2, _, b_below) = ancestors
                .first()
                .and_then(|c| path_link(g, b, c, down))
                .unwrap_or((LinkKind::Biological, Sex::Unknown, g.sex(b)));
            let link = if !l1.is_blood() { l1 } else { l2 };
            let anc_sex = ancestors.first().map(|c| g.sex(c)).unwrap_or(Sex::Unknown);
            Blood {
                up,
                down,
                ancestors,
                side,
                link,
                anc_sex,
                b_below,
            }
        })
        .collect();
    out.sort_by_key(|r| (r.up + r.down, r.up));
    out
}

/// The closest relationship between two people, plus any alternative blood paths (pedigree collapse / multiple routes).
pub fn relationship(g: &Graph, a: &str, b: &str) -> Vec<Relation> {
    let sa = g.sex(a);
    let sb = g.sex(b);
    let mk = |kind| Relation {
        kind,
        a_sex: sa,
        b_sex: sb,
    };
    if a == b {
        return vec![mk(Kind::Same)];
    }
    let blood = blood_relations(g, a, b);
    if !blood.is_empty() {
        return blood.into_iter().map(|bl| mk(Kind::Blood(bl))).collect();
    }
    let pa = &g.people[a];
    let pb = &g.people[b];
    if pa.spouses.iter().any(|s| s == b) {
        return vec![mk(Kind::Spouse)];
    }
    // step relations
    for s in &pa.spouses {
        if g.people[s].children.iter().any(|(c, _, _)| c == b) {
            return vec![mk(Kind::StepChild)];
        }
    }
    for (p, _, _) in &pa.parents {
        if g.people[p].spouses.iter().any(|s| s == b) {
            return vec![mk(Kind::StepParent)];
        }
    }
    let mut out = Vec::new();
    // b is a blood relative of a's spouse
    for s in &pa.spouses {
        for bl in blood_relations(g, s, b) {
            out.push((
                bl.up + bl.down + 1,
                Kind::SpouseRelative {
                    via: s.clone(),
                    blood: bl,
                },
            ));
        }
        if blood_relations(g, s, b).is_empty() && s == b {
            continue;
        }
    }
    // b is the spouse of a blood relative of a
    for s in &pb.spouses {
        for bl in blood_relations(g, a, s) {
            out.push((
                bl.up + bl.down + 1,
                Kind::RelativeSpouse {
                    via: s.clone(),
                    blood: bl,
                },
            ));
        }
        // ... or a's own descendant/ancestor (blood_relations excludes identity but includes them)
    }
    // b is the spouse of a blood relative of a's spouse
    for sa_ in &pa.spouses {
        for sb_ in &pb.spouses {
            for bl in blood_relations(g, sa_, sb_) {
                out.push((
                    bl.up + bl.down + 2,
                    Kind::SpouseRelativeSpouse {
                        spouse: sa_.clone(),
                        via: sb_.clone(),
                        blood: bl,
                    },
                ));
            }
        }
    }
    out.sort_by_key(|(d, _)| *d);
    if out.is_empty() {
        return vec![mk(Kind::Unrelated)];
    }
    // Only the closest class of in-law connection is meaningful; longer compounds are noise.
    let best = out[0].0;
    out.into_iter()
        .filter(|(d, _)| *d == best)
        .map(|(_, k)| mk(k))
        .collect()
}

// ---------------- inbreeding ----------------

pub struct Kinship<'a> {
    g: &'a Graph,
    depth: HashMap<String, usize>,
    memo: HashMap<(String, String), f64>,
    in_progress: HashSet<(String, String)>,
}

impl<'a> Kinship<'a> {
    pub fn new(g: &'a Graph) -> Self {
        // generation depth = longest chain to a founder (cycle-safe)
        let mut depth: HashMap<String, usize> = HashMap::new();
        fn d(
            g: &Graph,
            id: &str,
            depth: &mut HashMap<String, usize>,
            visiting: &mut HashSet<String>,
        ) -> usize {
            if let Some(x) = depth.get(id) {
                return *x;
            }
            if !visiting.insert(id.to_string()) {
                return 0;
            }
            let v = g
                .parents(id)
                .iter()
                .map(|(p, _, _)| d(g, p, depth, visiting) + 1)
                .max()
                .unwrap_or(0);
            visiting.remove(id);
            depth.insert(id.to_string(), v);
            v
        }
        let mut visiting = HashSet::new();
        for id in &g.order {
            d(g, id, &mut depth, &mut visiting);
        }
        Kinship {
            g,
            depth,
            memo: HashMap::new(),
            in_progress: HashSet::new(),
        }
    }

    fn parents_pair(&self, id: &str) -> (Option<String>, Option<String>) {
        let ps = self.g.parents(id);
        // use the first two distinct parents (father/mother by data order)
        let mut v: Vec<String> = Vec::new();
        for (p, k, _) in ps {
            if k.is_blood() && !v.contains(p) {
                v.push(p.clone());
            }
        }
        (v.first().cloned(), v.get(1).cloned())
    }

    /// Kinship coefficient φ(a,b): probability that a random allele from each is identical by descent.
    pub fn phi(&mut self, a: &str, b: &str) -> f64 {
        let key = if a <= b {
            (a.to_string(), b.to_string())
        } else {
            (b.to_string(), a.to_string())
        };
        if let Some(v) = self.memo.get(&key) {
            return *v;
        }
        if !self.in_progress.insert(key.clone()) {
            return 0.0; // corrupt (cyclic) data: break the loop
        }
        let da = self.depth.get(a).copied().unwrap_or(0);
        let db = self.depth.get(b).copied().unwrap_or(0);
        let v = if a == b {
            let (f, m) = self.parents_pair(a);
            0.5 * (1.0 + self.pair_phi(f, m))
        } else {
            // expand the deeper (younger) individual; it cannot be an ancestor of the shallower one
            let (young, old) = if da >= db { (a, b) } else { (b, a) };
            let (f, m) = self.parents_pair(young);
            if f.is_none() && m.is_none() {
                0.0
            } else {
                let mut s = 0.0;
                if let Some(f) = f {
                    s += self.phi(&f, old);
                }
                if let Some(m) = m {
                    s += self.phi(&m, old);
                }
                0.5 * s
            }
        };
        self.in_progress.remove(&key);
        self.memo.insert(key, v);
        v
    }

    fn pair_phi(&mut self, a: Option<String>, b: Option<String>) -> f64 {
        match (a, b) {
            (Some(a), Some(b)) if a != b => self.phi(&a, &b),
            _ => 0.0,
        }
    }

    /// Inbreeding coefficient F of a person = kinship of their parents.
    pub fn inbreeding(&mut self, id: &str) -> f64 {
        let (f, m) = self.parents_pair(id);
        self.pair_phi(f, m)
    }

    /// Coefficient of relationship r = 2·φ(a,b) / sqrt((1+Fa)(1+Fb)).
    pub fn relatedness(&mut self, a: &str, b: &str) -> f64 {
        let fa = self.inbreeding(a);
        let fb = self.inbreeding(b);
        2.0 * self.phi(a, b) / ((1.0 + fa) * (1.0 + fb)).sqrt()
    }
}

// ---------------- numbering systems ----------------

/// Ahnentafel (Sosa-Stradonitz): root = 1, father = 2n, mother = 2n+1. Persons reached by several routes appear several times.
pub fn ahnentafel(g: &Graph, root: &str, max_gen: usize) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    let mut q = VecDeque::new();
    q.push_back((1u64, root.to_string(), 0usize));
    while let Some((n, id, gen)) = q.pop_front() {
        out.push((n, id.clone()));
        if gen >= max_gen {
            continue;
        }
        if let Some(f) = g.father(&id) {
            q.push_back((n * 2, f, gen + 1));
        }
        if let Some(m) = g.mother(&id) {
            q.push_back((n * 2 + 1, m, gen + 1));
        }
    }
    out.sort_by_key(|x| x.0);
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescendantNumbering {
    /// 1, 1.2, 1.2.3
    DAboville,
    /// 1, 11, 112 (digits; 10th+ child written X, A…)
    Henry,
}

/// Children ordered by birth sort key where known, otherwise by link order.
fn ordered_children(g: &Graph, store_dates: &HashMap<String, i64>, id: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut kids: Vec<(usize, String)> = g.people[id]
        .children
        .iter()
        .filter(|(c, _, _)| seen.insert(c.clone()))
        .enumerate()
        .map(|(i, (c, _, _))| (i, c.clone()))
        .collect();
    kids.sort_by_key(|(i, c)| (store_dates.get(c).copied().unwrap_or(i64::MAX), *i));
    kids.into_iter().map(|(_, c)| c).collect()
}

pub fn birth_keys(store: &Store) -> Result<HashMap<String, i64>> {
    let mut m = HashMap::new();
    for e in store.rows("event")? {
        if e["owner_type"] == "person" && e["kind"] == "BIRT" {
            if let (Some(o), Some(k)) = (e["owner_id"].as_str(), e["date_sort"].as_i64()) {
                m.entry(o.to_string()).or_insert(k);
            }
        }
    }
    Ok(m)
}

pub fn number_descendants(
    g: &Graph,
    births: &HashMap<String, i64>,
    root: &str,
    max_gen: usize,
    system: DescendantNumbering,
) -> Vec<(String, String)> {
    fn henry_digit(i: usize) -> String {
        match i {
            1..=9 => i.to_string(),
            10 => "X".into(),
            n => ((b'A' + (n - 11) as u8 % 26) as char).to_string(),
        }
    }
    let mut out = vec![("1".to_string(), root.to_string())];
    let mut stack = vec![("1".to_string(), root.to_string(), 0usize)];
    let mut visited = HashSet::new();
    visited.insert(root.to_string());
    while let Some((num, id, gen)) = stack.pop() {
        if gen >= max_gen {
            continue;
        }
        for (i, c) in ordered_children(g, births, &id).into_iter().enumerate() {
            if !visited.insert(c.clone()) {
                continue; // pedigree collapse / loops: number each descendant once
            }
            let n = match system {
                DescendantNumbering::DAboville => format!("{}.{}", num, i + 1),
                DescendantNumbering::Henry => format!("{}{}", num, henry_digit(i + 1)),
            };
            out.push((n.clone(), c.clone()));
            stack.push((n, c, gen + 1));
        }
    }
    out.sort_by(|a, b| a.0.len().cmp(&b.0.len()).then(a.0.cmp(&b.0)));
    out
}
