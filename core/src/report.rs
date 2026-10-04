//! Report engine: narrative sentences from user-editable templates, footnoted citations, name index,
//! privacy filtering, and HTML / Markdown rendering. Reports are built as a small block model so new
//! output formats only need a renderer.

use crate::date::{Calendar, GenDate, Locale, Qualifier};
use crate::facts::{EventFact, Facts};
use crate::model::living_ids;
use crate::relationship::{ahnentafel, birth_keys, number_descendants, DescendantNumbering, Sex};
use crate::store::{Result, Store};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportLang {
    En,
    Tr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Privacy {
    /// Show everyone.
    Off,
    /// Living people appear as "Living" with no details.
    Mask,
    /// Living people are left out.
    Exclude,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub lang: ReportLang,
    pub privacy: Privacy,
    pub current_year: i32,
    pub living_years: i32,
    pub generations: usize,
    /// Overrides for sentence templates, keyed by template name (see [`default_template`]).
    pub templates: HashMap<String, String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            lang: ReportLang::En,
            privacy: Privacy::Off,
            current_year: 2026,
            living_years: 110,
            generations: 4,
            templates: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        text: String,
        anchor: Option<String>,
    },
    Para(String),
    Table {
        header: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    List(Vec<String>),
    /// Picture with a `data:` URI (stories); Markdown output keeps only the caption.
    Image {
        src: String,
        alt: String,
        caption: Option<String>,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    pub title: String,
    pub lang: String,
    pub blocks: Vec<Block>,
    /// (display name, anchor) for the index of names, sorted.
    pub index: Vec<(String, String)>,
    pub footnotes: Vec<String>,
}

// ---------------- templates ----------------

/// Built-in sentence templates. Placeholders: `{name}` `{He}` `{he}` `{his}` `{date}` `{place}` `{spouse}` `{n}` `{children}` `{value}` `{age}` `{father}` `{mother}`.
pub fn default_template(lang: ReportLang, key: &str) -> &'static str {
    match (lang, key) {
        (ReportLang::En, "birth_full") => "{name} was born {date} in {place}.",
        (ReportLang::En, "birth_date") => "{name} was born {date}.",
        (ReportLang::En, "birth_place") => "{name} was born in {place}.",
        (ReportLang::En, "parents") => "{He} was the child of {father} and {mother}.",
        (ReportLang::En, "parent_one") => "{He} was the child of {father}.",
        (ReportLang::En, "marriage_full") => "{He} married {spouse} {date} in {place}.",
        (ReportLang::En, "marriage_date") => "{He} married {spouse} {date}.",
        (ReportLang::En, "marriage_place") => "{He} married {spouse} in {place}.",
        (ReportLang::En, "marriage") => "{He} married {spouse}.",
        (ReportLang::En, "partner") => "{He} had a relationship with {spouse}.",
        (ReportLang::En, "children_one") => "{They} had one child: {children}.",
        (ReportLang::En, "children") => "{They} had {n} children: {children}.",
        (ReportLang::En, "occupation") => "{He} worked as {value}.",
        (ReportLang::En, "occupation_date") => "{He} worked as {value} {date}.",
        (ReportLang::En, "residence") => "{He} lived in {place}{date}.",
        (ReportLang::En, "death_full") => "{He} died {date} in {place}{age}.",
        (ReportLang::En, "death_date") => "{He} died {date}{age}.",
        (ReportLang::En, "death_place") => "{He} died in {place}.",
        (ReportLang::En, "burial") => "{He} was buried in {place}.",

        (ReportLang::Tr, "birth_full") => "{name}, {date} {place_loc} doğdu.",
        (ReportLang::Tr, "birth_date") => "{name}, {date} doğdu.",
        (ReportLang::Tr, "birth_place") => "{name}, {place_loc} doğdu.",
        (ReportLang::Tr, "parents") => "{father} ile {mother} çiftinin çocuğudur.",
        (ReportLang::Tr, "parent_one") => "{father} adlı kişinin çocuğudur.",
        (ReportLang::Tr, "marriage_full") => "{spouse} ile {date} {place_loc} evlendi.",
        (ReportLang::Tr, "marriage_date") => "{spouse} ile {date} evlendi.",
        (ReportLang::Tr, "marriage_place") => "{spouse} ile {place_loc} evlendi.",
        (ReportLang::Tr, "marriage") => "{spouse} ile evlendi.",
        (ReportLang::Tr, "partner") => "{spouse} ile birlikte yaşadı.",
        (ReportLang::Tr, "children_one") => "Bir çocukları oldu: {children}.",
        (ReportLang::Tr, "children") => "{n} çocukları oldu: {children}.",
        (ReportLang::Tr, "occupation") => "Mesleği: {value}.",
        (ReportLang::Tr, "occupation_date") => "{date} {value} olarak çalıştı.",
        (ReportLang::Tr, "residence") => "{place_loc} yaşadı{date}.",
        (ReportLang::Tr, "death_full") => "{date} {place_loc} vefat etti{age}.",
        (ReportLang::Tr, "death_date") => "{date} vefat etti{age}.",
        (ReportLang::Tr, "death_place") => "{place_loc} vefat etti.",
        (ReportLang::Tr, "burial") => "{place_loc} defnedildi.",
        _ => "",
    }
}

fn render(tpl: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::new();
    let mut rest = tpl;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('}') else {
            out.push_str(&rest[i..]);
            return out;
        };
        let key = &rest[i + 1..i + j];
        out.push_str(vars.get(key).map(String::as_str).unwrap_or(""));
        rest = &rest[i + j + 1..];
    }
    out.push_str(rest);
    // tidy spaces left by empty placeholders
    let mut s = out.split_whitespace().collect::<Vec<_>>().join(" ");
    s = s.replace(" ,", ",").replace(" .", ".").replace(" :", ":");
    capitalise(&s)
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Turkish locative suffix with vowel harmony and consonant assimilation: Konya → Konya'da, İzmir → İzmir'de, Kars → Kars'ta.
pub fn locative_tr(word: &str) -> String {
    let w = word.trim();
    if w.is_empty() {
        return String::new();
    }
    let lower: Vec<char> = w
        .chars()
        .flat_map(|c| match c {
            'I' => vec!['ı'],
            'İ' => vec!['i'],
            c => c.to_lowercase().collect(),
        })
        .collect();
    let last_vowel = lower
        .iter()
        .rev()
        .find(|c| "aeıioöuü".contains(**c))
        .copied()
        .unwrap_or('e');
    let back = "aıou".contains(last_vowel);
    let last = *lower.last().unwrap();
    let voiceless = "çfhkpsşt".contains(last);
    let ends_vowel = "aeıioöuü".contains(last);
    let suffix = match (back, voiceless) {
        (true, true) => "ta",
        (true, false) => "da",
        (false, true) => "te",
        (false, false) => "de",
    };
    let _ = ends_vowel;
    format!("{}'{}", w, suffix)
}

// ---------------- phrases ----------------

fn date_phrase(d: &GenDate, lang: ReportLang) -> String {
    let loc = if lang == ReportLang::Tr {
        Locale::Tr
    } else {
        Locale::En
    };
    let plain = |p: &crate::date::Part| {
        let mut one = d.clone();
        one.qualifier = Qualifier::Exact;
        one.a = Some(*p);
        one.b = None;
        one.format_long(loc)
    };
    let Some(a) = d.a.as_ref() else {
        return d.phrase.clone().unwrap_or_default();
    };
    let day = a.day.is_some();
    let ca = plain(a);
    let cb = d.b.as_ref().map(plain).unwrap_or_default();
    let cal = if d.calendar == Calendar::Gregorian {
        ""
    } else {
        " (calendar-specific date)"
    };
    let s = match (lang, d.qualifier) {
        (ReportLang::En, Qualifier::Exact) => format!("{} {}", if day { "on" } else { "in" }, ca),
        (ReportLang::En, Qualifier::About | Qualifier::Estimated | Qualifier::Calculated) => {
            format!("around {}", ca)
        }
        (ReportLang::En, Qualifier::Before) => format!("before {}", ca),
        (ReportLang::En, Qualifier::After) => format!("after {}", ca),
        (ReportLang::En, Qualifier::Between) => format!("between {} and {}", ca, cb),
        (ReportLang::En, Qualifier::From) => format!("from {}", ca),
        (ReportLang::En, Qualifier::To) => format!("until {}", ca),
        (ReportLang::En, Qualifier::FromTo) => format!("from {} to {}", ca, cb),
        (ReportLang::Tr, Qualifier::Exact) => {
            format!("{} {}", ca, if day { "tarihinde" } else { "yılında" })
        }
        (ReportLang::Tr, Qualifier::About | Qualifier::Estimated | Qualifier::Calculated) => {
            format!("yaklaşık {} yılında", ca)
        }
        (ReportLang::Tr, Qualifier::Before) => format!("{} öncesinde", ca),
        (ReportLang::Tr, Qualifier::After) => format!("{} sonrasında", ca),
        (ReportLang::Tr, Qualifier::Between | Qualifier::FromTo) => {
            format!("{} ile {} arasında", ca, cb)
        }
        (ReportLang::Tr, Qualifier::From) => format!("{} tarihinden itibaren", ca),
        (ReportLang::Tr, Qualifier::To) => format!("{} tarihine kadar", ca),
    };
    format!("{}{}", s, cal)
}

// ---------------- builder ----------------

struct Cite {
    author: String,
    title: String,
    publication: String,
    page: String,
}

struct Builder<'a> {
    f: Facts,
    o: &'a Options,
    living: HashSet<String>,
    cites: HashMap<String, Vec<Cite>>,
    footnotes: Vec<String>,
    foot_index: HashMap<String, usize>,
    index: Vec<(String, String)>,
    births: HashMap<String, i64>,
}

