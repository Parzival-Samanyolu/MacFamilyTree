//! Static website export: one page per person, a searchable index, no external requests, no scripts beyond local search.

use crate::facts::Facts;
use crate::media;
use crate::model::living_ids;
use crate::name::fold;
use crate::report::{self, Options, Privacy, ReportLang};
use crate::store::{Result, Store};
use std::collections::{HashMap, HashSet};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const CSS: &str = "body{font-family:Georgia,serif;margin:0;color:#222;line-height:1.55}header{background:#2f6f5e;color:#fff;padding:.8rem 1.2rem}header a{color:#fff;text-decoration:none;font-family:system-ui,sans-serif}main{max-width:48rem;margin:1.5rem auto;padding:0 1rem}h1,h2,h3{font-family:system-ui,sans-serif}table{border-collapse:collapse;width:100%}td,th{border:1px solid #ccc;padding:.25rem .5rem;text-align:left}a{color:#1f5a4a}.rel li{margin:.15rem 0}.gallery{display:flex;flex-wrap:wrap;gap:.6rem}.gallery figure{margin:0;width:160px}.gallery img{width:160px;height:160px;object-fit:cover;border-radius:6px}input[type=search]{width:100%;padding:.5rem;font-size:1rem;margin:.5rem 0}footer{text-align:center;color:#666;font-size:.85rem;margin:2rem 0}.sr{position:absolute;left:-9999px}";

const SEARCH_JS: &str = "document.getElementById('q').addEventListener('input',function(e){var v=e.target.value.toLowerCase();document.querySelectorAll('#people li').forEach(function(li){li.hidden=v&&li.dataset.k.indexOf(v)<0})});";

fn page(lang: &str, title: &str, site: &str, root: &str, body: &str, script: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"{lang}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{t}</title><link rel=\"stylesheet\" href=\"{root}style.css\"></head><body><header><a href=\"{root}index.html\">{s}</a></header><main>{body}</main><footer>{f}</footer>{script}</body></html>\n",
        t = esc(title),
        s = esc(site),
        f = if lang == "tr" { "KinTree ile oluşturuldu" } else { "Made with KinTree" },
        script = if script.is_empty() { String::new() } else { format!("<script>{script}</script>") },
    )
}

fn fragment(html: &str) -> &str {
    let a = html.find("<main>").map(|i| i + 6).unwrap_or(0);
    let b = html.rfind("</main>").unwrap_or(html.len());
    &html[a..b]
}

