SELECT e.article_id,
    a.title IS ?3 AND a.url IS ?4 AND a.author IS ?5
    AND a.published IS ?6 AND a.body IS ?7 AND a.body_origin IS ?8
FROM server_entries AS e
JOIN articles AS a ON a.id = e.article_id
WHERE e.server_id = ?1 AND e.remote_id = ?2;
