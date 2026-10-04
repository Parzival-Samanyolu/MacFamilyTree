//! Stories: ordered blocks (headings, text, person write-ups, pictures, timelines, quotes) rendered to HTML/Markdown.

use crate::date::Locale;
use crate::media;
use crate::model::living_ids;
use crate::report::{self, Block, Document, Options, Privacy, ReportLang};
use crate::store::{new_id, Result, Row, Store, StoreError};
use crate::timeline::{self, Scope};
use base64_lite::encode as b64;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StoryBlock {
    Heading { text: String },
    Text { text: String },
    Quote { text: String, by: Option<String> },
    Person { id: String },
    Media { id: String, caption: Option<String> },
    Timeline { id: String },
}

/// Minimal standard Base64 (the core has no other need for a codec crate).
mod base64_lite {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    pub fn encode(data: &[u8]) -> String {
        let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
        for c in data.chunks(3) {
            let n = (c[0] as u32) << 16
                | (*c.get(1).unwrap_or(&0) as u32) << 8
                | *c.get(2).unwrap_or(&0) as u32;
            out.push(T[(n >> 18) as usize & 63] as char);
            out.push(T[(n >> 12) as usize & 63] as char);
            out.push(if c.len() > 1 {
                T[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            out.push(if c.len() > 2 {
                T[n as usize & 63] as char
            } else {
                '='
            });
        }
        out
    }
}

pub fn parse_blocks(json: &str) -> Vec<StoryBlock> {
    serde_json::from_str(json).unwrap_or_default()
}

pub fn list(store: &Store) -> Result<Vec<Value>> {
    let mut rows = store.rows("story")?;
    rows.sort_by_key(|r| {
        (
            r["sort_order"].as_i64().unwrap_or(0),
            r["created"].as_i64().unwrap_or(0),
        )
    });
    Ok(rows
        .into_iter()
        .map(|r| {
            let n = parse_blocks(r["blocks"].as_str().unwrap_or("[]")).len();
            json!({"id": r["id"], "title": r["title"], "blocks": n, "modified": r["modified"]})
        })
        .collect())
}

pub fn get(store: &Store, id: &str) -> Result<Option<Value>> {
    Ok(store.rows_where("story", "id", id)?.into_iter().next().map(|r| {
        json!({"id": r["id"], "title": r["title"],
               "blocks": serde_json::to_value(parse_blocks(r["blocks"].as_str().unwrap_or("[]"))).unwrap_or(json!([]))})
    }))
}

/// Create (no `id`) or update a story; returns its id.
pub fn save(
    store: &mut Store,
    id: Option<&str>,
    title: &str,
    blocks: &[StoryBlock],
) -> Result<String> {
    if title.trim().is_empty() {
        return Err(StoryError::EmptyTitle.into());
    }
    let json = serde_json::to_string(blocks)?;
    let existing = match id {
        Some(i) => store.rows_where("story", "id", i)?.into_iter().next(),
        None => None,
    };
    let order = store.count("story")?;
    store.transact(
        if existing.is_some() {
            "Edit story"
        } else {
            "New story"
        },
        |tx| {
            let mut r: Row = existing.unwrap_or_default();
            let sid = r
                .get("id")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(new_id);
            r.insert("id".into(), json!(sid));
            r.insert("title".into(), json!(title.trim()));
            r.insert("blocks".into(), json!(json));
            r.entry("sort_order").or_insert(json!(order));
            tx.put_row("story", r)?;
            Ok(sid)
        },
    )
}

#[derive(Debug)]
enum StoryError {
    EmptyTitle,
}
impl From<StoryError> for StoreError {
    fn from(_: StoryError) -> Self {
        StoreError::Other("a story needs a title".into())
    }
}

pub fn delete(store: &mut Store, id: &str) -> Result<()> {
    store.transact("Delete story", |tx| {
        tx.delete("story", id)?;
        Ok(())
    })
}

const MAX_INLINE_IMAGE: usize = 1_500_000;

pub fn render(store: &Store, id: &str, o: &Options) -> Result<Document> {
    let Some(row) = store.rows_where("story", "id", id)?.into_iter().next() else {
        return Err(StoreError::Other(format!("no story {id}")));
    };
    let title = row["title"].as_str().unwrap_or("").to_string();
    let loc = if o.lang == ReportLang::Tr {
        Locale::Tr
    } else {
        Locale::En
    };
    let living = if o.privacy == Privacy::Off {
        Default::default()
    } else {
        living_ids(store, o.living_years, o.current_year)?
    };
    let mut blocks = vec![Block::Heading {
        level: 1,
        text: title.clone(),
        anchor: None,
    }];
    for b in parse_blocks(row["blocks"].as_str().unwrap_or("[]")) {
        match b {
            StoryBlock::Heading { text } => blocks.push(Block::Heading {
                level: 2,
                text,
                anchor: None,
            }),
            StoryBlock::Text { text } => {
                for p in text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
                    blocks.push(Block::Para(p.replace('\n', " ")));
                }
            }
            StoryBlock::Quote { text, by } => blocks.push(Block::Para(match by {
                Some(b) if !b.trim().is_empty() => format!("“{}” — {}", text.trim(), b.trim()),
                _ => format!("“{}”", text.trim()),
            })),
            StoryBlock::Person { id } => {
                for blk in report::individual_summary(store, &id, o)?.blocks {
                    blocks.push(match blk {
                        Block::Heading {
                            level: 1,
                            text,
                            anchor,
                        } => Block::Heading {
                            level: 3,
                            text,
                            anchor,
                        },
                        other => other,
                    });
                }
            }
            StoryBlock::Timeline { id } => {
                let hidden = o.privacy != Privacy::Off && living.contains(&id);
                if !hidden {
                    let rows: Vec<Vec<String>> =
                        timeline::timeline(store, Scope::Person(&id), false, loc)?
                            .into_iter()
                            .map(|e| vec![e.date_text, e.text])
                            .collect();
                    if !rows.is_empty() {
                        let h = if o.lang == ReportLang::Tr {
                            ["Tarih", "Olay"]
                        } else {
                            ["Date", "Event"]
                        };
                        blocks.push(Block::Table {
                            header: h.iter().map(|s| s.to_string()).collect(),
                            rows,
                        });
                    }
                }
            }
            StoryBlock::Media { id, caption } => {
                // Pictures of living people are left out whenever privacy is on.
                let shown_to_living = media::links(store, &id)?.iter().any(|l| {
                    l["target_type"] == "person"
                        && living.contains(l["target_id"].as_str().unwrap_or(""))
                });
                if shown_to_living {
                    continue;
                }
                let name = store
                    .rows_where("media", "id", &id)?
                    .first()
                    .map(|m| m["path"].as_str().unwrap_or("").to_string())
                    .unwrap_or_default();
                let data = media::file(store, &id)?;
                let src = match data {
                    Some((_, mime, bytes))
                        if mime.starts_with("image/") && bytes.len() <= MAX_INLINE_IMAGE =>
                    {
                        Some(format!("data:{mime};base64,{}", b64(&bytes)))
                    }
                    _ => media::thumb(store, &id)
                        .map(|t| format!("data:image/jpeg;base64,{}", b64(&t))),
                };
                if let Some(src) = src {
                    let cap = caption.filter(|c| !c.trim().is_empty()).or_else(|| {
                        store.rows_where("media", "id", &id).ok().and_then(|r| {
                            r.first()
                                .and_then(|m| m["caption"].as_str().map(String::from))
                        })
                    });
                    blocks.push(Block::Image {
                        src,
                        alt: cap.clone().unwrap_or(name),
                        caption: cap,
                    });
                }
            }
        }
    }
    Ok(Document {
        title,
        lang: if o.lang == ReportLang::Tr { "tr" } else { "en" }.into(),
        blocks,
        index: vec![],
        footnotes: vec![],
    })
}
