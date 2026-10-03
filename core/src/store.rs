//! SQLite project storage with migrations, row-image change tracking, grouped undo/redo and FTS5 search.
//!
//! All writes go through [`Tx::put`] / [`Tx::delete`]. Each call records the before/after *full row image*,
//! so undo/redo is generic for every table and crash-safe (history commits atomically with the data).
//! Foreign keys are intentionally not enforced by SQLite: referential cleanup is explicit (and therefore
//! undoable) in the typed layer.

use rusqlite::{params, types::ValueRef, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

pub type Row = Map<String, Value>;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unknown table: {0}")]
    UnknownTable(String),
    #[error("row for {0} has no string `id`")]
    MissingId(String),
    #[error("database schema v{found} is newer than supported v{supported}")]
    TooNew { found: i64, supported: i64 },
    #[error("{0}")]
    Other(String),
}
pub type Result<T> = std::result::Result<T, StoreError>;

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Tables whose rows are tracked by history. Order is irrelevant.
pub const TABLES: &[&str] = &[
    "person",
    "person_name",
    "family",
    "family_child",
    "event",
    "place",
    "repository",
    "source",
    "citation",
    "media",
    "media_link",
    "note",
    "note_link",
    "task",
    "task_link",
    "association",
    "tag",
    "tag_link",
    "story",
    "journal",
    "raw_tag",
    "setting",
];

const MIGRATIONS: &[&str] = &[include_str!("schema_v1.sql")];

pub struct Store {
    conn: Connection,
    columns: HashMap<&'static str, Vec<String>>,
    group: Option<i64>,
}

#[derive(Debug, Clone)]
struct Op {
    table: &'static str,
    id: String,
    before: Option<Row>,
    after: Option<Row>,
}

pub struct Tx<'a> {
    conn: &'a Connection,
    columns: &'a HashMap<&'static str, Vec<String>>,
    ops: Vec<Op>,
    track: bool,
}

fn table_name(t: &str) -> Result<&'static str> {
    TABLES
        .iter()
        .find(|x| **x == t)
        .copied()
        .ok_or_else(|| StoreError::UnknownTable(t.to_string()))
}

fn value_ref(v: ValueRef) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(n) => Value::from(n),
        ValueRef::Real(f) => Value::from(f),
        ValueRef::Text(t) => Value::from(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(_) => Value::Null,
    }
}

fn read_row(conn: &Connection, table: &str, cols: &[String], id: &str) -> Result<Option<Row>> {
    // `_rowid` is carried in row images so undo/redo restores a row to its exact original position.
    let sql = format!(
        "SELECT rowid, {} FROM {} WHERE id = ?1",
        cols.join(","),
        table
    );
    Ok(conn
        .prepare_cached(&sql)?
        .query_row([id], |r| {
            let mut m = Row::new();
            m.insert("_rowid".into(), value_ref(r.get_ref(0)?));
            for (i, c) in cols.iter().enumerate() {
                m.insert(c.clone(), value_ref(r.get_ref(i + 1)?));
            }
            Ok(m)
        })
        .optional()?)
}

fn write_row(conn: &Connection, table: &str, row: &Row) -> Result<()> {
    let cols: Vec<&String> = row.keys().collect();
    let sql = format!(
        "INSERT OR REPLACE INTO {} ({}) VALUES ({})",
        table,
        cols.iter()
            .map(|c| if c.as_str() == "_rowid" {
                "rowid"
            } else {
                c.as_str()
            })
            .collect::<Vec<_>>()
            .join(","),
        (1..=cols.len())
            .map(|i| format!("?{}", i))
            .collect::<Vec<_>>()
            .join(",")
    );
    let vals: Vec<rusqlite::types::Value> = cols
        .iter()
        .map(|c| match &row[*c] {
            Value::Null => rusqlite::types::Value::Null,
            Value::Bool(b) => rusqlite::types::Value::Integer(*b as i64),
            Value::Number(n) => match n.as_i64() {
                Some(i) => rusqlite::types::Value::Integer(i),
                None => rusqlite::types::Value::Real(n.as_f64().unwrap_or(0.0)),
            },
            Value::String(s) => rusqlite::types::Value::Text(s.clone()),
            other => rusqlite::types::Value::Text(other.to_string()),
        })
        .collect();
    conn.prepare_cached(&sql)?
        .execute(rusqlite::params_from_iter(vals))?;
    Ok(())
}

