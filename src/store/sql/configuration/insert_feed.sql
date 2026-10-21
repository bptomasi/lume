INSERT INTO feeds (url, title, enabled, article_selector)
VALUES (?1, ?1, ?2, ?3)
ON CONFLICT (url) DO UPDATE
SET enabled = excluded.enabled,
    article_selector = excluded.article_selector
WHERE feeds.enabled <> excluded.enabled
    OR feeds.article_selector <> excluded.article_selector;
