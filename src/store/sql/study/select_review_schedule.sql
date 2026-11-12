
        SELECT due_at, interval_days, reviews
        FROM annotation_reviews
        WHERE comment_id = ?1
