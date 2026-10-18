SELECT (
    SELECT count(*)
    FROM articles AS newer
    WHERE newer.deleted = 0
        AND (
            newer.published > target.published
            OR (
                newer.published = target.published
                AND newer.id > target.id
            )
        )
)
FROM articles AS target
WHERE target.id = ?1
    AND target.deleted = 0
