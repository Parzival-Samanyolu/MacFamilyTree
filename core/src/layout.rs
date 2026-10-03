//! Collision-free tree layout (Reingold–Tilford contours) for pedigree, descendant and hourglass views.
//!
//! The algorithm works in an abstract frame: `b` is the position across the tree (breadth), `d` the position along it
//! (depth). `Direction` maps that frame to screen coordinates.

use crate::relationship::{Graph, LinkKind};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Root at the bottom, ancestors above (classic top-down pedigree) / descendants below the root.
    TopDown,
    /// Root at the left, relatives to the right.
    LeftRight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    pub card_w: f64,
    pub card_h: f64,
    pub h_gap: f64,
    pub v_gap: f64,
    pub direction: Direction,
    pub ancestors: usize,
    pub descendants: usize,
    pub show_spouses: bool,
    /// Person ids whose descendant branches are collapsed.
    pub collapsed: HashSet<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            card_w: 160.0,
            card_h: 64.0,
            h_gap: 20.0,
            v_gap: 48.0,
            direction: Direction::TopDown,
            ancestors: 4,
            descendants: 3,
            show_spouses: true,
            collapsed: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LNode {
    pub person_id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Generation relative to the root: negative = ancestors, positive = descendants.
    pub generation: i32,
    /// Shown a second time (pedigree collapse); `dup_of` is the index of the primary node.
    pub dup_of: Option<usize>,
    /// True when more relatives exist beyond the generation limit or in a collapsed branch.
    pub has_more: bool,
    pub is_spouse: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LUnion {
    pub family_id: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EdgeKind {
    /// person node -> union (partner line)
    Partner,
    /// union -> child node (descendant direction) or child node -> parent node (ancestor direction)
    Child,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LEdge {
    pub kind: EdgeKind,
    /// Index into `nodes` (or `unions` if `from_union`).
    pub from: usize,
    pub from_union: bool,
    pub to: usize,
    pub to_union: bool,
    pub link: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Layout {
    pub nodes: Vec<LNode>,
    pub unions: Vec<LUnion>,
    pub edges: Vec<LEdge>,
    pub width: f64,
    pub height: f64,
}

// ---------- generic contour tree layout ----------

struct TNode {
    breadth: f64,
    children: Vec<usize>,
}

/// Returns each node's (relative-to-root) breadth centre. Depth is the caller's business.
fn tidy(nodes: &[TNode], root: usize, gap: f64) -> Vec<f64> {
    // Iterative post-order with contours stored per subtree.
    #[derive(Clone)]
    struct Sub {
        // contour[d] = (min, max) extent relative to this subtree's root centre at depth d
        contour: Vec<(f64, f64)>,
        child_offsets: Vec<f64>,
    }
    let n = nodes.len();
    let mut subs: Vec<Option<Sub>> = vec![None; n];
    let mut order = vec![];
    let mut stack = vec![root];
    while let Some(x) = stack.pop() {
        order.push(x);
        stack.extend(nodes[x].children.iter().copied());
    }
    for &x in order.iter().rev() {
        let half = nodes[x].breadth / 2.0;
        if nodes[x].children.is_empty() {
            subs[x] = Some(Sub {
                contour: vec![(-half, half)],
                child_offsets: vec![],
            });
            continue;
        }
        // Place children left to right, each as close as the contour allows.
        let mut placed: Vec<(usize, f64)> = vec![];
        let mut merged: Vec<(f64, f64)> = vec![];
        for &c in &nodes[x].children {
            let cs = subs[c].as_ref().unwrap();
            let mut shift = 0.0f64;
            if !placed.is_empty() {
                shift = f64::NEG_INFINITY;
                for (d, (lo, _)) in cs.contour.iter().enumerate() {
                    if let Some((_, mhi)) = merged.get(d) {
                        shift = shift.max(mhi + gap - lo);
                    }
                }
                if shift == f64::NEG_INFINITY {
                    // no shared depth (cannot happen: every child has depth 0), keep adjacent
                    shift = merged
                        .first()
                        .map(|m| m.1 + gap - cs.contour[0].0)
                        .unwrap_or(0.0);
                }
            }
            for (d, (lo, hi)) in cs.contour.iter().enumerate() {
                let (lo, hi) = (lo + shift, hi + shift);
                if d < merged.len() {
                    merged[d].0 = merged[d].0.min(lo);
                    merged[d].1 = merged[d].1.max(hi);
                } else {
                    merged.push((lo, hi));
                }
            }
            placed.push((c, shift));
        }
        // Centre the parent over its first and last child.
        let first = placed.first().unwrap();
        let last = placed.last().unwrap();
        let centre = (first.1 + last.1) / 2.0;
        let child_offsets: Vec<f64> = placed.iter().map(|(_, s)| s - centre).collect();
        let mut contour = vec![(-half, half)];
        for (lo, hi) in merged {
            contour.push((lo - centre, hi - centre));
        }
        subs[x] = Some(Sub {
            contour,
            child_offsets,
        });
    }
    let mut pos = vec![0.0; n];
    let mut stack = vec![root];
    while let Some(x) = stack.pop() {
        let s = subs[x].as_ref().unwrap();
        for (i, &c) in nodes[x].children.iter().enumerate() {
            pos[c] = pos[x] + s.child_offsets[i];
            stack.push(c);
        }
    }
    pos
}

struct Frame<'a> {
    o: &'a Options,
    dir_sign: f64,
}

impl Frame<'_> {
    fn breadth_size(&self) -> f64 {
        match self.o.direction {
            Direction::TopDown => self.o.card_w,
            Direction::LeftRight => self.o.card_h,
        }
    }
    fn depth_step(&self) -> f64 {
        match self.o.direction {
            Direction::TopDown => self.o.card_h + self.o.v_gap,
            Direction::LeftRight => self.o.card_w + self.o.v_gap,
        }
    }
    fn gap(&self) -> f64 {
        match self.o.direction {
            Direction::TopDown => self.o.h_gap,
            Direction::LeftRight => self.o.h_gap.min(self.o.v_gap),
        }
    }
    /// Abstract (breadth, depth-index) to screen top-left of a card.
    fn place(&self, b: f64, depth: f64) -> (f64, f64) {
        let d = depth * self.depth_step() * self.dir_sign;
        match self.o.direction {
            Direction::TopDown => (b - self.o.card_w / 2.0, d),
            Direction::LeftRight => (d, b - self.o.card_h / 2.0),
        }
    }
}

// ---------- ancestors ----------

struct AncNode {
    person: String,
    depth: i32,
    dup_of: Option<usize>,
    has_more: bool,
    parents: Vec<usize>,
}

fn build_ancestors(g: &Graph, root: &str, max: usize) -> Vec<AncNode> {
    let mut nodes: Vec<AncNode> = vec![AncNode {
        person: root.into(),
        depth: 0,
        dup_of: None,
        has_more: false,
        parents: vec![],
    }];
    let mut first_seen: HashMap<String, usize> = HashMap::new();
    first_seen.insert(root.into(), 0);
    let mut i = 0;
    while i < nodes.len() {
        if nodes[i].dup_of.is_some() {
            i += 1;
            continue;
        }
        let pid = nodes[i].person.clone();
        let depth = nodes[i].depth;
        // father first, then mother (so the father sits left / above)
        let mut ps: Vec<&(String, LinkKind, String)> = g.parents(&pid);
        ps.sort_by_key(|(p, _, _)| match g.sex(p) {
            crate::relationship::Sex::Male => 0,
            crate::relationship::Sex::Female => 1,
            _ => 2,
        });
        let mut seen_parent = HashSet::new();
        ps.retain(|(p, _, _)| seen_parent.insert(p.clone()));
        if depth as usize >= max {
            nodes[i].has_more = !ps.is_empty();
            i += 1;
            continue;
        }
        for (p, _, _) in ps.into_iter().take(2) {
            let idx = nodes.len();
            let dup = first_seen.get(p).copied();
            if dup.is_none() {
                first_seen.insert(p.clone(), idx);
            }
            nodes.push(AncNode {
                person: p.clone(),
                depth: depth + 1,
                dup_of: dup,
                has_more: false,
                parents: vec![],
            });
            nodes[i].parents.push(idx);
        }
        i += 1;
    }
    nodes
}

pub fn layout_ancestors(g: &Graph, root: &str, o: &Options) -> Layout {
    let sign = if o.direction == Direction::TopDown {
        -1.0
    } else {
        1.0
    };
    finish(ancestors_raw(g, root, o, sign))
}

/// Ancestor layout in un-normalised coordinates (root at the origin).
fn ancestors_raw(g: &Graph, root: &str, o: &Options, sign: f64) -> Layout {
    let anc = build_ancestors(g, root, o.ancestors);
    let fr = Frame { o, dir_sign: sign };
    let tn: Vec<TNode> = anc
        .iter()
        .map(|a| TNode {
            breadth: fr.breadth_size(),
            children: a.parents.clone(),
        })
        .collect();
    let b = tidy(&tn, 0, fr.gap());
    let mut out = Layout::default();
    for (i, a) in anc.iter().enumerate() {
        let (x, y) = fr.place(b[i], a.depth as f64);
        out.nodes.push(LNode {
            person_id: a.person.clone(),
            x,
            y,
            w: o.card_w,
            h: o.card_h,
            generation: -a.depth,
            dup_of: a.dup_of,
            has_more: a.has_more,
            is_spouse: false,
        });
    }
    for (i, a) in anc.iter().enumerate() {
        for &p in &a.parents {
            let link = g
                .parents(&a.person)
                .iter()
                .find(|(pp, _, _)| *pp == anc[p].person)
                .map(|(_, k, _)| format!("{:?}", k))
                .unwrap_or_default();
            out.edges.push(LEdge {
                kind: EdgeKind::Child,
                from: i,
                from_union: false,
                to: p,
                to_union: false,
                link,
            });
        }
    }
    out
}

// ---------- descendants ----------

struct Unit {
    person: usize,
    spouses: Vec<usize>,
    unions: Vec<(String, Vec<usize>)>,
    depth: i32,
    children: Vec<usize>,
    has_more: bool,
}

fn build_descendants(
    g: &Graph,
    births: &HashMap<String, i64>,
    root: &str,
    o: &Options,
    nodes: &mut Vec<LNode>,
    depth0: i32,
) -> Vec<Unit> {
    let mut units: Vec<Unit> = vec![];
    let mut visited: HashSet<String> = HashSet::new();
    let add_person = |nodes: &mut Vec<LNode>, id: &str, generation: i32, spouse: bool| -> usize {
        nodes.push(LNode {
            person_id: id.into(),
            x: 0.0,
            y: 0.0,
            w: o.card_w,
            h: o.card_h,
            generation,
            dup_of: None,
            has_more: false,
            is_spouse: spouse,
        });
        nodes.len() - 1
    };
    let root_idx = add_person(nodes, root, depth0, false);
    visited.insert(root.to_string());
    units.push(Unit {
        person: root_idx,
        spouses: vec![],
        unions: vec![],
        depth: 0,
        children: vec![],
        has_more: false,
    });
    let mut i = 0;
    while i < units.len() {
        let pid = nodes[units[i].person].person_id.clone();
        let depth = units[i].depth;
        // families this person is a partner in, ordered by marriage date when known
        let mut fams: Vec<(String, Vec<String>)> = vec![];
        let mut seen_f = HashSet::new();
        for (_, _, fid) in &g.people[&pid].children {
            if seen_f.insert(fid.clone()) {
                fams.push((fid.clone(), vec![]));
            }
        }
        for f in fams.iter_mut() {
            let mut kids: Vec<String> = g.people[&pid]
                .children
                .iter()
                .filter(|(_, _, fid)| *fid == f.0)
                .map(|(c, _, _)| c.clone())
                .collect();
            kids.sort_by_key(|c| births.get(c).copied().unwrap_or(i64::MAX));
            kids.dedup();
            f.1 = kids;
        }
        let collapsed = o.collapsed.contains(&pid);
        let all_kids: Vec<String> = fams.iter().flat_map(|f| f.1.clone()).collect();
        if o.show_spouses {
            let mut sp_seen = HashSet::new();
            for sp in &g.people[&pid].spouses {
                if sp_seen.insert(sp.clone()) && !visited.contains(sp) {
                    let idx = add_person(nodes, sp, depth0 + depth, true);
                    units[i].spouses.push(idx);
                }
            }
        }
        if depth as usize >= o.descendants || collapsed {
            units[i].has_more = !all_kids.is_empty();
            i += 1;
            continue;
        }
        for (fid, kids) in fams {
            let mut idxs = vec![];
            for k in kids {
                if !visited.insert(k.clone()) {
                    continue;
                }
                let n = add_person(nodes, &k, depth0 + depth + 1, false);
                let u = units.len();
                units.push(Unit {
                    person: n,
                    spouses: vec![],
                    unions: vec![],
                    depth: depth + 1,
                    children: vec![],
                    has_more: false,
                });
                units[i].children.push(u);
                idxs.push(u);
            }
            units[i].unions.push((fid, idxs));
        }
        i += 1;
    }
    units
}

pub fn layout_descendants(
    g: &Graph,
    births: &HashMap<String, i64>,
    root: &str,
    o: &Options,
) -> Layout {
    let mut out = Layout::default();
    let units = build_descendants(g, births, root, o, &mut out.nodes, 0);
    place_descendants(g, &units, o, &mut out, 0.0, 0.0, 1.0);
    finish(out)
}

/// Lay out descendant units; `sign` is +1 for descending away from the root.
fn place_descendants(
    g: &Graph,
    units: &[Unit],
    o: &Options,
    out: &mut Layout,
    origin_b: f64,
    origin_depth: f64,
    sign: f64,
) {
    let fr = Frame { o, dir_sign: sign };
    let unit_breadth = |u: &Unit| -> f64 {
        let n = 1 + u.spouses.len();
        n as f64 * fr.breadth_size() + (n - 1) as f64 * fr.gap()
    };
    let tn: Vec<TNode> = units
        .iter()
        .map(|u| TNode {
            breadth: unit_breadth(u),
            children: u.children.clone(),
        })
        .collect();
    let b = tidy(&tn, 0, fr.gap());
    for (i, u) in units.iter().enumerate() {
        let left = b[i] + origin_b - unit_breadth(u) / 2.0 + fr.breadth_size() / 2.0;
        let (x, y) = fr.place(left, origin_depth + u.depth as f64);
        out.nodes[u.person].x = x;
        out.nodes[u.person].y = y;
        out.nodes[u.person].has_more = u.has_more;
        for (k, &sp) in u.spouses.iter().enumerate() {
            let sb = left + (k + 1) as f64 * (fr.breadth_size() + fr.gap());
            let (x, y) = fr.place(sb, origin_depth + u.depth as f64);
            out.nodes[sp].x = x;
            out.nodes[sp].y = y;
        }
    }
    // unions & edges
    for u in units {
        let person_idx = u.person;
        for (fid, kids) in &u.unions {
            // partner of this family: the spouse node whose person is in the family, if shown
            let partner = u.spouses.iter().copied().find(|&s| {
                let sid = &out.nodes[s].person_id;
                g.people[&out.nodes[person_idx].person_id]
                    .spouses
                    .contains(sid)
                    && family_has(g, &out.nodes[person_idx].person_id, sid, fid)
            });
            let a = &out.nodes[person_idx];
            let (ax, ay) = (a.x, a.y);
            let (ux, uy) = match partner {
                Some(p) => ((ax + out.nodes[p].x) / 2.0, (ay + out.nodes[p].y) / 2.0),
                None => (ax, ay),
            };
            let (cx, cy) = (ux + o.card_w / 2.0, uy + o.card_h / 2.0);
            out.unions.push(LUnion {
                family_id: fid.clone(),
                x: cx,
                y: cy,
            });
            let ui = out.unions.len() - 1;
            out.edges.push(LEdge {
                kind: EdgeKind::Partner,
                from: person_idx,
                from_union: false,
                to: ui,
                to_union: true,
                link: String::new(),
            });
            if let Some(p) = partner {
                out.edges.push(LEdge {
                    kind: EdgeKind::Partner,
                    from: p,
                    from_union: false,
                    to: ui,
                    to_union: true,
                    link: String::new(),
                });
            }
            for &cu in kids {
                let child = units[cu].person;
                let link = g.people[&out.nodes[child].person_id]
                    .parents
                    .iter()
                    .find(|(_, _, f)| f == fid)
                    .map(|(_, k, _)| format!("{:?}", k))
                    .unwrap_or_default();
                out.edges.push(LEdge {
                    kind: EdgeKind::Child,
                    from: ui,
                    from_union: true,
                    to: child,
                    to_union: false,
                    link,
                });
            }
        }
    }
}

fn family_has(g: &Graph, a: &str, b: &str, fid: &str) -> bool {
    g.people[a].children.iter().any(|(c, _, f)| {
        f == fid
            && g.people[c]
                .parents
                .iter()
                .any(|(p, _, f2)| p == b && f2 == fid)
    }) || g.people[a].children.iter().all(|(_, _, f)| f != fid)
}

// ---------- hourglass ----------

pub fn layout_hourglass(
    g: &Graph,
    births: &HashMap<String, i64>,
    root: &str,
    o: &Options,
) -> Layout {
    let up = ancestors_raw(g, root, o, -1.0);
    let mut down = Layout::default();
    let units = build_descendants(g, births, root, o, &mut down.nodes, 0);
    place_descendants(g, &units, o, &mut down, 0.0, 0.0, 1.0);
    // Merge: root from the ancestor layout is kept; the descendant root node is dropped.
    let mut out = up;
    // align the roots on the breadth axis
    let (root_up, root_down) = (&out.nodes[0], &down.nodes[0]);
    let (dx, dy) = match o.direction {
        Direction::TopDown => (root_up.x - root_down.x, 0.0),
        Direction::LeftRight => (0.0, root_up.y - root_down.y),
    };
    let ubase = out.unions.len();
    let mut map: Vec<usize> = vec![0; down.nodes.len()];
    for (i, n) in down.nodes.iter().enumerate() {
        if i == 0 {
            map[0] = 0;
            continue;
        }
        let mut n = n.clone();
        n.x += dx;
        n.y += dy;
        out.nodes.push(n);
        map[i] = out.nodes.len() - 1;
    }
    for u in &down.unions {
        out.unions.push(LUnion {
            family_id: u.family_id.clone(),
            x: u.x + dx,
            y: u.y + dy,
        });
    }
    for e in &down.edges {
        let m = |i: usize, is_u: bool| if is_u { i + ubase } else { map[i] };
        out.edges.push(LEdge {
            kind: e.kind.clone(),
            from: m(e.from, e.from_union),
            from_union: e.from_union,
            to: m(e.to, e.to_union),
            to_union: e.to_union,
            link: e.link.clone(),
        });
    }
    // root spouses (from descendant layout) were appended as nodes; shift is applied above.
    out.nodes[0].has_more = false;
    finish(out)
}

fn finish(mut l: Layout) -> Layout {
    if l.nodes.is_empty() {
        return l;
    }
    let min_x = l.nodes.iter().map(|n| n.x).fold(f64::INFINITY, f64::min);
    let min_y = l.nodes.iter().map(|n| n.y).fold(f64::INFINITY, f64::min);
    let pad = 20.0;
    for n in &mut l.nodes {
        n.x = n.x - min_x + pad;
        n.y = n.y - min_y + pad;
    }
    for u in &mut l.unions {
        u.x = u.x - min_x + pad;
        u.y = u.y - min_y + pad;
    }
    l.width = l.nodes.iter().map(|n| n.x + n.w).fold(0.0, f64::max) + pad;
    l.height = l.nodes.iter().map(|n| n.y + n.h).fold(0.0, f64::max) + pad;
    l
}

/// True if any two cards overlap (used by tests and as a runtime assertion in debug builds).
pub fn has_overlap(l: &Layout) -> bool {
    let mut v: Vec<&LNode> = l.nodes.iter().collect();
    v.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    for i in 0..v.len() {
        for j in i + 1..v.len() {
            if v[j].x >= v[i].x + v[i].w - 1e-6 {
                break;
            }
            if v[j].y < v[i].y + v[i].h - 1e-6 && v[i].y < v[j].y + v[j].h - 1e-6 {
                return true;
            }
        }
    }
    false
}
