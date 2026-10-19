-- Classification is disposable; personal preferences and feedback are durable.
CREATE TABLE recommendation_dirty (
    article_id INTEGER PRIMARY KEY REFERENCES articles(id) ON DELETE CASCADE,
    analyze INTEGER NOT NULL CHECK (analyze IN (0, 1))
);

CREATE TABLE recommendation_matches (
    article_id INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    topic TEXT NOT NULL,
    headline INTEGER NOT NULL CHECK (headline IN (0, 1)),
    PRIMARY KEY (article_id, topic)
);
CREATE INDEX recommendation_topic ON recommendation_matches (topic, article_id);

CREATE TABLE recommendation_features (
    article_id INTEGER PRIMARY KEY REFERENCES articles(id) ON DELETE CASCADE,
    score INTEGER NOT NULL,
    minutes INTEGER NOT NULL,
    evidence TEXT NOT NULL CHECK (length(CAST(evidence AS BLOB)) <= 8192),
    excluded INTEGER NOT NULL CHECK (excluded IN (0, 1))
);
CREATE INDEX recommendation_score
    ON recommendation_features (score DESC, article_id DESC);

CREATE TABLE recommendation_items (
    article_id INTEGER PRIMARY KEY REFERENCES articles(id) ON DELETE CASCADE,
    position INTEGER NOT NULL UNIQUE
);

CREATE TABLE recommendation_feedback (
    article_id INTEGER PRIMARY KEY REFERENCES articles(id) ON DELETE CASCADE,
    feedback TEXT NOT NULL CHECK (
        feedback IN ('useful', 'not-interested', 'too-basic', 'too-advanced')
    )
);
CREATE TABLE recommendation_feedback_topics (
    article_id INTEGER NOT NULL
        REFERENCES recommendation_feedback(article_id) ON DELETE CASCADE,
    topic TEXT NOT NULL,
    PRIMARY KEY (article_id, topic)
);

INSERT INTO recommendation_dirty (article_id, analyze)
SELECT id, 1 FROM articles;

CREATE TRIGGER recommendation_article_insert
AFTER INSERT ON articles
BEGIN
    INSERT INTO recommendation_dirty (article_id, analyze)
    VALUES (NEW.id, 1)
    ON CONFLICT (article_id) DO UPDATE
    SET analyze = max(recommendation_dirty.analyze, excluded.analyze);
END;

CREATE TRIGGER recommendation_article_text
AFTER UPDATE OF title, author, body, url, body_origin ON articles
WHEN OLD.title != NEW.title OR OLD.author != NEW.author
    OR OLD.body != NEW.body OR OLD.url != NEW.url
    OR OLD.body_origin != NEW.body_origin
BEGIN
    INSERT INTO recommendation_dirty (article_id, analyze)
    VALUES (NEW.id, 1)
    ON CONFLICT (article_id) DO UPDATE
    SET analyze = max(recommendation_dirty.analyze, excluded.analyze);
END;

CREATE TRIGGER recommendation_article_state
AFTER UPDATE OF unread, starred, deleted, reading_minutes ON articles
WHEN OLD.unread != NEW.unread OR OLD.starred != NEW.starred
    OR OLD.deleted != NEW.deleted OR OLD.reading_minutes != NEW.reading_minutes
BEGIN
    INSERT INTO recommendation_dirty (article_id, analyze)
    VALUES (NEW.id, 0)
    ON CONFLICT (article_id) DO UPDATE
    SET analyze = max(recommendation_dirty.analyze, excluded.analyze);
END;

CREATE TRIGGER recommendation_full_insert
AFTER INSERT ON article_documents
BEGIN
    INSERT INTO recommendation_dirty (article_id, analyze)
    VALUES (NEW.article_id, 1)
    ON CONFLICT (article_id) DO UPDATE
    SET analyze = max(recommendation_dirty.analyze, excluded.analyze);
END;

CREATE TRIGGER recommendation_full_update
AFTER UPDATE OF body, url ON article_documents
WHEN OLD.body != NEW.body OR OLD.url != NEW.url
BEGIN
    INSERT INTO recommendation_dirty (article_id, analyze)
    VALUES (NEW.article_id, 1)
    ON CONFLICT (article_id) DO UPDATE
    SET analyze = max(recommendation_dirty.analyze, excluded.analyze);
END;

CREATE TRIGGER recommendation_full_delete
AFTER DELETE ON article_documents
BEGIN
    INSERT INTO recommendation_dirty (article_id, analyze)
    SELECT id, 1 FROM articles WHERE id = OLD.article_id
    ON CONFLICT (article_id) DO UPDATE
    SET analyze = max(recommendation_dirty.analyze, excluded.analyze);
END;

CREATE TRIGGER recommendation_tag_insert
AFTER INSERT ON article_tags
BEGIN
    INSERT INTO recommendation_dirty (article_id, analyze)
    VALUES (NEW.article_id, 1)
    ON CONFLICT (article_id) DO UPDATE
    SET analyze = max(recommendation_dirty.analyze, excluded.analyze);
END;

CREATE TRIGGER recommendation_tag_delete
AFTER DELETE ON article_tags
BEGIN
    INSERT INTO recommendation_dirty (article_id, analyze)
    SELECT id, 1 FROM articles WHERE id = OLD.article_id
    ON CONFLICT (article_id) DO UPDATE
    SET analyze = max(recommendation_dirty.analyze, excluded.analyze);
END;

CREATE TRIGGER recommendation_feed_title
AFTER UPDATE OF title ON feeds
WHEN OLD.title != NEW.title
BEGIN
    INSERT INTO recommendation_dirty (article_id, analyze)
    SELECT id, 1 FROM articles WHERE feed_id = NEW.id
    ON CONFLICT (article_id) DO UPDATE SET analyze = 1;
END;

PRAGMA user_version = 10;
