ALTER TABLE articles ADD COLUMN reading_state_at INTEGER NOT NULL DEFAULT 0;
CREATE TRIGGER retention_read_insert AFTER INSERT ON articles
WHEN NEW.unread = 0
BEGIN
    UPDATE articles SET reading_state_at = unixepoch() WHERE id = NEW.id;
END;
CREATE TRIGGER retention_read_update AFTER UPDATE OF unread ON articles
WHEN NEW.unread IS NOT OLD.unread
BEGIN
    UPDATE articles SET reading_state_at = unixepoch() WHERE id = NEW.id;
END;
CREATE INDEX articles_retention_kept ON articles(id)
WHERE starred = 1 OR queued = 1;
CREATE INDEX note_links_article ON note_links(article_id);

-- Keep issued identities monotonic even after the highest article is removed.
CREATE TABLE article_identity_clock (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    last_id INTEGER NOT NULL,
    legacy_upper_id INTEGER NOT NULL,
    legacy_state_at INTEGER NOT NULL
);
-- A single adoption marker gives existing read items a grace period without
-- rewriting every article just to enable an initially inactive capability.
INSERT INTO article_identity_clock
SELECT 1, coalesce(max(id), 0), coalesce(max(id), 0), unixepoch() FROM articles;
CREATE TRIGGER article_identity_insert AFTER INSERT ON articles
BEGIN
    UPDATE article_identity_clock SET last_id = max(last_id, NEW.id)
    WHERE singleton = 1;
END;

CREATE TABLE retention_tombstones (
    feed_id INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    external_id TEXT NOT NULL,
    canonical_key TEXT NOT NULL DEFAULT '',
    removed_at INTEGER NOT NULL,
    PRIMARY KEY (feed_id, external_id)
);
CREATE INDEX retention_removed_canonical
ON retention_tombstones(feed_id, canonical_key) WHERE canonical_key <> '';

CREATE TRIGGER configuration_retention_insert AFTER INSERT ON settings
WHEN NEW.key = 'retention_policy'
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;
CREATE TRIGGER configuration_retention_update AFTER UPDATE ON settings
WHEN NEW.key = 'retention_policy' AND NEW.value IS NOT OLD.value
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;
CREATE TRIGGER configuration_retention_delete AFTER DELETE ON settings
WHEN OLD.key = 'retention_policy'
BEGIN
    UPDATE configuration_state SET revision = revision + 1;
END;
PRAGMA user_version = 21;
