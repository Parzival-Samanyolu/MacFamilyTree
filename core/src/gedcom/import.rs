use super::charset::{self, Charset};
use super::tree::{self, Node};
use super::{Issue, Severity};
use crate::date::GenDate;
use crate::model::EventRec;
use crate::store::{new_id, Result, Row, Store, Tx};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub const INDI_EVENTS: &[&str] = &[
    "BIRT", "CHR", "DEAT", "BURI", "CREM", "ADOP", "BAPM", "BARM", "BASM", "BLES", "CHRA", "CONF",
    "FCOM", "ORDN", "NATU", "EMIG", "IMMI", "CENS", "PROB", "WILL", "GRAD", "RETI", "EVEN", "CAST",
    "DSCR", "EDUC", "IDNO", "NATI", "NCHI", "OCCU", "PROP", "RELI", "RESI", "SSN", "TITL", "FACT",
];
pub const FAM_EVENTS: &[&str] = &[
    "ANUL", "CENS", "DIV", "DIVF", "ENGA", "MARB", "MARC", "MARR", "MARL", "MARS", "EVEN", "RESI",
    "NCHI",
];

#[derive(Debug, Default, Clone)]
pub struct ImportReport {
    pub issues: Vec<Issue>,
    pub version: String,
    pub charset: Option<Charset>,
    pub persons: usize,
    pub families: usize,
    pub events: usize,
    pub places: usize,
    pub sources: usize,
    pub repositories: usize,
    pub notes: usize,
    pub media: usize,
    pub raw_structures: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Indi,
    Fam,
    Sour,
    Repo,
    Note,
    Obje,
}

struct Cx {
    ids: HashMap<String, (Kind, String)>,
    places: HashMap<(String, String), String>,
    place_has_coords: HashSet<String>,
    pedi: HashMap<(String, String), String>,
    rep: ImportReport,
    seq: i64,
}

impl Cx {
    fn warn(&mut self, line: usize, m: String) {
        self.rep.issues.push(Issue::new(Severity::Warning, line, m));
    }
    fn lookup(&mut self, node: &Node, kind: Kind, what: &str) -> Option<String> {
        match self.ids.get(&node.value) {
            Some((k, id)) if *k == kind => Some(id.clone()),
            Some(_) => {
                self.warn(
                    node.line,
                    format!(
                        "pointer {} does not reference a {} record",
                        node.value, what
                    ),
                );
                None
            }
            None => {
                self.warn(
                    node.line,
                    format!("dangling pointer {} (no such {} record)", node.value, what),
                );
                None
            }
        }
    }
}

fn put(tx: &mut Tx, table: &str, id: &str, f: &[(&str, Value)]) -> Result<()> {
    let mut m = Row::new();
    m.insert("id".into(), id.into());
    for (k, v) in f {
        m.insert(k.to_string(), v.clone());
    }
    tx.put_row(table, m)?;
    Ok(())
}

fn s(v: &str) -> Value {
    Value::from(v)
}
fn opt(v: Option<&str>) -> Value {
    v.map(Value::from).unwrap_or(Value::Null)
}
fn bare_xref(x: &str) -> String {
    x.trim_matches('@').to_string()
}

fn take<'a>(node: &'a Node, used: &mut [bool], tag: &str) -> Option<&'a Node> {
    for (i, c) in node.children.iter().enumerate() {
        if !used[i] && c.tag == tag {
            used[i] = true;
            return Some(c);
        }
    }
    None
}

fn raw(cx: &mut Cx, tx: &mut Tx, owner_type: &str, owner_id: &str, n: &Node) -> Result<()> {
    cx.seq += 1;
    cx.rep.raw_structures += 1;
    let line = tree::write(std::slice::from_ref(n), &tree::WriteOpts { max_len: 0 });
    put(
        tx,
        "raw_tag",
        &new_id(),
        &[
            ("owner_type", s(owner_type)),
            ("owner_id", s(owner_id)),
            ("seq", cx.seq.into()),
            ("line", s(line.trim_end_matches('\n'))),
        ],
    )
}

