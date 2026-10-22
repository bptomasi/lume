CREATE TABLE manual_sources (
    feed_id INTEGER PRIMARY KEY REFERENCES feeds(id),
    CHECK (feed_id > 0)
);

CREATE TRIGGER manual_sources_keep_collection_disabled
BEFORE UPDATE OF enabled ON feeds
WHEN NEW.enabled != 0
    AND EXISTS (
        SELECT 1 FROM manual_sources WHERE feed_id = NEW.id
    )
BEGIN
    SELECT RAISE(ABORT, 'Saved pages are not a network subscription');
END;

PRAGMA user_version = 14;