pub fn build(store: &Store, title: &str, o: &Options) -> Result<Vec<(String, Vec<u8>)>> {
    let tr = o.lang == ReportLang::Tr;
    let lang = if tr { "tr" } else { "en" };
    let f = Facts::load(store)?;
    let living = if o.privacy == Privacy::Off {
        HashSet::new()
    } else {
        living_ids(store, o.living_years, o.current_year)?
    };
    let hidden = |id: &str| {
        o.privacy != Privacy::Off
            && (living.contains(id) || f.persons.get(id).map(|p| p.is_private).unwrap_or(false))
    };
    let mut order: Vec<&String> = f
        .order
        .iter()
        .filter(|id| !(o.privacy == Privacy::Exclude && hidden(id)))
        .collect();
    order.sort_by_key(|id| {
        let p = &f.persons[*id];
        (fold(&p.surname), fold(&p.given), (*id).clone())
    });
    let file_of: HashMap<&str, String> = order
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), format!("people/p{}.html", i + 1)))
        .collect();
    let label = |id: &str| -> String {
        if hidden(id) {
            if tr { "Yaşıyor" } else { "Living" }.to_string()
        } else {
            let n = f.display_name(id);
            if n.is_empty() {
                "?".into()
            } else {
                n
            }
        }
    };
    let life = |id: &str| -> String {
        if hidden(id) {
            return String::new();
        }
        let y = |e: Option<&crate::facts::EventFact>| {
            e.and_then(|e| e.start)
                .map(|j| crate::date::jdn_to_gregorian(j).0.to_string())
        };
        match (y(f.birth(id)), y(f.death(id))) {
            (Some(b), Some(d)) => format!("{b}–{d}"),
            (Some(b), None) => format!("{b}–"),
            (None, Some(d)) => format!("–{d}"),
            _ => String::new(),
        }
    };
    let link = |id: &str, from_people: bool| -> String {
        let pre = if from_people { "" } else { "people/" };
        match file_of.get(id) {
            Some(file) if !hidden(id) => format!(
                "<a href=\"{}\">{}</a>",
                esc(&format!("{pre}{}", file.trim_start_matches("people/"))),
                esc(&label(id))
            ),
            _ => esc(&label(id)),
        }
    };
    let mut out: Vec<(String, Vec<u8>)> = vec![("style.css".into(), CSS.as_bytes().to_vec())];

    // media per person
    let mut by_person: HashMap<String, Vec<String>> = HashMap::new();
    for l in store.rows("media_link")? {
        if l["target_type"] == "person" {
            by_person
                .entry(l["target_id"].as_str().unwrap_or("").into())
                .or_default()
                .push(l["media_id"].as_str().unwrap_or("").into());
        }
    }
    let mut media_files: HashSet<String> = HashSet::new();

    for id in &order {
        let id = id.as_str();
        let doc = report::individual_summary(store, id, o)?;
        let mut body = fragment(&report::to_html(&doc)).to_string();
        if !hidden(id) {
            let mut rel = String::new();
            let fams = f.families.iter();
            let (mut parents, mut partners, mut kids) = (vec![], vec![], vec![]);
            for fam in fams {
                if fam.children.iter().any(|c| c == id) {
                    parents.extend(fam.partners.iter().cloned());
                }
                if fam.partners.iter().any(|p| p == id) {
                    partners.extend(fam.partners.iter().filter(|p| p.as_str() != id).cloned());
                    kids.extend(fam.children.iter().cloned());
                }
            }
            for (heading, ids) in [
                (if tr { "Ebeveynler" } else { "Parents" }, parents),
                (if tr { "Eş / partner" } else { "Partners" }, partners),
                (if tr { "Çocuklar" } else { "Children" }, kids),
            ] {
                let ids: Vec<&String> = ids
                    .iter()
                    .filter(|x| o.privacy != Privacy::Exclude || !hidden(x))
                    .collect();
                if !ids.is_empty() {
                    rel.push_str(&format!("<h2>{heading}</h2><ul class=\"rel\">"));
                    for x in ids {
                        rel.push_str(&format!("<li>{}</li>", link(x, true)));
                    }
                    rel.push_str("</ul>");
                }
            }
            body.push_str(&rel);
            let mut gallery = String::new();
            for mid in by_person.get(id).into_iter().flatten() {
                if let Some(t) = media::thumb(store, mid) {
                    let cap = store
                        .rows_where("media", "id", mid)?
                        .first()
                        .and_then(|m| m["caption"].as_str().map(String::from))
                        .unwrap_or_default();
                    let path = format!("media/{mid}.jpg");
                    if media_files.insert(path.clone()) {
                        out.push((path.clone(), t));
                    }
                    gallery.push_str(&format!("<figure><img src=\"../{path}\" alt=\"{c}\" loading=\"lazy\"><figcaption>{c}</figcaption></figure>", c = esc(&cap)));
                }
            }
            if !gallery.is_empty() {
                body.push_str(&format!(
                    "<h2>{}</h2><div class=\"gallery\">{gallery}</div>",
                    if tr { "Ortam" } else { "Media" }
                ));
            }
        }
        out.push((
            file_of[id].clone(),
            page(lang, &label(id), title, "../", &body, "").into_bytes(),
        ));
    }

    let mut idx = format!("<h1>{}</h1><p>{}</p><label class=\"sr\" for=\"q\">{}</label><input id=\"q\" type=\"search\" placeholder=\"{}\"><ul id=\"people\" class=\"rel\">", esc(title), if tr { format!("{} kişi", order.len()) } else { format!("{} people", order.len()) }, if tr { "Ara" } else { "Search" }, if tr { "Ad ara…" } else { "Search names…" });
    for id in &order {
        let id = id.as_str();
        idx.push_str(&format!(
            "<li data-k=\"{}\">{} <small>{}</small></li>",
            esc(&fold(&label(id))),
            link(id, false),
            esc(&life(id))
        ));
    }
    idx.push_str("</ul>");
    out.push((
        "index.html".into(),
        page(lang, title, title, "", &idx, SEARCH_JS).into_bytes(),
    ));
    Ok(out)
}
