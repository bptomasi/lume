SELECT c.article_id, c.id
FROM comments AS c
JOIN study_cards AS s ON s.comment_id = c.id
WHERE s.id = ?1;