fn raw_rest(
    cx: &mut Cx,
    tx: &mut Tx,
    node: &Node,
    used: &[bool],
    owner_type: &str,
    owner_id: &str,
) -> Result<()> {
    for (i, c) in node.children.iter().enumerate() {
        if !used[i] {
            raw(cx, tx, owner_type, owner_id, c)?;
        }
    }
    Ok(())
}

/// NOTE / SNOTE / SOUR / OBJE children shared by almost every structure.
fn common(
    cx: &mut Cx,
    tx: &mut Tx,
    node: &Node,
    used: &mut [bool],
    ttype: &str,
    tid: &str,
) -> Result<()> {
    for (i, c) in node.children.iter().enumerate() {
        if used[i] {
            continue;
        }
        match c.tag.as_str() {
            "NOTE" | "SNOTE" => {
                used[i] = true;
                let note_id = if c.is_pointer() {
                    match cx.lookup(c, Kind::Note, "NOTE") {
                        Some(id) => id,
                        None => continue,
                    }
                } else {
                    let id = new_id();
                    put(
                        tx,
                        "note",
                        &id,
                        &[("body", s(&c.value)), ("inline", 1.into())],
                    )?;
                    cx.rep.notes += 1;
                    let mut u = vec![false; c.children.len()];
                    common(cx, tx, c, &mut u, "note", &id)?;
                    raw_rest(cx, tx, c, &u, "note", &id)?;
                    id
                };
                put(
                    tx,
                    "note_link",
                    &new_id(),
                    &[
                        ("note_id", s(&note_id)),
                        ("target_type", s(ttype)),
                        ("target_id", s(tid)),
                    ],
                )?;
            }
            "SOUR" => {
                used[i] = true;
                let source_id = if c.is_pointer() {
                    match cx.lookup(c, Kind::Sour, "SOURCE") {
                        Some(id) => id,
                        None => continue,
                    }
                } else {
                    let id = new_id();
                    put(
                        tx,
                        "source",
                        &id,
                        &[("title", s(&c.value)), ("inline", 1.into())],
                    )?;
                    cx.rep.sources += 1;
                    id
                };
                let cid = new_id();
                let mut u = vec![false; c.children.len()];
                let page = take(c, &mut u, "PAGE").map(|n| n.value.clone());
                let quay = take(c, &mut u, "QUAY").map(|n| n.value.clone());
                put(
                    tx,
                    "citation",
                    &cid,
                    &[
                        ("source_id", s(&source_id)),
                        ("target_type", s(ttype)),
                        ("target_id", s(tid)),
                        ("page", opt(page.as_deref())),
                        ("quality", opt(quay.as_deref())),
                    ],
                )?;
                common(cx, tx, c, &mut u, "citation", &cid)?;
                raw_rest(cx, tx, c, &u, "citation", &cid)?;
            }
            "OBJE" => {
                used[i] = true;
                let mid = if c.is_pointer() {
                    match cx.lookup(c, Kind::Obje, "OBJE") {
                        Some(id) => id,
                        None => continue,
                    }
                } else {
                    let id = new_id();
                    import_media(cx, tx, c, &id, true)?;
                    id
                };
                put(
                    tx,
                    "media_link",
                    &new_id(),
                    &[
                        ("media_id", s(&mid)),
                        ("target_type", s(ttype)),
                        ("target_id", s(tid)),
                    ],
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn import_media(cx: &mut Cx, tx: &mut Tx, node: &Node, id: &str, inline: bool) -> Result<()> {
    let mut used = vec![false; node.children.len()];
    let file = take(node, &mut used, "FILE");
    let (path, form, titl, file_idx_used);
    match file {
        Some(f) => {
            let mut fu = vec![false; f.children.len()];
            form = take(f, &mut fu, "FORM").map(|n| n.value.clone());
            titl = take(f, &mut fu, "TITL").map(|n| n.value.clone());
            path = f.value.clone();
            file_idx_used = fu;
        }
        None => {
            path = String::new();
            form = None;
            titl = None;
            file_idx_used = vec![];
        }
    }
    let titl = titl.or_else(|| take(node, &mut used, "TITL").map(|n| n.value.clone()));
    let form = form.or_else(|| take(node, &mut used, "FORM").map(|n| n.value.clone()));
    let xref = node.xref.as_deref().map(bare_xref);
    put(
        tx,
        "media",
        id,
        &[
            ("xref", opt(xref.as_deref())),
            ("kind", opt(form.as_deref())),
            ("path", s(&path)),
            ("caption", opt(titl.as_deref())),
            ("inline", (inline as i64).into()),
        ],
    )?;
    cx.rep.media += 1;
    if let Some(f) = file {
        raw_rest(cx, tx, f, &file_idx_used, "media_file", id)?;
    }
    common(cx, tx, node, &mut used, "media", id)?;
    raw_rest(cx, tx, node, &used, "media", id)
}

fn parse_coord(v: &str) -> Option<f64> {
    let v = v.trim();
    let (sign, num) = match v.chars().next()? {
        'N' | 'E' => (1.0, &v[1..]),
        'S' | 'W' => (-1.0, &v[1..]),
        _ => (1.0, v),
    };
    num.parse::<f64>().ok().map(|x| x * sign)
}

fn import_place(
    cx: &mut Cx,
    tx: &mut Tx,
    plac: &Node,
    event_id: &str,
) -> Result<(Option<String>, bool)> {
    let mut used = vec![false; plac.children.len()];
    let mut leaf = None;
    let mut had_map = false;
    if !plac.value.trim().is_empty() {
        let mut parent = String::new();
        for name in plac.value.split(',').map(|x| x.trim().to_string()).rev() {
            let key = (parent.clone(), name.clone());
            let id = match cx.places.get(&key) {
                Some(id) => id.clone(),
                None => {
                    let id = new_id();
                    put(
                        tx,
                        "place",
                        &id,
                        &[
                            (
                                "parent_id",
                                if parent.is_empty() {
                                    Value::Null
                                } else {
                                    s(&parent)
                                },
                            ),
                            ("name", s(&name)),
                        ],
                    )?;
                    cx.places.insert(key, id.clone());
                    cx.rep.places += 1;
                    id
                }
            };
            parent = id;
        }
        leaf = Some(parent);
    }
    if let (Some(leaf_id), Some(map)) = (&leaf, plac.children.iter().position(|c| c.tag == "MAP")) {
        let m = &plac.children[map];
        let lat = m.child_value("LATI").and_then(parse_coord);
        let lon = m.child_value("LONG").and_then(parse_coord);
        if let (Some(lat), Some(lon)) = (lat, lon) {
            if m.children.len() == 2 {
                used[map] = true;
                had_map = true;
                if cx.place_has_coords.insert(leaf_id.clone()) {
                    put(
                        tx,
                        "place",
                        leaf_id,
                        &[("lat", lat.into()), ("lon", lon.into())],
                    )?;
                }
            }
        }
    }
    raw_rest(cx, tx, plac, &used, "event_plac", event_id)?;
    Ok((leaf, had_map))
}

fn import_event(
    cx: &mut Cx,
    tx: &mut Tx,
    node: &Node,
    owner_type: &str,
    owner_id: &str,
    order: i64,
) -> Result<()> {
    let id = new_id();
    let mut used = vec![false; node.children.len()];
    let custom = take(node, &mut used, "TYPE").map(|n| n.value.clone());
    let date = take(node, &mut used, "DATE").map(|n| n.value.clone());
    let plac = take(node, &mut used, "PLAC");
    let caus = take(node, &mut used, "CAUS").map(|n| n.value.clone());
    let agnc = take(node, &mut used, "AGNC").map(|n| n.value.clone());
    let (place_id, had_map) = match plac {
        Some(p) => import_place(cx, tx, p, &id)?,
        None => (None, false),
    };
    let mut e = EventRec {
        id: id.clone(),
        owner_type: owner_type.into(),
        owner_id: owner_id.into(),
        kind: node.tag.clone(),
        custom_kind: custom,
        value: if node.value.is_empty() {
            None
        } else {
            Some(node.value.clone())
        },
        place_id,
        cause: caus,
        agency: agnc,
        plac_map: Some(had_map as i64),
        sort_order: order,
        ..Default::default()
    };
    if let Some(d) = date.as_deref().and_then(GenDate::parse_lenient) {
        if d.verbatim {
            cx.warn(
                node.line,
                format!(
                    "unparseable date {:?} kept as text",
                    date.as_deref().unwrap_or("")
                ),
            );
        }
        e.set_date(Some(&d));
    }
    tx.put("event", &e)?;
    cx.rep.events += 1;
    common(cx, tx, node, &mut used, "event", &id)?;
    raw_rest(cx, tx, node, &used, "event", &id)
}

fn import_indi(cx: &mut Cx, tx: &mut Tx, node: &Node) -> Result<()> {
    let id = cx.ids[node.xref.as_ref().unwrap()].1.clone();
    let mut used = vec![false; node.children.len()];
    let sex = take(node, &mut used, "SEX")
        .map(|n| n.value.clone())
        .unwrap_or_default();
    let refn = take(node, &mut used, "REFN").map(|n| n.value.clone());
    let private = node
        .children
        .iter()
        .enumerate()
        .find(|(i, c)| !used[*i] && c.tag == "RESN" && c.value.eq_ignore_ascii_case("PRIVACY"))
        .map(|(i, _)| i);
    if let Some(i) = private {
        used[i] = true;
    }
    put(
        tx,
        "person",
        &id,
        &[
            ("xref", s(&bare_xref(node.xref.as_deref().unwrap()))),
            ("sex", s(&sex)),
            ("ref_no", opt(refn.as_deref())),
            ("is_private", (private.is_some() as i64).into()),
        ],
    )?;
    cx.rep.persons += 1;
    let mut order = 0;
    for (i, c) in node.children.iter().enumerate() {
        if used[i] {
            continue;
        }
        match c.tag.as_str() {
            "NAME" => {
                used[i] = true;
                import_name(cx, tx, c, &id, order)?;
                order += 1;
            }
            t if INDI_EVENTS.contains(&t) => {
                used[i] = true;
                import_event(cx, tx, c, "person", &id, order)?;
                order += 1;
            }
            "FAMC" | "FAMS" => {
                used[i] = true;
                if c.tag == "FAMC" {
                    if let Some(p) = c.child_value("PEDI") {
                        cx.pedi
                            .insert((node.xref.clone().unwrap(), c.value.clone()), p.to_string());
                    }
                }
                let extra: Vec<&Node> = c.children.iter().filter(|x| x.tag != "PEDI").collect();
                for x in extra {
                    raw(
                        cx,
                        tx,
                        if c.tag == "FAMC" { "famc" } else { "fams" },
                        &format!("{}|{}", id, c.value),
                        x,
                    )?;
                }
            }
            "ASSO" => {
                used[i] = true;
                if let Some(other) = cx.lookup(c, Kind::Indi, "INDI") {
                    let aid = new_id();
                    let mut u = vec![false; c.children.len()];
                    let rela = take(c, &mut u, "RELA")
                        .map(|n| n.value.clone())
                        .unwrap_or_default();
                    put(
                        tx,
                        "association",
                        &aid,
                        &[
                            ("person_id", s(&id)),
                            ("other_id", s(&other)),
                            ("role", s(&rela)),
                        ],
                    )?;
                    common(cx, tx, c, &mut u, "association", &aid)?;
                    raw_rest(cx, tx, c, &u, "association", &aid)?;
                }
            }
            _ => {}
        }
    }
    common(cx, tx, node, &mut used, "person", &id)?;
    raw_rest(cx, tx, node, &used, "person", &id)
}

fn import_name(cx: &mut Cx, tx: &mut Tx, node: &Node, pid: &str, order: i64) -> Result<()> {
    let mut used = vec![false; node.children.len()];
    let mut n = crate::name::PersonName::from_gedcom(&node.value);
    let mut present: Vec<&str> = Vec::new();
    let mut get = |tag: &'static str, used: &mut [bool]| {
        let v = take(node, used, tag).map(|c| c.value.clone());
        if v.is_some() && tag != "TYPE" {
            present.push(tag);
        }
        v
    };
    if let Some(v) = get("NPFX", &mut used) {
        n.prefix = v;
        // the NAME line repeats the prefix in the given part; avoid duplicating it
        if let Some(rest) = n.given.strip_prefix(&n.prefix) {
            n.given = rest.trim().to_string();
        }
    }
    if let Some(v) = get("GIVN", &mut used) {
        n.given = v;
    }
    if let Some(v) = get("NICK", &mut used) {
        n.nickname = v;
    }
    if let Some(v) = get("SPFX", &mut used) {
        n.surname_prefix = v;
        if let Some(rest) = n.surname.strip_prefix(&n.surname_prefix) {
            n.surname = rest.trim().to_string();
        }
    }
    if let Some(v) = get("SURN", &mut used) {
        n.surname = v;
    }
    if let Some(v) = get("NSFX", &mut used) {
        n.suffix = v;
    }
    let kind = get("TYPE", &mut used).unwrap_or_default();
    let src_tags = present.join(",");
    let id = new_id();
    put(
        tx,
        "person_name",
        &id,
        &[
            ("person_id", s(pid)),
            ("kind", s(&kind)),
            ("prefix", s(&n.prefix)),
            ("given", s(&n.given)),
            ("nickname", s(&n.nickname)),
            ("surname_prefix", s(&n.surname_prefix)),
            ("surname", s(&n.surname)),
            ("suffix", s(&n.suffix)),
            ("src_tags", s(&src_tags)),
            ("sort_order", order.into()),
        ],
    )?;
    common(cx, tx, node, &mut used, "person_name", &id)?;
    raw_rest(cx, tx, node, &used, "person_name", &id)
}

fn import_fam(cx: &mut Cx, tx: &mut Tx, node: &Node) -> Result<()> {
    let xref = node.xref.clone().unwrap();
    let id = cx.ids[&xref].1.clone();
    let mut used = vec![false; node.children.len()];
    let partner = |tag: &str, cx: &mut Cx, used: &mut [bool]| -> Option<String> {
        let c = take(node, used, tag)?;
        cx.lookup(c, Kind::Indi, "INDI")
    };
    let p1 = partner("HUSB", cx, &mut used);
    let p2 = partner("WIFE", cx, &mut used);
    let has_marr = node.children.iter().any(|c| c.tag == "MARR");
    put(
        tx,
        "family",
        &id,
        &[
            ("xref", s(&bare_xref(&xref))),
            ("partner1", p1.map(Value::from).unwrap_or(Value::Null)),
            ("partner2", p2.map(Value::from).unwrap_or(Value::Null)),
            ("rel_type", s(if has_marr { "married" } else { "" })),
        ],
    )?;
    cx.rep.families += 1;
    let mut order = 0;
    let mut seen_child: HashSet<String> = HashSet::new();
    for (i, c) in node.children.iter().enumerate() {
        if used[i] {
            continue;
        }
        if c.tag == "CHIL" {
            used[i] = true;
            if let Some(pid) = cx.lookup(c, Kind::Indi, "INDI") {
                if seen_child.insert(pid.clone()) {
                    let pedi = cx
                        .pedi
                        .get(&(c.value.clone(), xref.clone()))
                        .cloned()
                        .unwrap_or_default();
                    put(
                        tx,
                        "family_child",
                        &new_id(),
                        &[
                            ("family_id", s(&id)),
                            ("person_id", s(&pid)),
                            ("rel_type", s(&pedi)),
                            ("sort_order", order.into()),
                        ],
                    )?;
                    order += 1;
                }
            }
        } else if FAM_EVENTS.contains(&c.tag.as_str()) {
            used[i] = true;
            import_event(cx, tx, c, "family", &id, order)?;
            order += 1;
        }
    }
    common(cx, tx, node, &mut used, "family", &id)?;
    raw_rest(cx, tx, node, &used, "family", &id)
}

fn import_source(cx: &mut Cx, tx: &mut Tx, node: &Node) -> Result<()> {
    let xref = node.xref.clone().unwrap();
    let id = cx.ids[&xref].1.clone();
    let mut used = vec![false; node.children.len()];
    let title = take(node, &mut used, "TITL").map(|n| n.value.clone());
    let auth = take(node, &mut used, "AUTH").map(|n| n.value.clone());
    let publ = take(node, &mut used, "PUBL").map(|n| n.value.clone());
    let text = take(node, &mut used, "TEXT").map(|n| n.value.clone());
    let repo = take(node, &mut used, "REPO");
    let mut repo_id = None;
    if let Some(r) = repo {
        repo_id = cx.lookup(r, Kind::Repo, "REPO");
        for x in &r.children {
            raw(cx, tx, "source_repo", &id, x)?;
        }
    }
    put(
        tx,
        "source",
        &id,
        &[
            ("xref", s(&bare_xref(&xref))),
            ("title", s(title.as_deref().unwrap_or(""))),
            ("author", opt(auth.as_deref())),
            ("publication", opt(publ.as_deref())),
            ("text", opt(text.as_deref())),
            (
                "repository_id",
                repo_id.map(Value::from).unwrap_or(Value::Null),
            ),
        ],
    )?;
    cx.rep.sources += 1;
    common(cx, tx, node, &mut used, "source", &id)?;
    raw_rest(cx, tx, node, &used, "source", &id)
}

fn import_repo(cx: &mut Cx, tx: &mut Tx, node: &Node) -> Result<()> {
    let xref = node.xref.clone().unwrap();
    let id = cx.ids[&xref].1.clone();
    let mut used = vec![false; node.children.len()];
    let name = take(node, &mut used, "NAME").map(|n| n.value.clone());
    let www = take(node, &mut used, "WWW").map(|n| n.value.clone());
    let addr = take(node, &mut used, "ADDR");
    if let Some(a) = addr {
        for x in &a.children {
            raw(cx, tx, "repository_addr", &id, x)?;
        }
    }
    put(
        tx,
        "repository",
        &id,
        &[
            ("xref", s(&bare_xref(&xref))),
            ("name", s(name.as_deref().unwrap_or(""))),
            ("address", opt(addr.map(|a| a.value.as_str()))),
            ("website", opt(www.as_deref())),
        ],
    )?;
    cx.rep.repositories += 1;
    common(cx, tx, node, &mut used, "repository", &id)?;
    raw_rest(cx, tx, node, &used, "repository", &id)
}

fn import_note_record(cx: &mut Cx, tx: &mut Tx, node: &Node) -> Result<()> {
    let xref = node.xref.clone().unwrap();
    let id = cx.ids[&xref].1.clone();
    put(
        tx,
        "note",
        &id,
        &[
            ("xref", s(&bare_xref(&xref))),
            ("body", s(&node.value)),
            ("inline", 0.into()),
        ],
    )?;
    cx.rep.notes += 1;
    let mut used = vec![false; node.children.len()];
    common(cx, tx, node, &mut used, "note", &id)?;
    raw_rest(cx, tx, node, &used, "note", &id)
}

pub fn import(store: &mut Store, bytes: &[u8]) -> Result<ImportReport> {
    let mut notes = Vec::new();
    let (text, cs) = charset::decode(bytes, &mut |m| notes.push(m));
    let mut issues: Vec<Issue> = notes
        .into_iter()
        .map(|m| Issue::new(Severity::Warning, 0, m))
        .collect();
    let records = tree::parse(&text, &mut issues);

    let version = records
        .iter()
        .find(|r| r.tag == "HEAD")
        .and_then(|h| h.child("GEDC"))
        .and_then(|g| g.child_value("VERS"))
        .unwrap_or("")
        .to_string();
    if version.is_empty() {
        issues.push(Issue::new(
            Severity::Warning,
            0,
            "no GEDCOM version in header; assuming 5.5.1".into(),
        ));
    }
    let mut cx = Cx {
        ids: HashMap::new(),
        places: HashMap::new(),
        place_has_coords: HashSet::new(),
        pedi: HashMap::new(),
        rep: ImportReport {
            issues,
            version: if version.is_empty() {
                "5.5.1".into()
            } else {
                version
            },
            charset: Some(cs),
            ..Default::default()
        },
        seq: 0,
    };
    // Pass 1: allocate ids for every addressable record.
    for r in &records {
        let kind = match r.tag.as_str() {
            "INDI" => Kind::Indi,
            "FAM" => Kind::Fam,
            "SOUR" => Kind::Sour,
            "REPO" => Kind::Repo,
            "NOTE" | "SNOTE" => Kind::Note,
            "OBJE" => Kind::Obje,
            _ => continue,
        };
        match &r.xref {
            Some(x) => {
                if cx.ids.insert(x.clone(), (kind, new_id())).is_some() {
                    cx.warn(
                        r.line,
                        format!("duplicate xref {}; later record shadows earlier", x),
                    );
                }
            }
            None => cx.warn(r.line, format!("{} record without xref ignored", r.tag)),
        }
    }
    // PEDI values live on INDI.FAMC; collect before families are processed.
    store.transact_untracked(|tx| {
        for r in records
            .iter()
            .filter(|r| r.tag == "INDI" && r.xref.is_some())
        {
            for c in r.children_of("FAMC") {
                if let Some(p) = c.child_value("PEDI") {
                    cx.pedi
                        .insert((r.xref.clone().unwrap(), c.value.clone()), p.to_string());
                }
            }
        }
        for r in &records {
            match (r.tag.as_str(), &r.xref) {
                ("HEAD", _) => raw(&mut cx, tx, "head", "head", r)?,
                ("TRLR", _) => {}
                ("INDI", Some(_)) => import_indi(&mut cx, tx, r)?,
                ("SOUR", Some(_)) => import_source(&mut cx, tx, r)?,
                ("REPO", Some(_)) => import_repo(&mut cx, tx, r)?,
                ("NOTE" | "SNOTE", Some(_)) => import_note_record(&mut cx, tx, r)?,
                ("OBJE", Some(x)) => {
                    let id = cx.ids[x].1.clone();
                    import_media(&mut cx, tx, r, &id, false)?;
                }
                ("INDI" | "FAM" | "SOUR" | "REPO" | "NOTE" | "SNOTE" | "OBJE", None) => {}
                ("FAM", Some(_)) => {}
                _ => {
                    cx.rep.issues.push(Issue::new(
                        Severity::Info,
                        r.line,
                        format!("record {} preserved verbatim", r.tag),
                    ));
                    raw(&mut cx, tx, "record", "", r)?;
                }
            }
        }
        // Families after individuals so pointer lookups and PEDI info are complete.
        for r in records
            .iter()
            .filter(|r| r.tag == "FAM" && r.xref.is_some())
        {
            import_fam(&mut cx, tx, r)?;
        }
        // INDI.FAMC without matching FAM.CHIL (vendor quirk): add the child link.
        for r in records
            .iter()
            .filter(|r| r.tag == "INDI" && r.xref.is_some())
        {
            let pid = cx.ids[r.xref.as_ref().unwrap()].1.clone();
            for c in r.children_of("FAMC") {
                let Some((Kind::Fam, fid)) = cx.ids.get(&c.value).cloned() else {
                    if !cx.ids.contains_key(&c.value) {
                        cx.warn(c.line, format!("FAMC points to unknown family {}", c.value));
                    }
                    continue;
                };
                let existing = tx.ids_where("family_child", "family_id", &fid)?;
                let mut found = false;
                for e in &existing {
                    if tx
                        .get("family_child", e)?
                        .map(|row| row["person_id"].as_str() == Some(pid.as_str()))
                        .unwrap_or(false)
                    {
                        found = true;
                        break;
                    }
                }
                if !found {
                    cx.warn(
                        c.line,
                        format!(
                            "{} lists FAMC {} but family has no matching CHIL; link added",
                            r.xref.as_deref().unwrap(),
                            c.value
                        ),
                    );
                    let pedi = cx
                        .pedi
                        .get(&(r.xref.clone().unwrap(), c.value.clone()))
                        .cloned()
                        .unwrap_or_default();
                    put(
                        tx,
                        "family_child",
                        &new_id(),
                        &[
                            ("family_id", s(&fid)),
                            ("person_id", s(&pid)),
                            ("rel_type", s(&pedi)),
                            ("sort_order", (existing.len() as i64).into()),
                        ],
                    )?;
                }
            }
        }
        Ok(())
    })?;
    store.conn().execute(
        "INSERT OR REPLACE INTO setting (id, value) VALUES ('gedcom_version', ?1)",
        [&cx.rep.version],
    )?;
    Ok(cx.rep)
}
