EXISTS (
    SELECT 1
    FROM members AS m
    WHERE m.article_id = a.id
        AND m.collection_id = :id
)
