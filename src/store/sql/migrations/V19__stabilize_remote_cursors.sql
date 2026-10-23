-- Resume by immutable remote identity, not an offset into a changing result set.
-- Restart incomplete legacy passes at their retained timestamp watermark.
ALTER TABLE server_feeds ADD COLUMN sync_after_id INTEGER NOT NULL DEFAULT 0;
UPDATE server_feeds SET sync_offset = 0;

-- Distinct remote IDs can refer to the same canonical local article.
CREATE TABLE server_entries_v19 (
    server_id INTEGER NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    remote_id INTEGER NOT NULL,
    article_id INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    PRIMARY KEY (server_id, remote_id)
);
INSERT INTO server_entries_v19 SELECT server_id, remote_id, article_id
FROM server_entries;
DROP TABLE server_entries;
ALTER TABLE server_entries_v19 RENAME TO server_entries;
CREATE INDEX server_entries_article ON server_entries(article_id);

PRAGMA user_version = 19;
