WITH RECURSIVE sequence (id) AS (
    VALUES (1)
    UNION ALL
    SELECT id + 1 FROM sequence WHERE id < ?1
)
INSERT INTO comments (article_id, body, quote, anchor, created)
SELECT
    (id - 1) % ?2 + 1,
    CASE WHEN id % 10 = 0
        THEN 'Quorum consensus evidence: one transaction owner preserves '
            || 'ordering across retries. Compare the failure mechanism '
            || 'with the implementation before changing the design.'
        ELSE 'Networking evidence: bounded queues preserve responsiveness '
            || 'while source processing and disk work continue independently.'
    END,
    'A durable mechanism needs one explicit owner.',
    0,
    2000000000 + id
FROM sequence;
