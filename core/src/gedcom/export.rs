use super::charset::{self, Charset};
use super::tree::{self, Node};
use crate::date::GenDate;
use crate::model::living_ids;
use crate::name::PersonName;
use crate::store::{Result, Row, Store};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V551,
    V70,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LivingPolicy {
    /// Export everyone.
    Include,
    /// Keep living persons as "Living" with sex only; drop their events, notes, sources, media.
    Mask,
    /// Remove living persons entirely.
    Exclude,
}

/// Target software. Currently only changes the header's SOURce identification, which some importers key on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Standard,
    Ancestry,
    FamilyTreeMaker,
    RootsMagic,
    Legacy,
    Gramps,
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub version: Version,
    pub charset: Charset,
    pub living: LivingPolicy,
    pub living_years: i32,
    pub current_year: i32,
    pub dialect: Dialect,
    pub include_media: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            version: Version::V551,
            charset: Charset::Utf8,
            living: LivingPolicy::Include,
            living_years: 110,
            current_year: 2026,
            dialect: Dialect::Standard,
            include_media: true,
        }
    }
}

fn sv(r: &Row, k: &str) -> Option<String> {
    r.get(k)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from)
}
fn sid(r: &Row) -> String {
    r["id"].as_str().unwrap_or("").to_string()
}
fn si(r: &Row, k: &str) -> i64 {
    r.get(k).and_then(|v| v.as_i64()).unwrap_or(0)
}
fn group(rows: Vec<Row>, key: impl Fn(&Row) -> String) -> HashMap<String, Vec<Row>> {
    let mut m: HashMap<String, Vec<Row>> = HashMap::new();
    for r in rows {
        m.entry(key(&r)).or_default().push(r);
    }
    m
}
fn tkey(t: &str, i: &str) -> String {
    format!("{}\u{1}{}", t, i)
}

struct Ctx<'o> {
    o: &'o ExportOptions,
    xref: HashMap<String, String>,
    raws: HashMap<String, Vec<Node>>,
    notes: HashMap<String, Row>,
    sources: HashMap<String, Row>,
    media: HashMap<String, Row>,
    places: HashMap<String, Row>,
    note_links: HashMap<String, Vec<Row>>,
    cites: HashMap<String, Vec<Row>>,
    media_links: HashMap<String, Vec<Row>>,
}

