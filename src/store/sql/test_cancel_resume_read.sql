WITH RECURSIVE sequence(value) AS (
    VALUES(0) UNION ALL SELECT value + 1 FROM sequence WHERE value < 9999
) SELECT sum(value) FROM sequence;
