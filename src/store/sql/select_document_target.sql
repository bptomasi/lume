SELECT EXISTS (
    SELECT 1
    FROM articles
    WHERE id = ?1
        AND url = ?2
        AND deleted = 0
);
