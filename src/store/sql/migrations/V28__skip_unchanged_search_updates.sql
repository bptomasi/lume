-- Metadata-only refreshes must not rebuild potentially large FTS documents.
-- Existing indexed bytes remain authoritative; this migration changes no data.
DROP TRIGGER articles_search_update;
CREATE TRIGGER articles_search_update
AFTER UPDATE OF title, author, body, url ON articles
WHEN NEW.title IS NOT OLD.title OR NEW.author IS NOT OLD.author
    OR NEW.body IS NOT OLD.body OR NEW.url IS NOT OLD.url
BEGIN
    UPDATE article_search SET title = NEW.title, author = NEW.author,
        body = NEW.body || char(10) || coalesce((
            SELECT d.body FROM article_documents AS d
            WHERE d.article_id = NEW.id AND d.url = NEW.url
        ), '')
    WHERE rowid = NEW.id;
END;

DROP TRIGGER feeds_search_update;
CREATE TRIGGER feeds_search_update AFTER UPDATE OF title ON feeds
WHEN NEW.title IS NOT OLD.title
BEGIN
    UPDATE article_search SET feed = NEW.title
    WHERE rowid IN (SELECT id FROM articles WHERE feed_id = NEW.id);
END;

DROP TRIGGER article_documents_search_update;
CREATE TRIGGER article_documents_search_update
AFTER UPDATE OF body, url ON article_documents
WHEN NEW.body IS NOT OLD.body OR NEW.url IS NOT OLD.url
BEGIN
    UPDATE article_search SET body = (
        SELECT a.body || char(10) || CASE
            WHEN a.url = NEW.url THEN NEW.body ELSE '' END
        FROM articles AS a WHERE a.id = NEW.article_id
    ) WHERE rowid = NEW.article_id;
END;
PRAGMA user_version = 28;
