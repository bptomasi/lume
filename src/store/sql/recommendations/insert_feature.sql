INSERT INTO recommendation_features (
    article_id, score, minutes, evidence, excluded
)
VALUES (?1, ?2, ?3, ?4, ?5)
ON CONFLICT (article_id) DO UPDATE
SET
    score = excluded.score,
    minutes = excluded.minutes,
    evidence = excluded.evidence,
    excluded = excluded.excluded;