impl Ctx<'_> {
    fn ptr(&self, id: &str) -> String {
        self.xref.get(id).cloned().unwrap_or_default()
    }

    fn raw_into(&self, node: &mut Node, owner_type: &str, owner_id: &str) {
        if let Some(v) = self.raws.get(&tkey(owner_type, owner_id)) {
            node.children.extend(v.iter().cloned());
        }
    }

    /// NOTE / SOUR / OBJE children for a target, followed by its preserved raw structures.
    fn common(&self, node: &mut Node, ttype: &str, tid: &str) {
        let key = tkey(ttype, tid);
        for l in self.note_links.get(&key).into_iter().flatten() {
            let nid = sv(l, "note_id").unwrap_or_default();
            let Some(n) = self.notes.get(&nid) else {
                continue;
            };
            if si(n, "inline") == 1 {
                let mut c = Node::new("NOTE", n["body"].as_str().unwrap_or(""));
                self.common(&mut c, "note", &nid);
                node.children.push(c);
            } else {
                node.children
                    .push(Node::new(self.shared_note_tag(), &self.ptr(&nid)));
            }
        }
        for c in self.cites.get(&key).into_iter().flatten() {
            let sid_ = sv(c, "source_id").unwrap_or_default();
            let Some(src) = self.sources.get(&sid_) else {
                continue;
            };
            let value = if si(src, "inline") == 1 {
                src["title"].as_str().unwrap_or("").to_string()
            } else {
                self.ptr(&sid_)
            };
            let mut cn = Node::new("SOUR", &value);
            if let Some(p) = sv(c, "page") {
                cn.children.push(Node::new("PAGE", &p));
            }
            if let Some(q) = sv(c, "quality") {
                cn.children.push(Node::new("QUAY", &q));
            }
            let cid = sid(c);
            self.common(&mut cn, "citation", &cid);
            node.children.push(cn);
        }
        if self.o.include_media {
            for l in self.media_links.get(&key).into_iter().flatten() {
                let mid = sv(l, "media_id").unwrap_or_default();
                let Some(m) = self.media.get(&mid) else {
                    continue;
                };
                if si(m, "inline") == 1 {
                    node.children.push(self.media_node(m, None));
                } else {
                    node.children.push(Node::new("OBJE", &self.ptr(&mid)));
                }
            }
        }
        self.raw_into(node, ttype, tid);
    }

    fn shared_note_tag(&self) -> &'static str {
        if self.o.version == Version::V70 {
            "SNOTE"
        } else {
            "NOTE"
        }
    }

    fn media_node(&self, m: &Row, xref: Option<String>) -> Node {
        let id = sid(m);
        let mut n = Node::new("OBJE", "");
        n.xref = xref;
        let mut f = Node::new("FILE", m["path"].as_str().unwrap_or(""));
        if let Some(k) = sv(m, "kind") {
            f.children.push(Node::new("FORM", &k));
        }
        if let Some(t) = sv(m, "caption") {
            f.children.push(Node::new("TITL", &t));
        }
        self.raw_into(&mut f, "media_file", &id);
        n.children.push(f);
        self.common(&mut n, "media", &id);
        n
    }

    fn place_chain(&self, mut id: Option<String>) -> String {
        let mut parts = Vec::new();
        let mut guard = 0;
        while let Some(pid) = id {
            let Some(p) = self.places.get(&pid) else {
                break;
            };
            parts.push(p["name"].as_str().unwrap_or("").to_string());
            id = sv(p, "parent_id");
            guard += 1;
            if guard > 64 {
                break;
            }
        }
        parts.join(", ")
    }

    fn date_text(&self, json: &str) -> Option<String> {
        let d: GenDate = serde_json::from_str(json).ok()?;
        let t = if self.o.version == Version::V70 {
            d.to_gedcom7()
        } else {
            d.to_gedcom()
        };
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    }

    fn event_node(&self, e: &Row) -> Node {
        let id = sid(e);
        let mut n = Node::new(
            e["kind"].as_str().unwrap_or("EVEN"),
            e["value"].as_str().unwrap_or(""),
        );
        if let Some(t) = sv(e, "custom_kind") {
            n.children.push(Node::new("TYPE", &t));
        }
        if let Some(d) = sv(e, "date_json").and_then(|j| self.date_text(&j)) {
            n.children.push(Node::new("DATE", &d));
        }
        let pid = sv(e, "place_id");
        let has_raw_plac = self.raws.contains_key(&tkey("event_plac", &id));
        if pid.is_some() || has_raw_plac {
            let mut p = Node::new("PLAC", &self.place_chain(pid.clone()));
            let want_map = e
                .get("plac_map")
                .and_then(|v| v.as_i64())
                .map(|v| v == 1)
                .unwrap_or(true);
            if let Some(pl) = pid
                .as_ref()
                .and_then(|i| self.places.get(i))
                .filter(|_| want_map)
            {
                if let (Some(lat), Some(lon)) = (pl["lat"].as_f64(), pl["lon"].as_f64()) {
                    let mut m = Node::new("MAP", "");
                    m.children.push(Node::new(
                        "LATI",
                        &format!("{}{}", if lat < 0.0 { "S" } else { "N" }, lat.abs()),
                    ));
                    m.children.push(Node::new(
                        "LONG",
                        &format!("{}{}", if lon < 0.0 { "W" } else { "E" }, lon.abs()),
                    ));
                    p.children.push(m);
                }
            }
            self.raw_into(&mut p, "event_plac", &id);
            n.children.push(p);
        }
        if let Some(c) = sv(e, "cause") {
            n.children.push(Node::new("CAUS", &c));
        }
        if let Some(a) = sv(e, "agency") {
            n.children.push(Node::new("AGNC", &a));
        }
        self.common(&mut n, "event", &id);
        n
    }

    fn name_node(&self, r: &Row) -> Node {
        let pn = PersonName {
            prefix: sv(r, "prefix").unwrap_or_default(),
            given: sv(r, "given").unwrap_or_default(),
            nickname: sv(r, "nickname").unwrap_or_default(),
            surname_prefix: sv(r, "surname_prefix").unwrap_or_default(),
            surname: sv(r, "surname").unwrap_or_default(),
            suffix: sv(r, "suffix").unwrap_or_default(),
            ..Default::default()
        };
        let mut n = Node::new("NAME", &pn.to_gedcom());
        let allowed: Vec<String> = match r.get("src_tags").and_then(|v| v.as_str()) {
            Some(t) => t
                .split(',')
                .filter(|x| !x.is_empty())
                .map(String::from)
                .collect(),
            None => ["NPFX", "GIVN", "NICK", "SPFX", "SURN", "NSFX"]
                .iter()
                .map(|x| x.to_string())
                .collect(),
        };
        for (tag, val) in [
            ("NPFX", &pn.prefix),
            ("GIVN", &pn.given),
            ("NICK", &pn.nickname),
            ("SPFX", &pn.surname_prefix),
            ("SURN", &pn.surname),
            ("NSFX", &pn.suffix),
        ] {
            if allowed.iter().any(|a| a == tag) && !val.is_empty() {
                n.children.push(Node::new(tag, val));
            }
        }
        if let Some(k) = sv(r, "kind") {
            let t = match k.as_str() {
                "Birth" => "birth".to_string(),
                "Married" => "married".to_string(),
                "Aka" => "aka".to_string(),
                "Religious" => "religious".to_string(),
                "Immigrant" => "immigrant".to_string(),
                other => other.to_string(),
            };
            n.children.push(Node::new("TYPE", &t));
        }
        let id = sid(r);
        self.common(&mut n, "person_name", &id);
        n
    }
}