fn anchor(id: &str) -> String {
    format!("p-{}", id)
}

impl<'a> Builder<'a> {
    fn new(store: &Store, o: &'a Options) -> Result<Builder<'a>> {
        let f = Facts::load(store)?;
        let living = if o.privacy == Privacy::Off {
            HashSet::new()
        } else {
            living_ids(store, o.living_years, o.current_year)?
        };
        let sources: HashMap<String, (String, String, String)> = store
            .rows("source")?
            .into_iter()
            .map(|r| {
                let g = |k: &str| r[k].as_str().unwrap_or("").to_string();
                (g("id"), (g("author"), g("title"), g("publication")))
            })
            .collect();
        let mut cites: HashMap<String, Vec<Cite>> = HashMap::new();
        for c in store.rows("citation")? {
            let key = format!(
                "{}:{}",
                c["target_type"].as_str().unwrap_or(""),
                c["target_id"].as_str().unwrap_or("")
            );
            if let Some((a, t, p)) = sources.get(c["source_id"].as_str().unwrap_or("")) {
                cites.entry(key).or_default().push(Cite {
                    author: a.clone(),
                    title: t.clone(),
                    publication: p.clone(),
                    page: c["page"].as_str().unwrap_or("").to_string(),
                });
            }
        }
        Ok(Builder {
            f,
            o,
            living,
            cites,
            footnotes: vec![],
            foot_index: HashMap::new(),
            index: vec![],
            births: birth_keys(store)?,
        })
    }

    fn hidden(&self, id: &str) -> bool {
        self.o.privacy == Privacy::Exclude && self.living.contains(id)
    }
    fn masked(&self, id: &str) -> bool {
        self.o.privacy != Privacy::Off && self.living.contains(id)
    }
    fn name(&self, id: &str) -> String {
        if self.masked(id) {
            return if self.o.lang == ReportLang::Tr {
                "Yaşayan kişi".into()
            } else {
                "Living".into()
            };
        }
        let n = self.f.display_name(id);
        if n.is_empty() {
            if self.o.lang == ReportLang::Tr {
                "(adsız)"
            } else {
                "(unnamed)"
            }
            .into()
        } else {
            n
        }
    }
    fn tpl(&self, key: &str) -> String {
        let l = if self.o.lang == ReportLang::Tr {
            "tr"
        } else {
            "en"
        };
        self.o
            .templates
            .get(&format!("{}.{}", l, key))
            .or_else(|| self.o.templates.get(key))
            .cloned()
            .unwrap_or_else(|| default_template(self.o.lang, key).to_string())
    }
    fn say(&self, key: &str, vars: &HashMap<&str, String>) -> String {
        render(&self.tpl(key), vars)
    }
    fn life(&self, id: &str) -> String {
        if self.masked(id) {
            return String::new();
        }
        let y = |e: Option<&EventFact>| {
            e.and_then(|e| e.start)
                .map(|k| crate::date::jdn_to_gregorian(k).0.to_string())
        };
        match (y(self.f.birth(id)), y(self.f.death(id))) {
            (Some(b), Some(d)) => format!("{}–{}", b, d),
            (Some(b), None) => format!("{}–", b),
            (None, Some(d)) => format!("–{}", d),
            _ => String::new(),
        }
    }
    fn pronouns(&self, id: &str) -> (String, String, String, String) {
        // (He, he, his, They)
        match (
            self.o.lang,
            self.f
                .persons
                .get(id)
                .map(|p| p.sex)
                .unwrap_or(Sex::Unknown),
        ) {
            (ReportLang::En, Sex::Male) => ("He".into(), "he".into(), "his".into(), "They".into()),
            (ReportLang::En, Sex::Female) => {
                ("She".into(), "she".into(), "her".into(), "They".into())
            }
            (ReportLang::En, _) => ("They".into(), "they".into(), "their".into(), "They".into()),
            (ReportLang::Tr, _) => ("O".into(), "o".into(), "onun".into(), "Onların".into()),
        }
    }
    fn footnote(&mut self, target: &str) -> String {
        let Some(cs) = self.cites.get(target) else {
            return String::new();
        };
        let mut marks = vec![];
        let items: Vec<String> = cs
            .iter()
            .map(|c| {
                let mut s = String::new();
                if !c.author.is_empty() {
                    s.push_str(&format!("{}, ", c.author));
                }
                s.push_str(&format!("*{}*", c.title));
                if !c.publication.is_empty() {
                    s.push_str(&format!(" ({})", c.publication));
                }
                if !c.page.is_empty() {
                    s.push_str(&format!(", {}", c.page));
                }
                s.push('.');
                s
            })
            .collect();
        for text in items {
            let n = match self.foot_index.get(&text) {
                Some(n) => *n,
                None => {
                    self.footnotes.push(text.clone());
                    self.foot_index.insert(text, self.footnotes.len());
                    self.footnotes.len()
                }
            };
            marks.push(format!("[{}]", n));
        }
        marks.join("")
    }
    fn date_of(&self, e: &EventFact) -> Option<String> {
        e.date
            .as_ref()
            .map(|d| date_phrase(d, self.o.lang))
            .filter(|s| !s.is_empty())
    }
    fn place_of(&self, e: &EventFact) -> Option<String> {
        self.f.place_name(e).cloned()
    }

    fn vars(&self, id: &str) -> HashMap<&'static str, String> {
        let (he_c, he, his, they) = self.pronouns(id);
        let mut v = HashMap::new();
        v.insert("name", self.name(id));
        v.insert("He", he_c);
        v.insert("he", he);
        v.insert("his", his);
        v.insert("They", they);
        v
    }

