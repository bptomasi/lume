SELECT q.article_id, q.generation, a.url,
    CASE WHEN instr(a.body, '![') > 0 THEN a.body ELSE '' END,
    coalesce(d.final_url, a.url),
    CASE WHEN instr(d.body, '![') > 0 THEN d.body ELSE '' END
FROM image_reference_dirty AS q
JOIN articles AS a ON a.id = q.article_id
LEFT JOIN article_documents AS d ON d.article_id = q.article_id
WHERE q.dirty = 1
ORDER BY q.article_id LIMIT 16;
