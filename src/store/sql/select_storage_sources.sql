SELECT f.id, f.title, coalesce(s.url, ''), count(a.id),
    coalesce(sum(octet_length(a.body)), 0), coalesce(sum(octet_length(d.body)), 0)
FROM feeds AS f
LEFT JOIN servers AS s ON s.id = f.server_id
LEFT JOIN articles AS a ON a.feed_id = f.id
LEFT JOIN article_documents AS d ON d.article_id = a.id
GROUP BY f.id
ORDER BY coalesce(sum(octet_length(a.body)), 0) + coalesce(sum(octet_length(d.body)), 0) DESC, f.id;