    fn with_event<'b>(
        &self,
        mut v: HashMap<&'b str, String>,
        e: &EventFact,
    ) -> HashMap<&'b str, String> {
        if let Some(d) = self.date_of(e) {
            v.insert("date", d);
        }
        if let Some(p) = self.place_of(e) {
            let last = p.split(',').next_back().unwrap_or(&p).trim().to_string();
            let head: Vec<&str> = p.split(',').map(str::trim).collect();
            let loc = if self.o.lang == ReportLang::Tr {
                let mut parts: Vec<String> = head.iter().map(|s| s.to_string()).collect();
                if let Some(l) = parts.last_mut() {
                    *l = locative_tr(&last);
                }
                parts.join(", ")
            } else {
                p.clone()
            };
            v.insert("place", p);
            v.insert("place_loc", loc);
        }
        if let Some(val) = &e.value {
            if val != "Y" {
                v.insert("value", val.clone());
            }
        }
        v
    }

    /// Narrative paragraphs for one person.
    fn narrative(&mut self, id: &str) -> Vec<String> {
        if self.masked(id) {
            return vec![];
        }
        let mut paras: Vec<String> = vec![];
        let mut first: Vec<String> = vec![];
        let birth = self
            .f
            .person_events(id)
            .find(|e| e.kind == "BIRT")
            .or_else(|| {
                self.f
                    .person_events(id)
                    .find(|e| matches!(e.kind.as_str(), "CHR" | "BAPM"))
            })
            .cloned();
        let base = self.vars(id);
        if let Some(b) = &birth {
            let v = self.with_event(base.clone(), b);
            let key = match (v.contains_key("date"), v.contains_key("place")) {
                (true, true) => "birth_full",
                (true, false) => "birth_date",
                (false, true) => "birth_place",
                _ => "",
            };
            if !key.is_empty() {
                let mut s = self.say(key, &v);
                s.push_str(&self.footnote(&format!("event:{}", b.id)));
                first.push(s);
            }
        }
        // parents
        let parents: Vec<String> = self.f.graph.people[id]
            .parents
            .iter()
            .map(|p| p.0.clone())
            .fold(vec![], |mut a, p| {
                if !a.contains(&p) {
                    a.push(p);
                }
                a
            });
        let pn: Vec<String> = parents
            .iter()
            .filter(|p| !self.hidden(p))
            .map(|p| self.name(p))
            .collect();
        if !pn.is_empty() {
            let mut v = base.clone();
            v.insert("father", pn[0].clone());
            if pn.len() > 1 {
                v.insert("mother", pn[1].clone());
            }
            first.push(self.say(
                if pn.len() > 1 {
                    "parents"
                } else {
                    "parent_one"
                },
                &v,
            ));
        }
        if !first.is_empty() {
            paras.push(first.join(" "));
        }
        // occupations & residences
        let mut life: Vec<String> = vec![];
        let evs: Vec<EventFact> = self.f.person_events(id).cloned().collect();
        for e in evs.iter().filter(|e| e.kind == "OCCU") {
            let v = self.with_event(base.clone(), e);
            if v.contains_key("value") {
                let key = if v.contains_key("date") {
                    "occupation_date"
                } else {
                    "occupation"
                };
                let mut s = self.say(key, &v);
                s.push_str(&self.footnote(&format!("event:{}", e.id)));
                life.push(s);
            }
        }
        for e in evs.iter().filter(|e| e.kind == "RESI") {
            let mut v = self.with_event(base.clone(), e);
            if v.contains_key("place") {
                if let Some(d) = v.get("date").cloned() {
                    v.insert("date", format!(" {}", d));
                }
                life.push(self.say("residence", &v));
            }
        }
        if !life.is_empty() {
            paras.push(life.join(" "));
        }
        // families
        let fams: Vec<crate::facts::FamilyFacts> = self
            .f
            .families
            .iter()
            .filter(|f| f.partners.iter().any(|p| p == id))
            .cloned()
            .collect();
        for fam in fams {
            let mut sents: Vec<String> = vec![];
            let spouse = fam.partners.iter().find(|p| p.as_str() != id).cloned();
            let marr = fam
                .events
                .iter()
                .map(|&i| self.f.events[i].clone())
                .find(|e| e.kind == "MARR");
            if let Some(sp) = spouse.as_ref().filter(|s| !self.hidden(s)) {
                let mut v = base.clone();
                v.insert("spouse", self.name(sp));
                match &marr {
                    Some(m) => {
                        let v = self.with_event(v, m);
                        let key = match (v.contains_key("date"), v.contains_key("place")) {
                            (true, true) => "marriage_full",
                            (true, false) => "marriage_date",
                            (false, true) => "marriage_place",
                            _ => "marriage",
                        };
                        let mut s = self.say(key, &v);
                        s.push_str(&self.footnote(&format!("event:{}", m.id)));
                        sents.push(s);
                    }
                    None => sents.push(self.say("partner", &v)),
                }
            }
            let mut kids: Vec<&String> = fam.children.iter().filter(|c| !self.hidden(c)).collect();
            kids.sort_by_key(|c| self.births.get(*c).copied().unwrap_or(i64::MAX));
            if !kids.is_empty() {
                let list: Vec<String> = kids
                    .iter()
                    .map(|c| {
                        let l = self.life(c);
                        let y = l.split('–').next().unwrap_or("").to_string();
                        // children who share the surname are listed by given name only
                        let same = !self.masked(c)
                            && !self.f.persons[id].surname.is_empty()
                            && crate::name::fold(&self.f.persons[c.as_str()].surname)
                                == crate::name::fold(&self.f.persons[id].surname)
                            && !self.f.persons[c.as_str()].given.is_empty();
                        let shown = if same {
                            self.f.persons[c.as_str()].given.clone()
                        } else {
                            self.name(c)
                        };
                        if y.is_empty() {
                            shown
                        } else {
                            format!("{} ({})", shown, y)
                        }
                    })
                    .collect();
                let mut v = base.clone();
                v.insert("n", kids.len().to_string());
                v.insert("children", list.join(", "));
                sents.push(self.say(
                    if kids.len() == 1 {
                        "children_one"
                    } else {
                        "children"
                    },
                    &v,
                ));
            }
            if !sents.is_empty() {
                paras.push(sents.join(" "));
            }
        }
        // death & burial
        let mut end: Vec<String> = vec![];
        if let Some(d) = evs.iter().find(|e| e.kind == "DEAT") {
            let mut v = self.with_event(base.clone(), d);
            if let (Some(b), Some(dd)) = (birth.as_ref().and_then(|b| b.start), d.start) {
                if birth.as_ref().map(|b| b.start == b.end).unwrap_or(false) && d.start == d.end {
                    let age = ((dd - b) as f64 / 365.2425).floor() as i64;
                    v.insert(
                        "age",
                        if self.o.lang == ReportLang::Tr {
                            format!(" ({} yaşında)", age)
                        } else {
                            format!(" (aged {})", age)
                        },
                    );
                }
            }
            let key = match (v.contains_key("date"), v.contains_key("place")) {
                (true, true) => "death_full",
                (true, false) => "death_date",
                (false, true) => "death_place",
                _ => "",
            };
            if !key.is_empty() {
                let mut s = self.say(key, &v);
                s.push_str(&self.footnote(&format!("event:{}", d.id)));
                end.push(s);
            }
        }
        if let Some(b) = evs.iter().find(|e| e.kind == "BURI") {
            let v = self.with_event(base.clone(), b);
            if v.contains_key("place") {
                end.push(self.say("burial", &v));
            }
        }
        if !end.is_empty() {
            paras.push(end.join(" "));
        }
        let pc = self.footnote(&format!("person:{}", id));
        if !pc.is_empty() {
            paras.push(format!(
                "{} {}",
                if self.o.lang == ReportLang::Tr {
                    "Kaynaklar:"
                } else {
                    "Sources:"
                },
                pc
            ));
        }
        paras
    }

    fn heading_for(&mut self, id: &str, level: u8, prefix: &str) -> Block {
        let name = self.name(id);
        if !self.masked(id) {
            self.index.push((
                self.f.persons[id].surname.clone() + ", " + &self.f.persons[id].given,
                anchor(id),
            ));
        }
        let life = self.life(id);
        Block::Heading {
            level,
            text: format!(
                "{}{}{}",
                prefix,
                name,
                if life.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", life)
                }
            ),
            anchor: Some(anchor(id)),
        }
    }

    fn finish(mut self, title: String, blocks: Vec<Block>) -> Document {
        self.index.sort_by_key(|a| crate::name::fold(&a.0));
        self.index.dedup();
        Document {
            title,
            lang: if self.o.lang == ReportLang::Tr {
                "tr".into()
            } else {
                "en".into()
            },
            blocks,
            index: self.index,
            footnotes: self.footnotes,
        }
    }
}

