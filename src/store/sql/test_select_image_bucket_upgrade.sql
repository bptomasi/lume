SELECT r.cache_bucket, q.dirty, q.generation
FROM image_references AS r JOIN image_reference_dirty AS q ON q.article_id = r.article_id
WHERE r.article_id = 1 AND r.source = 0;
