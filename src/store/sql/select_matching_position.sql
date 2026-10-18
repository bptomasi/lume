SELECT (
    SELECT count(*)
    FROM articles AS a
    JOIN matching AS candidate ON candidate.id = a.id
    WHERE a.published > target.published
        OR (a.published = target.published AND a.id > target.id)
)
FROM articles AS target
JOIN matching AS selected ON selected.id = target.id
WHERE target.id = :anchor;
