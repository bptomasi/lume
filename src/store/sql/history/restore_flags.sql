
        UPDATE articles
        SET unread = ?1, starred = ?2, queued = ?3
        WHERE id = ?4
