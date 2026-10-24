AND (a.published, a.id) < (:cursor_date, :cursor_id)
ORDER BY a.published DESC, a.id DESC LIMIT :limit