fn assign_xrefs(store: &Store) -> Result<HashMap<String, String>> {
    let mut used: HashSet<String> = HashSet::new();
    let mut map = HashMap::new();
    let tables = [
        ("person", "I"),
        ("family", "F"),
        ("source", "S"),
        ("repository", "R"),
        ("note", "N"),
        ("media", "O"),
    ];
    let mut rows_by_table = Vec::new();
    for (t, p) in tables {
        let rows = store.rows(t)?;
        for r in &rows {
            if let Some(x) = sv(r, "xref") {
                if used.insert(x.clone()) {
                    map.insert(sid(r), format!("@{}@", x));
                }
            }
        }
        rows_by_table.push((p, rows));
    }
    // Reserve xrefs referenced by raw level-0 records and the header (e.g. @U1@ submitter).
    for r in store.rows("raw_tag")? {
        if r["owner_type"] == "record" || r["owner_type"] == "head" {
            for tok in r["line"].as_str().unwrap_or("").split_whitespace() {
                if tree::is_pointer(tok) {
                    used.insert(tok.trim_matches('@').to_string());
                }
            }
        }
    }
    for (prefix, rows) in rows_by_table {
        let mut n = 0;
        for r in rows {
            if map.contains_key(&sid(&r)) {
                continue;
            }
            loop {
                n += 1;
                let cand = format!("{}{}", prefix, n);
                if used.insert(cand.clone()) {
                    map.insert(sid(&r), format!("@{}@", cand));
                    break;
                }
            }
        }
    }
    Ok(map)
}

pub fn export(store: &Store, o: &ExportOptions) -> Result<Vec<u8>> {
    let text = export_text(store, o)?;
    Ok(charset::encode(&text, o.charset))
}

