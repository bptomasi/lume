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
WHERE a.deleted = 0 AND s.due_at <= ?1
ORDER BY s.due_at, s.id
LIMIT 200;
