CREATE TABLE person (
  id TEXT PRIMARY KEY, xref TEXT, sex TEXT NOT NULL DEFAULT 'U', living_override INTEGER, is_private INTEGER NOT NULL DEFAULT 0,
  primary_media TEXT, color TEXT, bookmarked INTEGER NOT NULL DEFAULT 0, ref_no TEXT, custom TEXT,
  created INTEGER, modified INTEGER);
CREATE TABLE person_name (
  id TEXT PRIMARY KEY, person_id TEXT NOT NULL, kind TEXT NOT NULL DEFAULT 'Birth', prefix TEXT, given TEXT, nickname TEXT,
  surname_prefix TEXT, surname TEXT, suffix TEXT, src_tags TEXT, sort_order INTEGER NOT NULL DEFAULT 0, created INTEGER, modified INTEGER);
CREATE INDEX idx_name_person ON person_name(person_id);
CREATE INDEX idx_name_surname ON person_name(surname);
CREATE TABLE family (
  id TEXT PRIMARY KEY, xref TEXT, partner1 TEXT, partner2 TEXT, rel_type TEXT NOT NULL DEFAULT 'married', created INTEGER, modified INTEGER);
CREATE INDEX idx_family_p1 ON family(partner1);
CREATE INDEX idx_family_p2 ON family(partner2);
CREATE TABLE family_child (
  id TEXT PRIMARY KEY, family_id TEXT NOT NULL, person_id TEXT NOT NULL, rel_type TEXT NOT NULL DEFAULT 'biological',
  sort_order INTEGER NOT NULL DEFAULT 0, created INTEGER, modified INTEGER);
CREATE INDEX idx_fc_family ON family_child(family_id);
CREATE INDEX idx_fc_person ON family_child(person_id);
CREATE TABLE event (
  id TEXT PRIMARY KEY, owner_type TEXT NOT NULL, owner_id TEXT NOT NULL, kind TEXT NOT NULL, custom_kind TEXT, value TEXT, plac_map INTEGER,
  date_json TEXT, date_sort INTEGER, date_sort_end INTEGER, place_id TEXT, description TEXT, cause TEXT, agency TEXT,
  is_private INTEGER NOT NULL DEFAULT 0, sort_order INTEGER NOT NULL DEFAULT 0, created INTEGER, modified INTEGER);
CREATE INDEX idx_event_owner ON event(owner_type, owner_id);
CREATE INDEX idx_event_kind_date ON event(kind, date_sort);
CREATE INDEX idx_event_place ON event(place_id);
CREATE TABLE place (
  id TEXT PRIMARY KEY, parent_id TEXT, name TEXT NOT NULL, place_type TEXT, lat REAL, lon REAL,
  geocode_status TEXT, alt_names TEXT, valid_from INTEGER, valid_to INTEGER, standardized TEXT, created INTEGER, modified INTEGER);
CREATE INDEX idx_place_parent ON place(parent_id);
CREATE TABLE repository (
  id TEXT PRIMARY KEY, xref TEXT, name TEXT NOT NULL, address TEXT, contacts TEXT, website TEXT, notes TEXT, created INTEGER, modified INTEGER);
CREATE TABLE source (
  id TEXT PRIMARY KEY, xref TEXT, title TEXT NOT NULL, author TEXT, publication TEXT, repository_id TEXT, kind TEXT,
  reliability INTEGER, text TEXT, inline INTEGER NOT NULL DEFAULT 0, created INTEGER, modified INTEGER);
CREATE TABLE citation (
  id TEXT PRIMARY KEY, source_id TEXT NOT NULL, target_type TEXT NOT NULL, target_id TEXT NOT NULL, page TEXT,
  quality TEXT, transcription TEXT, created INTEGER, modified INTEGER);