pub fn export_text(store: &Store, o: &ExportOptions) -> Result<String> {
    let mut raws: HashMap<String, Vec<Node>> = HashMap::new();
    let mut head_raw: Option<Node> = None;
    let mut record_raws: Vec<Node> = Vec::new();
    for r in store.rows("raw_tag")? {
        let ot = r["owner_type"].as_str().unwrap_or("");
        let mut issues = Vec::new();
        let nodes = tree::parse(r["line"].as_str().unwrap_or(""), &mut issues);
        match ot {
            "head" => head_raw = nodes.into_iter().next(),
            "record" => record_raws.extend(nodes),
            _ => raws
                .entry(tkey(ot, r["owner_id"].as_str().unwrap_or("")))
                .or_default()
                .extend(nodes),
        }
    }
    let by_id = |t: &str| -> Result<HashMap<String, Row>> {
        Ok(store.rows(t)?.into_iter().map(|r| (sid(&r), r)).collect())
    };
    let cx = Ctx {
        o,
        xref: assign_xrefs(store)?,
        raws,
        notes: by_id("note")?,
        sources: by_id("source")?,
        media: by_id("media")?,
        places: by_id("place")?,
        note_links: group(store.rows("note_link")?, |r| {
            tkey(
                r["target_type"].as_str().unwrap_or(""),
                r["target_id"].as_str().unwrap_or(""),
            )
        }),
        cites: group(store.rows("citation")?, |r| {
            tkey(
                r["target_type"].as_str().unwrap_or(""),
                r["target_id"].as_str().unwrap_or(""),
            )
        }),
        media_links: group(store.rows("media_link")?, |r| {
            tkey(
                r["target_type"].as_str().unwrap_or(""),
                r["target_id"].as_str().unwrap_or(""),
            )
        }),
    };

    let living: HashSet<String> = if o.living == LivingPolicy::Include {
        HashSet::new()
    } else {
        living_ids(store, o.living_years, o.current_year)?
    };
    let persons = store.rows("person")?;
    let names = group(store.rows("person_name")?, |r| {
        r["person_id"].as_str().unwrap_or("").to_string()
    });
    let events = group(store.rows("event")?, |r| {
        tkey(
            r["owner_type"].as_str().unwrap_or(""),
            r["owner_id"].as_str().unwrap_or(""),
        )
    });
    let families = store.rows("family")?;
    let children = group(store.rows("family_child")?, |r| {
        r["family_id"].as_str().unwrap_or("").to_string()
    });
    let assos = group(store.rows("association")?, |r| {
        r["person_id"].as_str().unwrap_or("").to_string()
    });

    let hidden = |pid: &str| o.living == LivingPolicy::Exclude && living.contains(pid);
    let masked = |pid: &str| o.living == LivingPolicy::Mask && living.contains(pid);
    let alive_in = |pid: &str| !o.living.eq(&LivingPolicy::Include) && living.contains(pid);

    // Sort events by explicit order.
    let sorted_events = |key: &str| -> Vec<Row> {
        let mut v = events.get(key).cloned().unwrap_or_default();
        v.sort_by_key(|e| si(e, "sort_order"));
        v
    };

    // Per-person family links (FAMC / FAMS), and family nodes.
    let mut famc: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut fams: HashMap<String, Vec<String>> = HashMap::new();
    let mut fam_nodes: Vec<Node> = Vec::new();
    for f in &families {
        let fid = sid(f);
        let p1 = sv(f, "partner1").filter(|p| !hidden(p));
        let p2 = sv(f, "partner2").filter(|p| !hidden(p));
        let mut kids: Vec<Row> = children.get(&fid).cloned().unwrap_or_default();
        kids.sort_by_key(|k| si(k, "sort_order"));
        kids.retain(|k| !hidden(k["person_id"].as_str().unwrap_or("")));
        if p1.is_none() && p2.is_none() && kids.is_empty() && o.living != LivingPolicy::Include {
            continue;
        }
        let mut n = Node::new("FAM", "");
        n.xref = Some(cx.ptr(&fid));
        for (tag, p) in [("HUSB", &p1), ("WIFE", &p2)] {
            if let Some(p) = p {
                n.children.push(Node::new(tag, &cx.ptr(p)));
                fams.entry(p.clone()).or_default().push(fid.clone());
            }
        }
        for k in &kids {
            let pid = k["person_id"].as_str().unwrap_or("").to_string();
            n.children.push(Node::new("CHIL", &cx.ptr(&pid)));
            famc.entry(pid)
                .or_default()
                .push((fid.clone(), sv(k, "rel_type").unwrap_or_default()));
        }
        let partner_living = [&p1, &p2]
            .iter()
            .any(|p| p.as_deref().map(alive_in).unwrap_or(false));
        if !(o.living == LivingPolicy::Mask && partner_living) {
            for e in sorted_events(&tkey("family", &fid)) {
                n.children.push(cx.event_node(&e));
            }
            cx.common(&mut n, "family", &fid);
        } else {
            cx.raw_into(&mut Node::default(), "", "");
        }
        fam_nodes.push(n);
    }

    let mut indi_nodes = Vec::new();
    for p in &persons {
        let pid = sid(p);
        if hidden(&pid) {
            continue;
        }
        let mut n = Node::new("INDI", "");
        n.xref = Some(cx.ptr(&pid));
        let mask = masked(&pid);
        if mask {
            n.children.push(Node::new("NAME", "Living"));
        } else {
            for nm in names.get(&pid).into_iter().flatten() {
                n.children.push(cx.name_node(nm));
            }
        }
        if let Some(sx) = sv(p, "sex") {
            n.children.push(Node::new("SEX", &sx));
        }
        if !mask {
            if si(p, "is_private") == 1 {
                n.children.push(Node::new("RESN", "PRIVACY"));
            }
            if let Some(r) = sv(p, "ref_no") {
                n.children.push(Node::new("REFN", &r));
            }
            for e in sorted_events(&tkey("person", &pid)) {
                n.children.push(cx.event_node(&e));
            }
        }
        for (fid, pedi) in famc.get(&pid).into_iter().flatten() {
            let mut c = Node::new("FAMC", &cx.ptr(fid));
            if !pedi.is_empty() {
                c.children.push(Node::new("PEDI", pedi));
            }
            let key = format!("{}|{}", pid, cx.ptr(fid));
            cx.raw_into(&mut c, "famc", &key);
            n.children.push(c);
        }
        for fid in fams.get(&pid).into_iter().flatten() {
            let mut c = Node::new("FAMS", &cx.ptr(fid));
            let key = format!("{}|{}", pid, cx.ptr(fid));
            cx.raw_into(&mut c, "fams", &key);
            n.children.push(c);
        }
        if !mask {
            for a in assos.get(&pid).into_iter().flatten() {
                let other = a["other_id"].as_str().unwrap_or("");
                if hidden(other) {
                    continue;
                }
                let mut c = Node::new("ASSO", &cx.ptr(other));
                if let Some(r) = sv(a, "role") {
                    c.children.push(Node::new("RELA", &r));
                }
                cx.common(&mut c, "association", &sid(a));
                n.children.push(c);
            }
            cx.common(&mut n, "person", &pid);
        }
        indi_nodes.push(n);
    }

    // Records that stand alone (not inline) — skipped entirely when masking, since links were dropped.
    let mut other_nodes = Vec::new();
    if o.include_media {
        for m in store.rows("media")? {
            if si(&m, "inline") == 0 {
                other_nodes.push(cx.media_node(&m, Some(cx.ptr(&sid(&m)))));
            }
        }
    }
    for r in store.rows("note")? {
        if si(&r, "inline") == 0 {
            let id = sid(&r);
            let mut n = Node::new(cx.shared_note_tag(), r["body"].as_str().unwrap_or(""));
            n.xref = Some(cx.ptr(&id));
            cx.common(&mut n, "note", &id);
            other_nodes.push(n);
        }
    }
    for r in store.rows("source")? {
        if si(&r, "inline") == 0 {
            let id = sid(&r);
            let mut n = Node::new("SOUR", "");
            n.xref = Some(cx.ptr(&id));
            for (tag, col) in [
                ("TITL", "title"),
                ("AUTH", "author"),
                ("PUBL", "publication"),
                ("TEXT", "text"),
            ] {
                if let Some(v) = sv(&r, col) {
                    n.children.push(Node::new(tag, &v));
                }
            }
            if let Some(rid) = sv(&r, "repository_id") {
                let mut c = Node::new("REPO", &cx.ptr(&rid));
                cx.raw_into(&mut c, "source_repo", &id);
                n.children.push(c);
            }
            cx.common(&mut n, "source", &id);
            other_nodes.push(n);
        }
    }
    for r in store.rows("repository")? {
        let id = sid(&r);
        let mut n = Node::new("REPO", "");
        n.xref = Some(cx.ptr(&id));
        n.children
            .push(Node::new("NAME", r["name"].as_str().unwrap_or("")));
        if let Some(a) = sv(&r, "address") {
            let mut c = Node::new("ADDR", &a);
            cx.raw_into(&mut c, "repository_addr", &id);
            n.children.push(c);
        }
        if let Some(w) = sv(&r, "website") {
            n.children.push(Node::new("WWW", &w));
        }
        cx.common(&mut n, "repository", &id);
        other_nodes.push(n);
    }

    let mut out: Vec<Node> = Vec::new();
    let (head, subm) = build_head(o, head_raw, &record_raws);
    out.push(head);
    if let Some(s) = subm {
        out.push(s);
    }
    // Submitter records directly follow the header so output is stable across re-imports.
    let (submitters, record_raws): (Vec<Node>, Vec<Node>) =
        record_raws.into_iter().partition(|r| r.tag == "SUBM");
    out.extend(submitters);
    out.extend(indi_nodes);
    out.extend(fam_nodes);
    out.extend(other_nodes);
    out.extend(record_raws);
    out.push(Node::new("TRLR", ""));
    let opts = tree::WriteOpts {
        max_len: if o.version == Version::V70 { 0 } else { 248 },
    };
    Ok(tree::write(&out, &opts))
}

