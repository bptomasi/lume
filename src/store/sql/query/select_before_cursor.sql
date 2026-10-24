AND (a.published, a.id) > (:cursor_date, :cursor_id)
ORDER BY a.published ASC, a.id ASC LIMIT :limit
