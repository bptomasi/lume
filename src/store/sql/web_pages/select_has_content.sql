
        SELECT EXISTS (
            SELECT 1
            FROM article_documents
            WHERE article_id = ?1
                AND url = ?2
                AND body != ''
        )
