use kintree_core::gedcom;
use kintree_core::layout::*;
use kintree_core::relationship::{birth_keys, Graph};
use kintree_core::store::Store;
use std::collections::HashMap;

fn setup(n: usize, seed: u64) -> (Store, Graph, HashMap<String, i64>) {
    let mut s = Store::open_memory().unwrap();
    gedcom::import(
        &mut s,
        kintree_core::synth::generate_gedcom(n, seed).as_bytes(),
    )
    .unwrap();
    let g = Graph::load(&s).unwrap();
    let b = birth_keys(&s).unwrap();
    (s, g, b)
}

fn modes() -> Vec<Direction> {
    vec![Direction::TopDown, Direction::LeftRight]
}

#[test]
fn no_overlaps_in_any_mode_for_many_roots() {
    for seed in 1..=4 {
        let (_s, g, births) = setup(400, seed);
        for dir in modes() {
            let o = Options {
                direction: dir,
                ancestors: 6,
                descendants: 5,
                ..Default::default()
            };
            for root in g.order.iter().step_by(37) {
                for (name, l) in [
                    ("ancestors", layout_ancestors(&g, root, &o)),
                    ("descendants", layout_descendants(&g, &births, root, &o)),
                    ("hourglass", layout_hourglass(&g, &births, root, &o)),
                ] {
                    assert!(
                        !has_overlap(&l),
                        "overlap in {name} {dir:?} seed {seed} root {root}"
                    );
                    assert!(l.nodes.iter().all(|n| n.x >= 0.0 && n.y >= 0.0));
                    assert!(l.width > 0.0 && l.height > 0.0);
                }
            }
        }
    }
}

#[test]
fn ancestors_are_above_and_parents_centred_over_children() {
    let (_s, g, _) = setup(300, 5);
    let o = Options {
        ancestors: 5,
        ..Default::default()
    };
    // pick a person with a deep ancestry
    let root = g
        .order
        .iter()
        .max_by_key(|id| g.ancestor_depths(id, 20).len())
        .unwrap();
    let l = layout_ancestors(&g, root, &o);
    assert!(l.nodes.len() > 3);
    for e in &l.edges {
        let child = &l.nodes[e.from];
        let parent = &l.nodes[e.to];
        assert!(parent.y < child.y, "ancestor drawn above the descendant");
        assert_eq!(parent.generation, child.generation - 1);
    }
    // each person with two shown parents is horizontally centred between them
    for (i, n) in l.nodes.iter().enumerate() {
        let ps: Vec<&LNode> = l
            .edges
            .iter()
            .filter(|e| e.from == i)
            .map(|e| &l.nodes[e.to])
            .collect();
        if ps.len() == 2 {
            let mid = (ps[0].x + ps[1].x) / 2.0;
            assert!((mid - n.x).abs() < 1e-6, "child centred under its parents");
        }
    }
}

#[test]
fn left_right_puts_ancestors_to_the_right() {
    let (_s, g, _) = setup(200, 3);
    let o = Options {
        direction: Direction::LeftRight,
        ancestors: 4,
        ..Default::default()
    };
    let root = g
        .order
        .iter()
        .max_by_key(|id| g.ancestor_depths(id, 20).len())
        .unwrap();
    let l = layout_ancestors(&g, root, &o);
    for e in &l.edges {
        assert!(l.nodes[e.to].x > l.nodes[e.from].x);
    }
}

#[test]
fn pedigree_collapse_shows_stub_and_does_not_expand_it() {
    let ged = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n\
0 @I1@ INDI\n1 NAME Kid /X/\n1 FAMC @F3@\n\
0 @I2@ INDI\n1 NAME Dad /X/\n1 SEX M\n1 FAMS @F3@\n1 FAMC @F1@\n\
0 @I3@ INDI\n1 NAME Mom /X/\n1 SEX F\n1 FAMS @F3@\n1 FAMC @F1@\n\
0 @I4@ INDI\n1 NAME Gpa /X/\n1 SEX M\n1 FAMS @F1@\n\
0 @I5@ INDI\n1 NAME Gma /X/\n1 SEX F\n1 FAMS @F1@\n\
0 @F1@ FAM\n1 HUSB @I4@\n1 WIFE @I5@\n1 CHIL @I2@\n1 CHIL @I3@\n\
0 @F3@ FAM\n1 HUSB @I2@\n1 WIFE @I3@\n1 CHIL @I1@\n0 TRLR\n";
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, ged.as_bytes()).unwrap();
    let g = Graph::load(&s).unwrap();
    let kid = &g.order[0];
    let l = layout_ancestors(
        &g,
        kid,
        &Options {
            ancestors: 4,
            ..Default::default()
        },
    );
    let dups: Vec<&LNode> = l.nodes.iter().filter(|n| n.dup_of.is_some()).collect();
    assert_eq!(
        dups.len(),
        2,
        "grandparents appear twice, second time as stubs"
    );
    assert_eq!(l.nodes.len(), 7);
    assert!(!has_overlap(&l));
    for d in dups {
        let primary = &l.nodes[d.dup_of.unwrap()];
        assert_eq!(primary.person_id, d.person_id);
        assert!(primary.dup_of.is_none());
    }
}