fn build_head(o: &ExportOptions, raw: Option<Node>, records: &[Node]) -> (Node, Option<Node>) {
    let mut head = raw.unwrap_or_else(|| Node::new("HEAD", ""));
    head.xref = None;
    head.children.retain(|c| c.tag != "GEDC" && c.tag != "CHAR");
    let (name, ver) = match o.dialect {
        Dialect::Standard => ("KinTree", "0.1"),
        Dialect::Ancestry => ("Ancestry.com Family Trees", "2010.3"),
        Dialect::FamilyTreeMaker => ("FTM", "22.0"),
        Dialect::RootsMagic => ("RootsMagic", "8.0"),
        Dialect::Legacy => ("Legacy", "9.0"),
        Dialect::Gramps => ("Gramps", "5.1"),
    };
    if o.dialect != Dialect::Standard || !head.children.iter().any(|c| c.tag == "SOUR") {
        head.children.retain(|c| c.tag != "SOUR");
        head.children.insert(
            0,
            Node::new("SOUR", name)
                .with(Node::new("VERS", ver))
                .with(Node::new("NAME", name)),
        );
    }
    let gedc = Node::new("GEDC", "").with(Node::new(
        "VERS",
        if o.version == Version::V70 {
            "7.0"
        } else {
            "5.5.1"
        },
    ));
    let gedc = if o.version == Version::V70 {
        gedc
    } else {
        gedc.with(Node::new("FORM", "LINEAGE-LINKED"))
    };
    let at = head
        .children
        .iter()
        .position(|c| c.tag == "SOUR")
        .map(|i| i + 1)
        .unwrap_or(0);
    head.children.insert(at, gedc);
    if o.version != Version::V70 {
        head.children.insert(
            at + 1,
            Node::new("CHAR", charset::gedcom_char_name(o.charset)),
        );
    }
    let mut subm = None;
    if o.version != Version::V70 && !head.children.iter().any(|c| c.tag == "SUBM") {
        let existing = records
            .iter()
            .find(|r| r.tag == "SUBM")
            .and_then(|r| r.xref.clone());
        match existing {
            Some(x) => head.children.push(Node::new("SUBM", &x)),
            None => {
                head.children.push(Node::new("SUBM", "@SUBM@"));
                let mut s = Node::new("SUBM", "").with(Node::new("NAME", "KinTree user"));
                s.xref = Some("@SUBM@".into());
                subm = Some(s);
            }
        }
    }
    (head, subm)
}
