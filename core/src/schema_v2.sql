-- Embedded media bytes live outside the undo log: deleting a media row keeps its blob so undo works; `purge_orphans` reclaims it.
CREATE TABLE media_blob (media_id TEXT PRIMARY KEY, data BLOB NOT NULL, thumb BLOB);
