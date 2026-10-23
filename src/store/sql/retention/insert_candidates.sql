INSERT INTO temp.retention_candidates
SELECT a.id,
    CASE WHEN a.unread = 0 THEN
        p.read_cutoff > 0 AND max(a.added_at, a.reading_state_at, CASE
            WHEN a.reading_state_at = 0 AND a.id <= clock.legacy_upper_id
            THEN clock.legacy_state_at ELSE 0 END) > 0
        AND max(a.added_at, a.reading_state_at, CASE
            WHEN a.reading_state_at = 0 AND a.id <= clock.legacy_upper_id
            THEN clock.legacy_state_at ELSE 0 END) < p.read_cutoff
    ELSE p.unread_cutoff > 0 AND max(a.added_at, a.reading_state_at, CASE
        WHEN a.reading_state_at = 0 AND a.id <= clock.legacy_upper_id
        THEN clock.legacy_state_at ELSE 0 END) > 0
        AND max(a.added_at, a.reading_state_at, CASE
        WHEN a.reading_state_at = 0 AND a.id <= clock.legacy_upper_id
        THEN clock.legacy_state_at ELSE 0 END) < p.unread_cutoff END,
    p.full_cutoff > 0 AND coalesce(d.body, '') <> '' AND d.fetched_at > 0
        AND d.fetched_at < p.full_cutoff,
    p.image_cutoff > 0 AND a.added_at > 0 AND a.added_at < p.image_cutoff,
    p.image_cutoff
FROM articles AS a
CROSS JOIN article_identity_clock AS clock
JOIN temp.retention_sources AS p ON p.feed_id = a.feed_id
LEFT JOIN article_documents AS d ON d.article_id = a.id
WHERE a.id > :after AND a.id <= :upper
    AND NOT EXISTS (SELECT 1 FROM protected WHERE article_id = a.id)
    AND NOT EXISTS (SELECT 1 FROM temp.retention_smart WHERE article_id = a.id);
