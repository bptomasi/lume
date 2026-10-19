SELECT
    s.id,
    s.comment_id,
    s.question,
    s.answer,
    s.due_at,
    s.interval_days,
    s.reviews
FROM study_cards AS s
JOIN comments AS c ON c.id = s.comment_id
JOIN articles AS a ON a.id = c.article_id
JOIN feeds AS f ON f.id = a.feed_id
JOIN note_metadata AS m ON m.comment_id = c.id
WHERE a.deleted = 0
