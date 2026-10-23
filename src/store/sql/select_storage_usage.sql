SELECT
    (SELECT count(*) FROM articles),
    (SELECT coalesce(sum(octet_length(body)), 0) FROM articles),
    (SELECT coalesce(sum(octet_length(body)), 0)
        FROM article_documents),
    (SELECT coalesce(sum(octet_length(body)), 0) FROM comments),
    (SELECT count(*) FROM comments),
    (SELECT count(*) FROM comments WHERE quote != '');