fn tr(o: &Options, en: &str, trk: &str) -> String {
    if o.lang == ReportLang::Tr { trk } else { en }.to_string()
}

pub fn individual_summary(store: &Store, id: &str, o: &Options) -> Result<Document> {
    let mut b = Builder::new(store, o)?;
    if !b.f.persons.contains_key(id) || b.hidden(id) {
        return Ok(unavailable(o));
    }
    let mut blocks = vec![b.heading_for(id, 1, "")];
    for p in b.narrative(id) {
        blocks.push(Block::Para(p));
    }
    if !b.masked(id) {
        let rows: Vec<Vec<String>> =
            b.f.person_events(id)
                .map(|e| {
                    vec![
                        e.custom.clone().unwrap_or_else(|| e.kind.clone()),
                        e.date
                            .as_ref()
                            .map(|d| {
                                d.format(if o.lang == ReportLang::Tr {
                                    Locale::Tr
                                } else {
                                    Locale::En
                                })
                            })
                            .unwrap_or_default(),
                        b.f.place_name(e).cloned().unwrap_or_default(),
                        e.value.clone().unwrap_or_default(),
                    ]
                })
                .collect();
        if !rows.is_empty() {
            blocks.push(Block::Heading {
                level: 2,
                text: tr(o, "Events and facts", "Olaylar ve olgular"),
                anchor: None,
            });
            blocks.push(Block::Table {
                header: vec![
                    tr(o, "Event", "Olay"),
                    tr(o, "Date", "Tarih"),
                    tr(o, "Place", "Yer"),
                    tr(o, "Details", "Ayrıntı"),
                ],
                rows,
            });
        }
    }
    let title = b.name(id);
    Ok(b.finish(title, blocks))
}