#[test]
fn descendants_with_multiple_spouses_and_collapse() {
    let ged = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n\
0 @I1@ INDI\n1 NAME Root /X/\n1 SEX M\n1 FAMS @F1@\n1 FAMS @F2@\n\
0 @I2@ INDI\n1 NAME Wife1 /X/\n1 SEX F\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Wife2 /X/\n1 SEX F\n1 FAMS @F2@\n\
0 @I4@ INDI\n1 NAME C1 /X/\n1 BIRT\n2 DATE 1900\n1 FAMC @F1@\n1 FAMS @F9@\n\
0 @I5@ INDI\n1 NAME C2 /X/\n1 BIRT\n2 DATE 1910\n1 FAMC @F2@\n\
0 @I6@ INDI\n1 NAME G1 /X/\n1 FAMC @F9@\n\
0 @I7@ INDI\n1 NAME C1W /X/\n1 FAMS @F9@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I4@\n\
0 @F2@ FAM\n1 HUSB @I1@\n1 WIFE @I3@\n1 CHIL @I5@\n\
0 @F9@ FAM\n1 HUSB @I4@\n1 WIFE @I7@\n1 CHIL @I6@\n0 TRLR\n";
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, ged.as_bytes()).unwrap();
    let g = Graph::load(&s).unwrap();
    let births = birth_keys(&s).unwrap();
    let root = &g.order[0];
    let o = Options {
        descendants: 3,
        ..Default::default()
    };
    let l = layout_descendants(&g, &births, root, &o);
    assert_eq!(
        l.nodes.len(),
        7,
        "root, 2 wives, 2 children, grandchild, child's wife"
    );
    assert_eq!(l.nodes.iter().filter(|n| n.is_spouse).count(), 3);
    assert_eq!(l.unions.len(), 3);
    assert!(!has_overlap(&l));
    // root and its spouses share a row
    let ys: Vec<f64> = l
        .nodes
        .iter()
        .filter(|n| n.generation == 0)
        .map(|n| n.y)
        .collect();
    assert!(ys.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9));
    // collapsing C1 hides the grandchild but flags has_more
    let c1 = g
        .people
        .keys()
        .find(|k| {
            g.people[*k].children.len() == 1
                && g.people[*k].parents.len() == 2
                && g.people[&g.people[*k].children[0].0].parents.len() == 2
        })
        .unwrap()
        .clone();
    let mut o2 = o.clone();
    o2.collapsed.insert(c1.clone());
    let l2 = layout_descendants(&g, &births, root, &o2);
    assert_eq!(l2.nodes.len(), 6);
    assert!(l2.nodes.iter().any(|n| n.person_id == c1 && n.has_more));
    // generation limit
    let l3 = layout_descendants(
        &g,
        &births,
        root,
        &Options {
            descendants: 1,
            ..Default::default()
        },
    );
    assert!(l3.nodes.iter().all(|n| n.generation <= 1));
}

#[test]
fn contours_make_unbalanced_trees_compact() {
    // Root has a childless child A and a child B with three children. Side-by-side subtree boxes would need
    // 4w+3gap; with contours B's children tuck under A's empty space, so the whole chart is only 3w+2gap wide.
    let ged = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n\
0 @I1@ INDI\n1 NAME R /X/\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME A /X/\n1 BIRT\n2 DATE 1900\n1 FAMC @F1@\n\
0 @I3@ INDI\n1 NAME B /X/\n1 BIRT\n2 DATE 1910\n1 FAMC @F1@\n1 FAMS @F2@\n\
0 @I4@ INDI\n1 NAME B1 /X/\n1 FAMC @F2@\n0 @I5@ INDI\n1 NAME B2 /X/\n1 FAMC @F2@\n0 @I6@ INDI\n1 NAME B3 /X/\n1 FAMC @F2@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 CHIL @I2@\n1 CHIL @I3@\n\
0 @F2@ FAM\n1 HUSB @I3@\n1 CHIL @I4@\n1 CHIL @I5@\n1 CHIL @I6@\n0 TRLR\n";
    let mut s = Store::open_memory().unwrap();
    gedcom::import(&mut s, ged.as_bytes()).unwrap();
    let g = Graph::load(&s).unwrap();
    let births = birth_keys(&s).unwrap();
    let o = Options::default();
    let l = layout_descendants(&g, &births, &g.order[0], &o);
    assert_eq!(l.nodes.len(), 6);
    assert!(!has_overlap(&l));
    let span = l.width - 40.0;
    let compact = 3.0 * o.card_w + 2.0 * o.h_gap;
    assert!(
        (span - compact).abs() < 1e-6,
        "span {span} expected {compact}"
    );
}
