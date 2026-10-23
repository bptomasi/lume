CREATE TABLE image_references (
    article_id INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    source INTEGER NOT NULL CHECK(source IN (0, 1)),
    url TEXT NOT NULL,
    PRIMARY KEY(article_id, source, url)
);
CREATE INDEX image_references_url ON image_references(url, article_id);
CREATE TABLE image_reference_dirty (
    article_id INTEGER PRIMARY KEY REFERENCES articles(id) ON DELETE CASCADE,
    generation INTEGER NOT NULL DEFAULT 1,
    dirty INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX image_reference_pending ON image_reference_dirty(article_id) WHERE dirty = 1;
INSERT INTO image_reference_dirty(article_id) SELECT id FROM articles;
CREATE TRIGGER image_references_article_insert AFTER INSERT ON articles BEGIN
    INSERT OR IGNORE INTO image_reference_dirty(article_id) VALUES (NEW.id);
END;
CREATE TRIGGER image_references_article_update AFTER UPDATE OF body, url ON articles
WHEN NEW.body IS NOT OLD.body OR NEW.url IS NOT OLD.url BEGIN
    INSERT INTO image_reference_dirty(article_id) VALUES (NEW.id)
    ON CONFLICT(article_id) DO UPDATE SET generation = generation + 1, dirty = 1;
END;
CREATE TRIGGER image_references_full_insert AFTER INSERT ON article_documents BEGIN
    INSERT INTO image_reference_dirty(article_id) VALUES (NEW.article_id)
    ON CONFLICT(article_id) DO UPDATE SET generation = generation + 1, dirty = 1;
END;
CREATE TRIGGER image_references_full_update AFTER UPDATE OF body, final_url ON article_documents
WHEN NEW.body IS NOT OLD.body OR NEW.final_url IS NOT OLD.final_url BEGIN
    INSERT INTO image_reference_dirty(article_id) VALUES (NEW.article_id)
    ON CONFLICT(article_id) DO UPDATE SET generation = generation + 1, dirty = 1;
END;
CREATE TRIGGER image_references_full_delete AFTER DELETE ON article_documents
WHEN EXISTS (SELECT 1 FROM articles WHERE id = OLD.article_id) BEGIN
    DELETE FROM image_references WHERE article_id = OLD.article_id AND source = 1;
    INSERT INTO image_reference_dirty(article_id) VALUES (OLD.article_id)
    ON CONFLICT(article_id) DO UPDATE SET generation = generation + 1, dirty = 1;
END;
PRAGMA user_version = 22;
