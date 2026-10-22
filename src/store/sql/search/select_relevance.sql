-- Unary + prevents FTS5 from reopening MATCH for each candidate identity.
-- Membership is still checked, but each ranked text expression scans once.
WITH scores AS MATERIALIZED (
    SELECT rowid AS id,
        bm25(article_search, 16.0, 2.0, 2.0, 1.0, 0.5) AS relevance
    FROM article_search
    WHERE article_search MATCH :rank_text
        AND +rowid IN (SELECT id FROM candidates)
),
prefixes AS MATERIALIZED (
    SELECT rowid AS id FROM article_search
    WHERE article_search MATCH coalesce(:rank_prefix, 'title : ""')
        AND +rowid IN (SELECT id FROM candidates)
),
phrases AS MATERIALIZED (
    SELECT rowid AS id FROM article_search
    WHERE article_search MATCH coalesce(:rank_phrase, 'title : ""')
        AND +rowid IN (SELECT id FROM candidates)
),
titles AS MATERIALIZED (
    SELECT rowid AS id FROM article_search
    WHERE article_search MATCH coalesce(:rank_title, 'title : ""')
        AND +rowid IN (SELECT id FROM candidates)
)
SELECT a.id, a.published,
    CASE
        WHEN a.id IN (SELECT id FROM prefixes) THEN 0
        WHEN a.id IN (SELECT id FROM phrases) THEN 1
        WHEN a.id IN (SELECT id FROM titles) THEN 2
        ELSE 3
    END AS title_rank,
    coalesce(scores.relevance, 0.0) AS relevance
FROM candidates AS candidate
JOIN articles AS a ON a.id = candidate.id
LEFT JOIN scores ON scores.id = a.id
