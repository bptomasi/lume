(
    a.unread = 1 OR a.id = :retained
    OR coalesce(json_extract((
        SELECT value FROM settings WHERE key = 'recommendation_profile'
    ), '$.include_read'), 0)
)
