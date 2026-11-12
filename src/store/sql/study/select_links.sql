
        SELECT article_id
        FROM note_links
        WHERE comment_id = ?1
        ORDER BY article_id
