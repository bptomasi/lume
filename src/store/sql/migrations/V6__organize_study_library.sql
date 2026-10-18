ALTER TABLE articles ADD COLUMN reading_minutes INTEGER NOT NULL DEFAULT 0
    CHECK (reading_minutes >= 0);
ALTER TABLE articles ADD COLUMN added_at INTEGER NOT NULL DEFAULT 0;

CREATE TABLE article_tags (
    article_id INTEGER NOT NULL REFERENCES articles (id) ON DELETE CASCADE,
    tag TEXT NOT NULL COLLATE NOCASE,
    PRIMARY KEY (article_id, tag)
);

CREATE INDEX article_tags_lookup ON article_tags (tag, article_id);

CREATE TABLE comment_tags (
    comment_id INTEGER NOT NULL REFERENCES comments (id) ON DELETE CASCADE,
    tag TEXT NOT NULL COLLATE NOCASE,
    PRIMARY KEY (comment_id, tag)
);

CREATE TABLE note_links (
    comment_id INTEGER NOT NULL REFERENCES comments (id) ON DELETE CASCADE,
    article_id INTEGER NOT NULL REFERENCES articles (id) ON DELETE CASCADE,
    PRIMARY KEY (comment_id, article_id)
);

CREATE TABLE annotation_reviews (
    comment_id INTEGER PRIMARY KEY REFERENCES comments (id) ON DELETE CASCADE,
    due_at INTEGER NOT NULL DEFAULT 0,
    interval_days INTEGER NOT NULL DEFAULT 1 CHECK (interval_days >= 1),
    reviews INTEGER NOT NULL DEFAULT 0 CHECK (reviews >= 0)
);

CREATE TABLE study_cards (
    id INTEGER PRIMARY KEY,
    comment_id INTEGER NOT NULL REFERENCES comments (id) ON DELETE CASCADE,
    question TEXT NOT NULL,
    answer TEXT NOT NULL,
    due_at INTEGER NOT NULL DEFAULT 0,
    interval_days INTEGER NOT NULL DEFAULT 1 CHECK (interval_days >= 1),
    reviews INTEGER NOT NULL DEFAULT 0 CHECK (reviews >= 0)
);

CREATE INDEX study_cards_due ON study_cards (due_at, id);

CREATE TABLE export_manifest (
    namespace TEXT NOT NULL,
    article_id INTEGER NOT NULL REFERENCES articles (id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    PRIMARY KEY (namespace, article_id)
);

PRAGMA user_version = 6;
