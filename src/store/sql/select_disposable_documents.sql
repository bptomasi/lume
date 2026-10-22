SELECT d.article_id, length(CAST(d.body AS BLOB)), a.body
FROM article_documents AS d
JOIN articles AS a ON a.id = d.article_id
WHERE d.article_id > ?1
    AND d.article_id <= ?2
    AND d.body != ''
    AND a.starred = 0
    AND a.queued = 0
    AND NOT EXISTS (
        SELECT 1 FROM comments WHERE article_id = a.id
    )
    AND NOT EXISTS (
        SELECT 1
        FROM note_drafts AS draft, json_each(draft.target) AS captured
        WHERE json_extract(captured.value, '$.article') = a.id
    )
    AND NOT EXISTS (
        SELECT 1 FROM members WHERE article_id = a.id
    )
    AND NOT EXISTS (
        SELECT 1
        FROM settings AS s, json_each(s.value, '$.items') AS item
        WHERE s.key = 'offline_package'
            AND json_extract(item.value, '$.article') = a.id
    )
    AND NOT EXISTS (
        SELECT 1
        FROM settings AS s, json_each(s.value, '$.items') AS item
        WHERE s.key = 'reading_session'
            AND CAST(item.key AS INTEGER) >= json_extract(s.value, '$.position')
            AND json_extract(item.value, '$.article') = a.id
    )
ORDER BY d.article_id
LIMIT 16;
