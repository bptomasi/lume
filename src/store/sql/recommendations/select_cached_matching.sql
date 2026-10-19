SELECT a.id, recommended.position
FROM recommendation_items AS recommended
CROSS JOIN articles AS a ON a.id = recommended.article_id
WHERE a.deleted = 0
