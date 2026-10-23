SELECT id AS article_id, 'favorite' AS reason FROM articles WHERE starred = 1
UNION SELECT id, 'read-next queue' FROM articles WHERE queued = 1
UNION SELECT article_id, 'personal folder' FROM members
UNION SELECT article_id, 'annotation or highlight' FROM comments
UNION SELECT article_id, 'linked note' FROM note_links
UNION SELECT article_id, 'article tag' FROM article_tags
UNION SELECT article_id, 'Markdown export' FROM export_manifest
UNION SELECT a.id, 'saved page' FROM articles AS a JOIN manual_sources AS m ON m.feed_id = a.feed_id
UNION SELECT CAST(captured.value AS INTEGER), 'unsaved draft'
    FROM note_drafts AS draft, json_tree(draft.target) AS captured
    WHERE captured.key = 'article' AND captured.type = 'integer'
UNION SELECT CAST(captured.value AS INTEGER), 'undo history'
    FROM undo_log AS undo, json_tree(undo.payload) AS captured
    WHERE captured.key IN ('article', 'Article') AND captured.type = 'integer'
UNION SELECT json_extract(item.value, '$.article'), 'offline package'
    FROM settings AS s, json_each(s.value, '$.items') AS item WHERE s.key = 'offline_package'
UNION SELECT json_extract(item.value, '$.article'), 'active reading session'
    FROM settings AS s, json_each(s.value, '$.items') AS item
    WHERE s.key = 'reading_session'
        AND CAST(item.key AS INTEGER) >= json_extract(s.value, '$.position')
UNION SELECT json_extract(value, '$.article'), 'workspace article' FROM settings
    WHERE key = 'workspace' AND json_extract(value, '$.article') IS NOT NULL;
