SELECT coalesce(sum(octet_length(c3)), 0), coalesce(sum(octet_length(c4)), 0)
FROM article_search_content;
