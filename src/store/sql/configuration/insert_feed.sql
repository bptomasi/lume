INSERT INTO feeds (url, title, enabled, article_selector, article_policy)
VALUES (?1, ?1, ?2, ?3, ?4)
ON CONFLICT (url) DO UPDATE
SET enabled = excluded.enabled,
    article_selector = excluded.article_selector,
    article_policy = excluded.article_policy
WHERE feeds.enabled <> excluded.enabled
    OR feeds.article_selector <> excluded.article_selector
    OR feeds.article_policy <> excluded.article_policy;
