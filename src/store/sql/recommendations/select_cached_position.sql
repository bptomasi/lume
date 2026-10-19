SELECT (
    SELECT count(*) FROM matching AS candidate
    WHERE candidate.position < target.position
)
FROM matching AS target
WHERE target.id = :anchor;
