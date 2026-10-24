WITH matching AS MATERIALIZED ({matching})
UPDATE articles SET deleted = 1
WHERE deleted = 0 AND id IN (SELECT id FROM matching)
RETURNING id;