CREATE INDEX idx_cit_target ON citation(target_type, target_id);
CREATE INDEX idx_cit_source ON citation(source_id);
CREATE TABLE media (
  id TEXT PRIMARY KEY, xref TEXT, kind TEXT, path TEXT NOT NULL, inline INTEGER NOT NULL DEFAULT 0, mode TEXT NOT NULL DEFAULT 'embedded', caption TEXT, date_json TEXT,
  place_id TEXT, hash TEXT, width INTEGER, height INTEGER, meta TEXT, created INTEGER, modified INTEGER);
CREATE INDEX idx_media_hash ON media(hash);
CREATE TABLE media_link (
  id TEXT PRIMARY KEY, media_id TEXT NOT NULL, target_type TEXT NOT NULL, target_id TEXT NOT NULL, region TEXT, sort_order INTEGER NOT NULL DEFAULT 0);
CREATE INDEX idx_ml_target ON media_link(target_type, target_id);
CREATE TABLE note (id TEXT PRIMARY KEY, xref TEXT, inline INTEGER NOT NULL DEFAULT 0, title TEXT, body TEXT NOT NULL DEFAULT '', created INTEGER, modified INTEGER);
CREATE TABLE note_link (id TEXT PRIMARY KEY, note_id TEXT NOT NULL, target_type TEXT NOT NULL, target_id TEXT NOT NULL);
CREATE INDEX idx_nl_target ON note_link(target_type, target_id);
CREATE TABLE task (
  id TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT, status TEXT NOT NULL DEFAULT 'open', priority INTEGER NOT NULL DEFAULT 0,
  due INTEGER, created INTEGER, modified INTEGER);
CREATE TABLE task_link (id TEXT PRIMARY KEY, task_id TEXT NOT NULL, target_type TEXT NOT NULL, target_id TEXT NOT NULL);
CREATE TABLE association (
  id TEXT PRIMARY KEY, person_id TEXT NOT NULL, other_id TEXT NOT NULL, role TEXT NOT NULL, notes TEXT, created INTEGER, modified INTEGER);
CREATE TABLE tag (id TEXT PRIMARY KEY, name TEXT NOT NULL, color TEXT, created INTEGER, modified INTEGER);
CREATE TABLE tag_link (id TEXT PRIMARY KEY, tag_id TEXT NOT NULL, target_type TEXT NOT NULL, target_id TEXT NOT NULL);
CREATE INDEX idx_tl_target ON tag_link(target_type, target_id);
CREATE TABLE story (id TEXT PRIMARY KEY, title TEXT NOT NULL, blocks TEXT NOT NULL DEFAULT '[]', sort_order INTEGER NOT NULL DEFAULT 0, created INTEGER, modified INTEGER);
CREATE TABLE journal (id TEXT PRIMARY KEY, title TEXT, body TEXT, target_type TEXT, target_id TEXT, created INTEGER, modified INTEGER);
-- Unknown GEDCOM structures preserved verbatim for lossless re-export.
CREATE TABLE raw_tag (id TEXT PRIMARY KEY, owner_type TEXT NOT NULL, owner_id TEXT NOT NULL, seq INTEGER NOT NULL, line TEXT NOT NULL);
CREATE INDEX idx_raw_owner ON raw_tag(owner_type, owner_id);
CREATE TABLE setting (id TEXT PRIMARY KEY, value TEXT);

CREATE TABLE history (
  id INTEGER PRIMARY KEY AUTOINCREMENT, group_id INTEGER, label TEXT NOT NULL, ts INTEGER NOT NULL, undone INTEGER NOT NULL DEFAULT 0);
CREATE INDEX idx_history_group ON history(group_id);
CREATE TABLE history_ops (
  history_id INTEGER NOT NULL, seq INTEGER NOT NULL, tbl TEXT NOT NULL, row_id TEXT NOT NULL, before_json TEXT, after_json TEXT,
  PRIMARY KEY (history_id, seq));

CREATE TABLE search_map (fts_rowid INTEGER PRIMARY KEY AUTOINCREMENT, entity_id TEXT NOT NULL UNIQUE);
CREATE VIRTUAL TABLE search_index USING fts5(entity_type UNINDEXED, entity_id UNINDEXED, body, tokenize = "unicode61 remove_diacritics 2");