impl<'a> Tx<'a> {
    pub fn get(&self, table: &str, id: &str) -> Result<Option<Row>> {
        let t = table_name(table)?;
        read_row(self.conn, t, &self.columns[t], id)
    }

    /// Insert or update. Fields omitted from `row` keep their stored values on update.
    pub fn put_row(&mut self, table: &str, mut row: Row) -> Result<String> {
        let t = table_name(table)?;
        let id = match row.get("id") {
            Some(Value::String(s)) => s.clone(),
            _ => return Err(StoreError::MissingId(table.into())),
        };
        let cols = &self.columns[t];
        let before = read_row(self.conn, t, cols, &id)?;
        row.retain(|k, _| cols.contains(k));
        let mut after = before.clone().unwrap_or_else(|| {
            let mut m = Row::new();
            for c in cols {
                m.insert(c.clone(), Value::Null);
            }
            m
        });
        let ts = now();
        if before.is_none() && cols.iter().any(|c| c == "created") {
            after.insert("created".into(), ts.into());
        }
        if cols.iter().any(|c| c == "modified") {
            after.insert("modified".into(), ts.into());
        }
        for (k, v) in row {
            after.insert(k, v);
        }
        write_row(self.conn, t, &after)?;
        if !after.contains_key("_rowid") {
            after.insert("_rowid".into(), self.conn.last_insert_rowid().into());
        }
        if self.track {
            self.ops.push(Op {
                table: t,
                id: id.clone(),
                before,
                after: Some(after),
            });
        }
        Ok(id)
    }

    /// Typed insert/update; the struct's field names must match column names.
    pub fn put<T: Serialize>(&mut self, table: &str, value: &T) -> Result<String> {
        match serde_json::to_value(value)? {
            Value::Object(m) => self.put_row(table, m),
            _ => Err(StoreError::Other("put requires an object".into())),
        }
    }

    pub fn get_as<T: DeserializeOwned>(&self, table: &str, id: &str) -> Result<Option<T>> {
        Ok(match self.get(table, id)? {
            Some(r) => Some(serde_json::from_value(Value::Object(r))?),
            None => None,
        })
    }

    pub fn delete(&mut self, table: &str, id: &str) -> Result<bool> {
        let t = table_name(table)?;
        let Some(before) = read_row(self.conn, t, &self.columns[t], id)? else {
            return Ok(false);
        };
        self.conn
            .execute(&format!("DELETE FROM {} WHERE id = ?1", t), [id])?;
        if self.track {
            self.ops.push(Op {
                table: t,
                id: id.to_string(),
                before: Some(before),
                after: None,
            });
        }
        Ok(true)
    }

