SELECT
    id,
    comment_id,
    question,
    answer,
    due_at,
    interval_days,
    reviews
FROM study_cards
WHERE comment_id = ?1
ORDER BY id;
