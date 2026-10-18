SELECT
    id,
    title,
    attempted_at,
    succeeded_at,
    retry_at,
    failures,
    error,
    article_selector,
    url
FROM feeds
WHERE enabled = 1
ORDER BY (error <> '') DESC, title COLLATE NOCASE, id;
