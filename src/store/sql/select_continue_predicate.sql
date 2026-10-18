a.progress > 0 OR EXISTS (
    SELECT 1
    FROM article_documents AS d
    WHERE d.article_id = a.id
        AND d.url = a.url
        AND d.progress > 0
)
