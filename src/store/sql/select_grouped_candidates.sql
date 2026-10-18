SELECT id
FROM (
    SELECT
        a.id,
        row_number() OVER (
            PARTITION BY
                a.canonical_key,
                CASE WHEN a.canonical_key = '' THEN a.id ELSE 0 END
            ORDER BY a.published DESC, a.id DESC
        ) AS occurrence
    FROM articles AS a
    JOIN candidates AS candidate ON candidate.id = a.id
)
WHERE occurrence = 1
