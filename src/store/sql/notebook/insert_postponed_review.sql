INSERT INTO annotation_reviews (
    comment_id,
    due_at,
    interval_days,
    reviews
)
VALUES (?1, ?2, ?3, 0)
ON CONFLICT (comment_id) DO UPDATE SET
    due_at = excluded.due_at,
    interval_days = excluded.interval_days;
