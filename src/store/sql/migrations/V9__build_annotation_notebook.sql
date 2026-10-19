CREATE TABLE note_identities (
    id INTEGER PRIMARY KEY AUTOINCREMENT
);

INSERT INTO note_identities (id)
SELECT id FROM comments;

CREATE TABLE note_metadata (
    comment_id INTEGER PRIMARY KEY REFERENCES comments(id) ON DELETE CASCADE,
    kind TEXT NOT NULL DEFAULT 'insight'
        CHECK (kind IN ('insight', 'question', 'definition', 'example',
                       'todo', 'synthesis')),
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved')),
    pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);

INSERT INTO note_metadata (comment_id, updated_at)
SELECT id, created FROM comments;

CREATE INDEX note_metadata_order
ON note_metadata (pinned DESC, updated_at DESC, comment_id DESC);

CREATE TABLE note_references (
    source_id INTEGER NOT NULL REFERENCES comments(id) ON DELETE CASCADE,
    target_id INTEGER NOT NULL REFERENCES comments(id) ON DELETE CASCADE,
    PRIMARY KEY (source_id, target_id),
    CHECK (source_id <> target_id)
);

CREATE INDEX note_references_target ON note_references (target_id, source_id);

CREATE TABLE note_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    note_id INTEGER NOT NULL REFERENCES note_identities(id),
    article_id INTEGER NOT NULL REFERENCES articles(id),
    body TEXT NOT NULL,
    quote TEXT NOT NULL,
    anchor INTEGER NOT NULL,
    source TEXT NOT NULL,
    source_revision INTEGER NOT NULL,
    saved_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX note_versions_owner ON note_versions (note_id, id DESC);

INSERT INTO note_versions (
    note_id, article_id, body, quote, anchor, source, source_revision, saved_at
)
SELECT id, article_id, body, quote, anchor, source, source_revision, created
FROM comments;

CREATE TABLE note_drafts (
    key TEXT PRIMARY KEY,
    target TEXT NOT NULL CHECK (length(CAST(target AS BLOB)) <= 32768),
    body TEXT NOT NULL CHECK (length(CAST(body AS BLOB)) <= 16384),
    baseline TEXT NOT NULL CHECK (length(CAST(baseline AS BLOB)) <= 16384),
    initial_kind TEXT,
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE note_collections (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    query TEXT NOT NULL CHECK (length(CAST(query AS BLOB)) <= 4096)
);

CREATE TABLE note_export_manifest (
    namespace TEXT NOT NULL,
    note_id INTEGER NOT NULL REFERENCES note_identities(id),
    content TEXT NOT NULL CHECK (length(CAST(content AS BLOB)) <= 131072),
    PRIMARY KEY (namespace, note_id)
);

CREATE TABLE note_search_history (
    query TEXT PRIMARY KEY,
    sequence INTEGER NOT NULL
);

CREATE VIRTUAL TABLE note_search USING fts5(
    body,
    quote,
    content = 'comments',
    content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2',
    prefix = '2 3'
);

INSERT INTO note_search (note_search) VALUES ('rebuild');

DROP TRIGGER comments_search_insert;
DROP TRIGGER comments_search_update;
DROP TRIGGER comments_search_delete;

-- Article text is indexed independently. Editing one note must not reindex
-- a potentially 512 KiB cached article and all of its other annotations.
UPDATE article_search SET notes = '';

CREATE TRIGGER comments_search_insert AFTER INSERT ON comments
BEGIN
    INSERT OR IGNORE INTO note_identities (id) VALUES (NEW.id);
    INSERT INTO note_search (rowid, body, quote)
    VALUES (NEW.id, NEW.body, NEW.quote);
    INSERT INTO note_metadata (comment_id, updated_at)
    VALUES (NEW.id, NEW.created);
    INSERT INTO note_versions (
        note_id, article_id, body, quote, anchor, source, source_revision
    )
    VALUES (
        NEW.id, NEW.article_id, NEW.body, NEW.quote, NEW.anchor,
        NEW.source, NEW.source_revision
    );
    DELETE FROM note_versions
    WHERE note_id = NEW.id AND id NOT IN (
        SELECT id FROM note_versions
        WHERE note_id = NEW.id
        ORDER BY id DESC
        LIMIT 100
    );
END;

CREATE TRIGGER comments_search_update
AFTER UPDATE OF body, quote, anchor, source, source_revision ON comments
WHEN OLD.body <> NEW.body OR OLD.quote <> NEW.quote
    OR OLD.anchor <> NEW.anchor OR OLD.source <> NEW.source
    OR OLD.source_revision <> NEW.source_revision
BEGIN
    INSERT INTO note_search (note_search, rowid, body, quote)
    VALUES ('delete', OLD.id, OLD.body, OLD.quote);
    INSERT INTO note_search (rowid, body, quote)
    VALUES (NEW.id, NEW.body, NEW.quote);
    UPDATE note_metadata SET updated_at = unixepoch()
    WHERE comment_id = NEW.id;
    INSERT INTO note_versions (
        note_id, article_id, body, quote, anchor, source, source_revision
    )
    VALUES (
        NEW.id, NEW.article_id, NEW.body, NEW.quote, NEW.anchor,
        NEW.source, NEW.source_revision
    );
    DELETE FROM note_versions
    WHERE note_id = NEW.id AND id NOT IN (
        SELECT id FROM note_versions
        WHERE note_id = NEW.id
        ORDER BY id DESC
        LIMIT 100
    );
END;

CREATE TRIGGER comments_search_delete AFTER DELETE ON comments
BEGIN
    INSERT INTO note_search (note_search, rowid, body, quote)
    VALUES ('delete', OLD.id, OLD.body, OLD.quote);
END;

PRAGMA user_version = 9;
