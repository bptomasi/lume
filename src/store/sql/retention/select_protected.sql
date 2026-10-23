SELECT id AS article_id FROM articles
WHERE starred = 1 OR queued = 1
UNION SELECT article_id FROM members
UNION SELECT article_id FROM comments
UNION SELECT article_id FROM note_links
UNION SELECT article_id FROM article_tags
UNION SELECT article_id FROM export_manifest
UNION SELECT a.id FROM articles AS a JOIN manual_sources AS m ON m.feed_id = a.feed_id
UNION SELECT CAST(captured.value AS INTEGER)
    FROM note_drafts AS draft, json_tree(draft.target) AS captured
    WHERE captured.key = 'article' AND captured.type = 'integer'
UNION SELECT CAST(captured.value AS INTEGER)
    FROM undo_log AS undo, json_tree(undo.payload) AS captured
    WHERE captured.key IN ('article', 'Article') AND captured.type = 'integer'
UNION SELECT json_extract(item.value, '$.article')
    FROM settings AS s, json_each(s.value, '$.items') AS item
    WHERE s.key = 'offline_package'
UNION SELECT json_extract(item.value, '$.article')
    FROM settings AS s, json_each(s.value, '$.items') AS item
    WHERE s.key = 'reading_session'
        AND CAST(item.key AS INTEGER) >= json_extract(s.value, '$.position')
UNION SELECT json_extract(value, '$.article') FROM settings
    WHERE key = 'workspace' AND json_extract(value, '$.article') IS NOT NULL;
