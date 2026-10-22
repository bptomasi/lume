{candidates}
SELECT position FROM (
    SELECT ranked.id, row_number() OVER ({order}) - 1 AS position
    FROM scored AS ranked
)
WHERE id = :anchor