    /// IDs of rows in `table` where `col = value`.
    pub fn ids_where(&self, table: &str, col: &str, value: &str) -> Result<Vec<String>> {
        let t = table_name(table)?;
        if !self.columns[t].iter().any(|c| c == col) {
            return Err(StoreError::Other(format!("no column {}.{}", t, col)));
        }
        let mut st = self.conn.prepare(&format!(
            "SELECT id FROM {} WHERE {} = ?1 ORDER BY rowid",
            t, col
        ))?;
        let ids = st
            .query_map([value], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ids)
    }
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Store::init(conn)
    }

    pub fn open_memory() -> Result<Store> {
        Store::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Store> {
        conn.set_prepared_statement_cache_capacity(256);
        let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if v > MIGRATIONS.len() as i64 {
            return Err(StoreError::TooNew {
                found: v,
                supported: MIGRATIONS.len() as i64,
            });
        }
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(v as usize) {
            conn.execute_batch(&format!(
                "BEGIN; {} PRAGMA user_version = {}; COMMIT;",
                sql,
                i + 1
            ))?;
        }
        let mut columns = HashMap::new();
        for t in TABLES {
            let mut st = conn.prepare(&format!("PRAGMA table_info({})", t))?;
            let cols = st
                .query_map([], |r| r.get::<_, String>(1))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            columns.insert(*t, cols);
        }
        Ok(Store {
            conn,
            columns,
            group: None,
        })
    }

    pub fn schema_version(&self) -> i64 {
        self.conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap_or(0)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Run `f` atomically as one undoable command (or part of the current group).
    pub fn transact<T>(&mut self, label: &str, f: impl FnOnce(&mut Tx) -> Result<T>) -> Result<T> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let mut tx = Tx {
            conn: &self.conn,
            columns: &self.columns,
            ops: Vec::new(),
            track: true,
        };
        let out = f(&mut tx);
        let ops = std::mem::take(&mut tx.ops);
        match out {
            Err(e) => {
                self.conn.execute_batch("ROLLBACK")?;
                Err(e)
            }
            Ok(v) => {
                let res = self.record(label, &ops);
                match res {
                    Ok(()) => {
                        self.conn.execute_batch("COMMIT")?;
                        self.reindex_for(&ops)?;
                        Ok(v)
                    }
                    Err(e) => {
                        self.conn.execute_batch("ROLLBACK")?;
                        Err(e)
                    }
                }
            }
        }
    }

    fn record(&mut self, label: &str, ops: &[Op]) -> Result<()> {
        if ops.is_empty() {
            return Ok(());
        }
        // A new command invalidates the redo stack.
        self.conn.execute(
            "DELETE FROM history_ops WHERE history_id IN (SELECT id FROM history WHERE undone = 1)",
            [],
        )?;
        self.conn
            .execute("DELETE FROM history WHERE undone = 1", [])?;
        self.conn.execute(
            "INSERT INTO history (group_id, label, ts, undone) VALUES (?1, ?2, ?3, 0)",
            params![self.group, label, now()],
        )?;
        let hid = self.conn.last_insert_rowid();
        if self.group.is_none() {
            self.conn
                .execute("UPDATE history SET group_id = id WHERE id = ?1", [hid])?;
        }
        for (seq, op) in ops.iter().enumerate() {
            self.conn.execute(
                "INSERT INTO history_ops (history_id, seq, tbl, row_id, before_json, after_json) VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    hid,
                    seq as i64,
                    op.table,
                    op.id,
                    op.before.as_ref().map(serde_json::to_string).transpose()?,
                    op.after.as_ref().map(serde_json::to_string).transpose()?
                ],
            )?;
        }
        Ok(())
    }

    /// Start merging subsequent commands into a single undo step.
    pub fn begin_group(&mut self) -> Result<()> {
        if self.group.is_none() {
            let next: i64 =
                self.conn
                    .query_row("SELECT COALESCE(MAX(id),0)+1 FROM history", [], |r| {
                        r.get(0)
                    })?;
            self.group = Some(next);
        }
        Ok(())
    }
    pub fn end_group(&mut self) {
        self.group = None;
    }

    pub fn can_undo(&self) -> bool {
        self.conn
            .query_row("SELECT 1 FROM history WHERE undone = 0 LIMIT 1", [], |_| {
                Ok(())
            })
            .optional()
            .ok()
            .flatten()
            .is_some()
    }
    pub fn can_redo(&self) -> bool {
        self.conn
            .query_row("SELECT 1 FROM history WHERE undone = 1 LIMIT 1", [], |_| {
                Ok(())
            })
            .optional()
            .ok()
            .flatten()
            .is_some()
    }
    pub fn undo_label(&self) -> Option<String> {
        self.conn
            .query_row(
                "SELECT label FROM history WHERE undone = 0 ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()
            .ok()
            .flatten()
    }

    /// Undo the most recent step (a whole group). Returns its label.
    pub fn undo(&mut self) -> Result<Option<String>> {
        let Some(gid): Option<i64> = self
            .conn
            .query_row(
                "SELECT group_id FROM history WHERE undone = 0 ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?
        else {
            return Ok(None);
        };
        self.replay(gid, false)
    }

    pub fn redo(&mut self) -> Result<Option<String>> {
        let Some(gid): Option<i64> = self
            .conn
            .query_row(
                "SELECT group_id FROM history WHERE undone = 1 ORDER BY id ASC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?
        else {
            return Ok(None);
        };
        self.replay(gid, true)
    }

    fn replay(&mut self, gid: i64, forward: bool) -> Result<Option<String>> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let r = (|| -> Result<Option<String>> {
            let mut st = self.conn.prepare(&format!(
                "SELECT id, label FROM history WHERE group_id = ?1 AND undone = {} ORDER BY id {}",
                if forward { 1 } else { 0 },
                if forward { "ASC" } else { "DESC" }
            ))?;
            let entries: Vec<(i64, String)> = st
                .query_map([gid], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<std::result::Result<_, _>>()?;
            drop(st);
            let label = entries.first().map(|e| e.1.clone());
            let mut touched: Vec<Op> = Vec::new();
            for (hid, _) in &entries {
                let mut st = self.conn.prepare(&format!(
                    "SELECT tbl, row_id, before_json, after_json FROM history_ops WHERE history_id = ?1 ORDER BY seq {}",
                    if forward { "ASC" } else { "DESC" }
                ))?;
                let ops: Vec<(String, String, Option<String>, Option<String>)> = st
                    .query_map([hid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                    .collect::<std::result::Result<_, _>>()?;
                drop(st);
                for (tbl, id, b, a) in ops {
                    let t = table_name(&tbl)?;
                    let target = if forward { a } else { b };
                    match target {
                        Some(json) => {
                            write_row(&self.conn, t, &serde_json::from_str::<Row>(&json)?)?
                        }
                        None => {
                            self.conn
                                .execute(&format!("DELETE FROM {} WHERE id = ?1", t), [&id])?;
                        }
                    }
                    touched.push(Op {
                        table: t,
                        id,
                        before: None,
                        after: None,
                    });
                }
                self.conn.execute(
                    "UPDATE history SET undone = ?1 WHERE id = ?2",
                    params![!forward, hid],
                )?;
            }
            self.reindex_for(&touched)?;
            Ok(label)
        })();
        match r {
            Ok(v) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    // ---------- search ----------

    fn reindex_for(&self, ops: &[Op]) -> Result<()> {
        let mut persons: BTreeSet<String> = BTreeSet::new();
        for op in ops {
            let rows = [op.before.as_ref(), op.after.as_ref()];
            match op.table {
                "person" => {
                    persons.insert(op.id.clone());
                }
                "person_name" => {
                    for r in rows.into_iter().flatten() {
                        if let Some(Value::String(p)) = r.get("person_id") {
                            persons.insert(p.clone());
                        }
                    }
                }
                "event" => {
                    for r in rows.into_iter().flatten() {
                        if r.get("owner_type") == Some(&Value::String("person".into())) {
                            if let Some(Value::String(p)) = r.get("owner_id") {
                                persons.insert(p.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        for p in persons {
            self.reindex_person(&p)?;
        }
        Ok(())
    }

    pub fn reindex_person(&self, id: &str) -> Result<()> {
        let rid: Option<i64> = self
            .conn
            .query_row(
                "SELECT fts_rowid FROM search_map WHERE entity_id = ?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(r) = rid {
            self.conn
                .execute("DELETE FROM search_index WHERE rowid = ?1", [r])?;
        }
        let exists: bool = self
            .conn
            .query_row("SELECT 1 FROM person WHERE id = ?1", [id], |_| Ok(()))
            .optional()?
            .is_some();
        if !exists {
            self.conn
                .execute("DELETE FROM search_map WHERE entity_id = ?1", [id])?;
            return Ok(());
        }
        let rid = match rid {
            Some(r) => r,
            None => {
                self.conn
                    .execute("INSERT INTO search_map (entity_id) VALUES (?1)", [id])?;
                self.conn.last_insert_rowid()
            }
        };
        let mut text = String::new();
        let mut st = self.conn.prepare_cached(
            "SELECT prefix, given, nickname, surname_prefix, surname, suffix FROM person_name WHERE person_id = ?1",
        )?;
        let names = st.query_map([id], |r| {
            (0..6)
                .map(|i| r.get::<_, Option<String>>(i).map(|s| s.unwrap_or_default()))
                .collect::<std::result::Result<Vec<_>, _>>()
        })?;
        for n in names {
            text.push_str(&n?.join(" "));
            text.push(' ');
        }
        let mut st = self.conn.prepare_cached(
            "SELECT e.kind, e.description, p.name FROM event e LEFT JOIN place p ON p.id = e.place_id WHERE e.owner_type='person' AND e.owner_id = ?1",
        )?;
        let evs = st.query_map([id], |r| {
            Ok([r.get::<_, Option<String>>(0)?, r.get(1)?, r.get(2)?])
        })?;
        for e in evs {
            for part in e?.into_iter().flatten() {
                text.push_str(&part);
                text.push(' ');
            }
        }
        self.conn.execute(
            "INSERT INTO search_index (rowid, entity_type, entity_id, body) VALUES (?1, 'person', ?2, ?3)",
            params![rid, id, crate::name::fold(&text)],
        )?;
        Ok(())
    }

    /// Prefix search over folded text; every token must match. Returns entity ids (persons).
    pub fn search_persons(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        let folded = crate::name::fold(query);
        let toks: Vec<String> = folded
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .map(|t| format!("\"{}\"*", t))
            .collect();
        if toks.is_empty() {
            return Ok(vec![]);
        }
        let mut st = self.conn.prepare(
            "SELECT entity_id FROM search_index WHERE entity_type = 'person' AND search_index MATCH ?1 ORDER BY rank LIMIT ?2",
        )?;
        let ids = st
            .query_map(params![toks.join(" AND "), limit as i64], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    /// Run `f` without recording history (bulk import into a fresh project). Clears history and rebuilds the search index.
    pub fn transact_untracked<T>(&mut self, f: impl FnOnce(&mut Tx) -> Result<T>) -> Result<T> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let mut tx = Tx {
            conn: &self.conn,
            columns: &self.columns,
            ops: Vec::new(),
            track: false,
        };
        let out = f(&mut tx);
        match out {
            Err(e) => {
                self.conn.execute_batch("ROLLBACK")?;
                Err(e)
            }
            Ok(v) => {
                self.conn
                    .execute_batch("DELETE FROM history_ops; DELETE FROM history;")?;
                self.conn.execute_batch("COMMIT")?;
                self.reindex_all()?;
                Ok(v)
            }
        }
    }

    pub fn reindex_all(&self) -> Result<()> {
        self.conn.execute("DELETE FROM search_index", [])?;
        self.conn.execute("DELETE FROM search_map", [])?;
        let mut st = self.conn.prepare("SELECT id FROM person ORDER BY rowid")?;
        let ids = st
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for id in ids {
            self.reindex_person(&id)?;
        }
        Ok(())
    }

    /// All rows of a table in insertion order (single query).
    pub fn rows(&self, table: &str) -> Result<Vec<Row>> {
        let t = table_name(table)?;
        let cols = &self.columns[t];
        let sql = format!("SELECT {} FROM {} ORDER BY rowid", cols.join(","), t);
        let mut st = self.conn.prepare(&sql)?;
        let out = st
            .query_map([], |r| {
                let mut m = Row::new();
                for (i, c) in cols.iter().enumerate() {
                    m.insert(c.clone(), value_ref(r.get_ref(i)?));
                }
                Ok(m)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(out)
    }

    pub fn count(&self, table: &str) -> Result<i64> {
        let t = table_name(table)?;
        Ok(self
            .conn
            .query_row(&format!("SELECT COUNT(*) FROM {}", t), [], |r| r.get(0))?)
    }
}
