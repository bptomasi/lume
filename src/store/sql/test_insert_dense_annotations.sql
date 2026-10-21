WITH RECURSIVE sequence(value) AS (
    VALUES (1)
    UNION ALL
    SELECT value + 1
    FROM sequence
    WHERE value < ?2
)
INSERT INTO comments (
    article_id,
    body,
    quote,
    anchor,
    source,
    source_revision
)
SELECT
    ?1,
    'A retained private observation.',
    CASE value % 3
        WHEN 0 THEN 'Unique stable quotation'
        WHEN 1 THEN 'Ambiguous quotation'
        ELSE 'A quotation removed by the publisher'
    END,
    0,
    'full',
    1
FROM sequence;
