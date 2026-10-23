CREATE TEMP TABLE IF NOT EXISTS retention_smart (
    article_id INTEGER PRIMARY KEY
);
CREATE TEMP TABLE IF NOT EXISTS retention_sources (
    feed_id INTEGER PRIMARY KEY,
    full_cutoff INTEGER NOT NULL,
    image_cutoff INTEGER NOT NULL,
    read_cutoff INTEGER NOT NULL,
    unread_cutoff INTEGER NOT NULL
);
CREATE TEMP TABLE IF NOT EXISTS retention_candidates (
    article_id INTEGER PRIMARY KEY,
    remove_article INTEGER NOT NULL,
    clear_full INTEGER NOT NULL,
    expire_images INTEGER NOT NULL,
    image_cutoff INTEGER NOT NULL
);
DELETE FROM retention_smart;
DELETE FROM retention_sources;
DELETE FROM retention_candidates;
