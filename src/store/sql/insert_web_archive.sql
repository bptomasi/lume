INSERT INTO feeds (url, title, enabled, article_policy)
VALUES ('https://lume.invalid/saved-pages', 'Saved pages', 0, 'manual')
ON CONFLICT (url) DO UPDATE SET enabled = 0, article_policy = 'manual';

INSERT INTO manual_sources (feed_id)
SELECT id
FROM feeds
WHERE url = 'https://lume.invalid/saved-pages'
ON CONFLICT (feed_id) DO NOTHING;
