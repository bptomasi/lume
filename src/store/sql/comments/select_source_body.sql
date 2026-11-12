
        SELECT d.body
        FROM article_documents AS d
        JOIN articles AS a ON a.id = d.article_id
        WHERE a.id = ?1 AND d.url = a.url AND d.revision = ?2