fn ancestors_into(b: &mut Builder, root: &str, blocks: &mut Vec<Block>) {
    let o = b.o;
    let rows = ahnentafel(&b.f.graph, root, o.generations.saturating_sub(1));
    let mut seen: HashMap<String, u64> = HashMap::new();
    let mut gen = u32::MAX;
    for (n, id) in rows {
        if b.hidden(&id) {
            continue;
        }
        let g = 63 - n.leading_zeros();
        if g != gen {
            gen = g;
            blocks.push(Block::Heading {
                level: 2,
                text: format!("{} {}", tr(o, "Generation", "Kuşak"), g + 1),
                anchor: None,
            });
        }
        if let Some(first) = seen.get(&id) {
            blocks.push(Block::Para(format!(
                "{}. {} — {} {}.",
                n,
                b.name(&id),
                tr(o, "same person as", "ile aynı kişi:"),
                first
            )));
            continue;
        }
        seen.insert(id.clone(), n);
        let h = b.heading_for(&id, 3, &format!("{}. ", n));
        blocks.push(h);
        for p in b.narrative(&id) {
            blocks.push(Block::Para(p));
        }
    }
}

fn descendants_into(b: &mut Builder, root: &str, blocks: &mut Vec<Block>) {
    let o = b.o;
    let nums = number_descendants(
        &b.f.graph,
        &b.births,
        root,
        o.generations.saturating_sub(1),
        DescendantNumbering::DAboville,
    );
    let mut gen = usize::MAX;
    for (n, id) in nums {
        if b.hidden(&id) {
            continue;
        }
        let g = n.matches('.').count();
        if g != gen {
            gen = g;
            blocks.push(Block::Heading {
                level: 2,
                text: format!("{} {}", tr(o, "Generation", "Kuşak"), g + 1),
                anchor: None,
            });
        }
        let h = b.heading_for(&id, 3, &format!("{} ", n));
        blocks.push(h);
        for p in b.narrative(&id) {
            blocks.push(Block::Para(p));
        }
    }
}

