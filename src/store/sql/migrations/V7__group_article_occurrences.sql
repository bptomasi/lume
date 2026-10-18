ALTER TABLE articles ADD COLUMN canonical_key TEXT NOT NULL DEFAULT '';
CREATE INDEX article_occurrences ON articles (canonical_key, deleted, id);

PRAGMA user_version = 7;
