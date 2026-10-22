SELECT
    (SELECT count(*) FROM articles),
    (SELECT coalesce(sum(length(CAST(body AS BLOB))), 0) FROM articles),
    (SELECT coalesce(sum(length(CAST(body AS BLOB))), 0)
        FROM article_documents),
    (SELECT coalesce(sum(length(CAST(body AS BLOB))), 0) FROM comments),
    (SELECT count(*) FROM comments),
    (SELECT count(*) FROM comments WHERE quote != '');