fn unavailable(o: &Options) -> Document {
    Document {
        title: tr(o, "Person not available", "Kişi kullanılamıyor"),
        lang: if o.lang == ReportLang::Tr {
            "tr".into()
        } else {
            "en".into()
        },
        ..Default::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Numbering {
    Ahnentafel,
    DAboville,
    Henry,
}

/// Numbered ancestor or descendant list (Ahnentafel, d'Aboville, Henry) as a table; living people follow the privacy mode.
pub fn numbered_report(
    store: &Store,
    root: &str,
    o: &Options,
    system: Numbering,
) -> Result<Document> {
    use crate::relationship::{ahnentafel, number_descendants, DescendantNumbering};
    let b = Builder::new(store, o)?;
    if !b.f.persons.contains_key(root) || b.hidden(root) {
        return Ok(unavailable(o));
    }
    let rows: Vec<(String, String)> = match system {
        Numbering::Ahnentafel => ahnentafel(&b.f.graph, root, o.generations.saturating_sub(1))
            .into_iter()
            .map(|(n, id)| (n.to_string(), id))
            .collect(),
        Numbering::DAboville => number_descendants(
            &b.f.graph,
            &b.births,
            root,
            o.generations,
            DescendantNumbering::DAboville,
        ),
        Numbering::Henry => number_descendants(
            &b.f.graph,
            &b.births,
            root,
            o.generations,
            DescendantNumbering::Henry,
        ),
    };
    let header = vec![
        tr(o, "No.", "No.").to_string(),
        tr(o, "Name", "Ad").to_string(),
        tr(o, "Born", "Doğum").to_string(),
        tr(o, "Died", "Ölüm").to_string(),
    ];
    let year = |e: Option<&EventFact>| {
        e.and_then(|e| e.start)
            .map(|k| crate::date::jdn_to_gregorian(k).0.to_string())
            .unwrap_or_default()
    };
    let mut table = vec![];
    for (n, id) in rows {
        if b.hidden(&id) {
            continue;
        }
        let masked = b.masked(&id);
        table.push(vec![
            n,
            b.name(&id),
            if masked {
                String::new()
            } else {
                year(b.f.birth(&id))
            },
            if masked {
                String::new()
            } else {
                year(b.f.death(&id))
            },
        ]);
    }
    let (en, trk) = match system {
        Numbering::Ahnentafel => ("Ahnentafel of", "Ahnentafel:"),
        Numbering::DAboville => ("d'Aboville descendants of", "d'Aboville soyağacı:"),
        Numbering::Henry => ("Henry descendants of", "Henry soyağacı:"),
    };
    let title = format!("{} {}", tr(o, en, trk), b.name(root));
    Ok(b.finish(
        title,
        vec![Block::Table {
            header,
            rows: table,
        }],
    ))
}

pub fn ancestor_report(store: &Store, root: &str, o: &Options) -> Result<Document> {
    let mut b = Builder::new(store, o)?;
    if !b.f.persons.contains_key(root) || b.hidden(root) {
        return Ok(unavailable(o));
    }
    let mut blocks = vec![];
    ancestors_into(&mut b, root, &mut blocks);
    let title = format!("{} {}", tr(o, "Ancestors of", "Atalar:"), b.name(root));
    Ok(b.finish(title, blocks))
}

pub fn descendant_report(store: &Store, root: &str, o: &Options) -> Result<Document> {
    let mut b = Builder::new(store, o)?;
    if !b.f.persons.contains_key(root) || b.hidden(root) {
        return Ok(unavailable(o));
    }
    let mut blocks = vec![];
    descendants_into(&mut b, root, &mut blocks);
    let title = format!("{} {}", tr(o, "Descendants of", "Torunlar:"), b.name(root));
    Ok(b.finish(title, blocks))
}

pub fn family_group_sheet(store: &Store, family_id: &str, o: &Options) -> Result<Document> {
    let mut b = Builder::new(store, o)?;
    let Some(fam) = b.f.families.iter().find(|f| f.id == family_id).cloned() else {
        return Ok(Document {
            title: tr(o, "Family not found", "Aile bulunamadı"),
            lang: "en".into(),
            ..Default::default()
        });
    };
    let loc = if o.lang == ReportLang::Tr {
        Locale::Tr
    } else {
        Locale::En
    };
    let fact = |b: &Builder, id: &str, kinds: &[&str]| -> (String, String) {
        if b.masked(id) {
            return (String::new(), String::new());
        }
        for k in kinds {
            if let Some(e) = b.f.person_events(id).find(|e| e.kind == *k) {
                return (
                    e.date.as_ref().map(|d| d.format(loc)).unwrap_or_default(),
                    b.f.place_name(e).cloned().unwrap_or_default(),
                );
            }
        }
        (String::new(), String::new())
    };
    let mut blocks = vec![];
    let names: Vec<String> = fam
        .partners
        .iter()
        .filter(|p| !b.hidden(p))
        .map(|p| b.name(p))
        .collect();
    let title = format!(
        "{}: {}",
        tr(o, "Family group sheet", "Aile grubu çizelgesi"),
        names.join(" & ")
    );
    let shown: Vec<String> = fam
        .partners
        .iter()
        .filter(|p| !b.hidden(p))
        .cloned()
        .collect();
    for p in &shown {
        blocks.push(b.heading_for(p, 2, ""));
        let (bd, bp) = fact(&b, p, &["BIRT", "CHR"]);
        let (dd, dp) = fact(&b, p, &["DEAT", "BURI"]);
        blocks.push(Block::Table {
            header: vec![
                tr(o, "Fact", "Olgu"),
                tr(o, "Date", "Tarih"),
                tr(o, "Place", "Yer"),
            ],
            rows: vec![
                vec![tr(o, "Born", "Doğum"), bd, bp],
                vec![tr(o, "Died", "Ölüm"), dd, dp],
            ],
        });
    }
    if let Some(m) = fam
        .events
        .iter()
        .map(|&i| &b.f.events[i])
        .find(|e| e.kind == "MARR")
    {
        blocks.push(Block::Heading {
            level: 2,
            text: tr(o, "Marriage", "Evlilik"),
            anchor: None,
        });
        blocks.push(Block::Para(
            format!(
                "{} {}",
                m.date.as_ref().map(|d| d.format(loc)).unwrap_or_default(),
                b.f.place_name(m).cloned().unwrap_or_default()
            )
            .trim()
            .to_string(),
        ));
    }
    blocks.push(Block::Heading {
        level: 2,
        text: tr(o, "Children", "Çocuklar"),
        anchor: None,
    });
    let mut rows = vec![];
    let mut kids = fam.children.clone();
    kids.sort_by_key(|c| b.births.get(c).copied().unwrap_or(i64::MAX));
    let kids: Vec<String> = kids.into_iter().filter(|c| !b.hidden(c)).collect();
    for c in &kids {
        let (bd, bp) = fact(&b, c, &["BIRT", "CHR"]);
        let (dd, _) = fact(&b, c, &["DEAT", "BURI"]);
        rows.push(vec![b.name(c), bd, bp, dd]);
        b.index.push((
            b.f.persons[c.as_str()].surname.clone() + ", " + &b.f.persons[c.as_str()].given,
            anchor(c),
        ));
    }
    blocks.push(Block::Table {
        header: vec![
            tr(o, "Name", "Ad"),
            tr(o, "Born", "Doğum"),
            tr(o, "Place", "Yer"),
            tr(o, "Died", "Ölüm"),
        ],
        rows,
    });
    Ok(b.finish(title, blocks))
}

/// Complete family book: ancestors and descendants of one person with continuous footnotes and a single name index.
pub fn book(store: &Store, root: &str, o: &Options) -> Result<Document> {
    let mut b = Builder::new(store, o)?;
    if !b.f.persons.contains_key(root) || b.hidden(root) {
        return Ok(unavailable(o));
    }
    let name = b.name(root);
    let mut blocks = vec![Block::Heading {
        level: 2,
        text: format!("{} {}", tr(o, "Ancestors of", "Atalar:"), name),
        anchor: None,
    }];
    ancestors_into(&mut b, root, &mut blocks);
    blocks.push(Block::Heading {
        level: 2,
        text: format!("{} {}", tr(o, "Descendants of", "Torunlar:"), name),
        anchor: None,
    });
    descendants_into(&mut b, root, &mut blocks);
    let title = format!("{}: {}", tr(o, "Family book", "Aile kitabı"), name);
    Ok(b.finish(title, blocks))
}

/// Alphabetical bibliography of every non-inline source, Chicago-style, with how many citations use each.
pub fn bibliography(store: &Store, o: &Options) -> Result<Document> {
    let mut uses: HashMap<String, usize> = HashMap::new();
    for c in store.rows("citation")? {
        *uses
            .entry(c["source_id"].as_str().unwrap_or("").to_string())
            .or_default() += 1;
    }
    let repos: HashMap<String, String> = store
        .rows("repository")?
        .into_iter()
        .map(|r| {
            (
                r["id"].as_str().unwrap_or("").to_string(),
                r["name"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    let mut items: Vec<(String, String)> = vec![];
    for r in store.rows("source")? {
        let g = |k: &str| r[k].as_str().unwrap_or("").trim().to_string();
        let id = g("id");
        let mut line = String::new();
        if !g("author").is_empty() {
            line.push_str(&format!("{}. ", g("author").trim_end_matches('.')));
        }
        line.push_str(&format!("*{}*", g("title")));
        if !g("publication").is_empty() {
            line.push_str(&format!(". {}", g("publication").trim_end_matches('.')));
        }
        if let Some(rn) = r["repository_id"]
            .as_str()
            .and_then(|i| repos.get(i))
            .filter(|n| !n.is_empty())
        {
            line.push_str(&format!(". {}: {}", tr(o, "Held by", "Bulunduğu yer"), rn));
        }
        line.push('.');
        let n = uses.get(&id).copied().unwrap_or(0);
        line.push_str(&format!(
            " ({})",
            if o.lang == ReportLang::Tr {
                format!("{} atıf", n)
            } else {
                format!("{} citation{}", n, if n == 1 { "" } else { "s" })
            }
        ));
        let sort_key = crate::name::fold(&format!("{} {}", g("author"), g("title")));
        items.push((sort_key, line));
    }
    items.sort();
    let title = tr(o, "Bibliography", "Kaynakça");
    let blocks = vec![
        Block::Heading {
            level: 2,
            text: title.clone(),
            anchor: None,
        },
        Block::List(items.into_iter().map(|x| x.1).collect()),
    ];
    Ok(Document {
        title,
        lang: if o.lang == ReportLang::Tr {
            "tr".into()
        } else {
            "en".into()
        },
        blocks,
        ..Default::default()
    })
}

// ---------------- renderers ----------------

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Minimal inline markup: `*italic*` becomes `<em>`, footnote markers `[n]` become superscript links.
fn inline_html(s: &str) -> String {
    let e = esc(s);
    let mut out = String::new();
    let mut ital = false;
    let chars: Vec<char> = e.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '*' {
            out.push_str(if ital { "</em>" } else { "<em>" });
            ital = !ital;
        } else if c == '['
            && chars
                .get(i + 1)
                .map(|x| x.is_ascii_digit())
                .unwrap_or(false)
        {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if chars.get(j) == Some(&']') {
                let n: String = chars[i + 1..j].iter().collect();
                out.push_str(&format!("<sup><a href=\"#fn{}\">{}</a></sup>", n, n));
                i = j + 1;
                continue;
            }
            out.push(c);
        } else {
            out.push(c);
        }
        i += 1;
    }
    if ital {
        out.push_str("</em>");
    }
    out
}

pub fn to_html(d: &Document) -> String {
    let mut h = format!(
        "<!doctype html>\n<html lang=\"{}\"><head><meta charset=\"utf-8\"><title>{}</title>\n<style>body{{font-family:Georgia,serif;max-width:46rem;margin:2rem auto;padding:0 1rem;line-height:1.55;color:#222}}h1,h2,h3{{font-family:system-ui,sans-serif}}table{{border-collapse:collapse;width:100%;margin:.5rem 0}}td,th{{border:1px solid #ccc;padding:.25rem .5rem;text-align:left}}.fn{{font-size:.9rem}}a{{color:#2f6f5e}}@media print{{a{{color:inherit;text-decoration:none}}}}</style></head><body><main>\n",
        d.lang,
        esc(&d.title)
    );
    if !d
        .blocks
        .iter()
        .any(|b| matches!(b, Block::Heading { level: 1, .. }))
    {
        h.push_str(&format!("<h1>{}</h1>\n", esc(&d.title)));
    }
    let toc: Vec<(&str, &str)> = d
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Heading { level: 2, text, .. } => Some((text.as_str(), "")),
            _ => None,
        })
        .collect();
    if toc.len() > 1
        && d.blocks
            .iter()
            .filter(|b| matches!(b, Block::Heading { level: 3, .. }))
            .count()
            > 0
    {
        h.push_str(&format!(
            "<nav><h2>{}</h2><ul>",
            if d.lang == "tr" {
                "İçindekiler"
            } else {
                "Contents"
            }
        ));
        for (t, _) in &toc {
            h.push_str(&format!("<li>{}</li>", esc(t)));
        }
        h.push_str("</ul></nav>\n");
    }
    for b in &d.blocks {
        match b {
            Block::Heading {
                level,
                text,
                anchor,
            } => {
                let id = anchor
                    .as_ref()
                    .map(|a| format!(" id=\"{}\"", esc(a)))
                    .unwrap_or_default();
                h.push_str(&format!("<h{l}{id}>{}</h{l}>\n", esc(text), l = level));
            }
            Block::Para(p) => h.push_str(&format!("<p>{}</p>\n", inline_html(p))),
            Block::Image { src, alt, caption } => {
                h.push_str(&format!(
                    "<figure><img src=\"{}\" alt=\"{}\" style=\"max-width:100%\">{}</figure>\n",
                    esc(src),
                    esc(alt),
                    caption
                        .as_ref()
                        .map(|c| format!("<figcaption>{}</figcaption>", esc(c)))
                        .unwrap_or_default()
                ));
            }
            Block::List(items) => {
                h.push_str("<ul>");
                for i in items {
                    h.push_str(&format!("<li>{}</li>", inline_html(i)));
                }
                h.push_str("</ul>\n");
            }
            Block::Table { header, rows } => {
                h.push_str("<table><thead><tr>");
                for c in header {
                    h.push_str(&format!("<th>{}</th>", esc(c)));
                }
                h.push_str("</tr></thead><tbody>");
                for r in rows {
                    h.push_str("<tr>");
                    for c in r {
                        h.push_str(&format!("<td>{}</td>", esc(c)));
                    }
                    h.push_str("</tr>");
                }
                h.push_str("</tbody></table>\n");
            }
        }
    }
    if !d.footnotes.is_empty() {
        h.push_str(&format!(
            "<h2>{}</h2><ol class=\"fn\">",
            if d.lang == "tr" {
                "Dipnotlar"
            } else {
                "Sources"
            }
        ));
        for (i, f) in d.footnotes.iter().enumerate() {
            h.push_str(&format!("<li id=\"fn{}\">{}</li>", i + 1, inline_html(f)));
        }
        h.push_str("</ol>\n");
    }
    if !d.index.is_empty() {
        h.push_str(&format!(
            "<h2>{}</h2><ul class=\"index\">",
            if d.lang == "tr" {
                "Ad dizini"
            } else {
                "Index of names"
            }
        ));
        for (n, a) in &d.index {
            h.push_str(&format!("<li><a href=\"#{}\">{}</a></li>", esc(a), esc(n)));
        }
        h.push_str("</ul>\n");
    }
    h.push_str("</main></body></html>\n");
    h
}

pub fn to_markdown(d: &Document) -> String {
    let mut m = format!("# {}\n\n", d.title);
    for b in &d.blocks {
        match b {
            Block::Heading { level, text, .. } => m.push_str(&format!(
                "{} {}\n\n",
                "#".repeat((*level as usize + 1).min(6)),
                text
            )),
            Block::Para(p) => m.push_str(&format!("{}\n\n", p)),
            Block::Image { alt, caption, .. } => {
                m.push_str(&format!("*[{}]*\n\n", caption.as_ref().unwrap_or(alt)))
            }
            Block::List(items) => {
                for i in items {
                    m.push_str(&format!("- {}\n", i));
                }
                m.push('\n');
            }
            Block::Table { header, rows } => {
                m.push_str(&format!(
                    "| {} |\n|{}|\n",
                    header.join(" | "),
                    header.iter().map(|_| "---").collect::<Vec<_>>().join("|")
                ));
                for r in rows {
                    m.push_str(&format!(
                        "| {} |\n",
                        r.iter()
                            .map(|c| c.replace('|', "\\|"))
                            .collect::<Vec<_>>()
                            .join(" | ")
                    ));
                }
                m.push('\n');
            }
        }
    }
    if !d.footnotes.is_empty() {
        m.push_str(&format!(
            "## {}\n\n",
            if d.lang == "tr" {
                "Dipnotlar"
            } else {
                "Sources"
            }
        ));
        for (i, f) in d.footnotes.iter().enumerate() {
            m.push_str(&format!("{}. {}\n", i + 1, f));
        }
        m.push('\n');
    }
    if !d.index.is_empty() {
        m.push_str(&format!(
            "## {}\n\n",
            if d.lang == "tr" {
                "Ad dizini"
            } else {
                "Index of names"
            }
        ));
        for (n, _) in &d.index {
            m.push_str(&format!("- {}\n", n));
        }
    }
    m
}
