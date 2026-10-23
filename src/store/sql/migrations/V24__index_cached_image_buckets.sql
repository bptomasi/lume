-- A short digest prefix bounds ordinary-cache lookups without storing another
-- complete URL/hash index. Callers always recheck the complete SHA-256 path.
ALTER TABLE image_references ADD COLUMN cache_bucket TEXT NOT NULL DEFAULT '';
CREATE INDEX image_references_bucket ON image_references(cache_bucket, article_id);
-- Existing references rebuild lazily with generation checks, never at startup.
UPDATE image_reference_dirty SET generation = generation + 1, dirty = 1
WHERE EXISTS (SELECT 1 FROM image_references AS r
    WHERE r.article_id = image_reference_dirty.article_id);
PRAGMA user_version = 24;
