SELECT a.id
FROM articles AS a
WHERE a.deleted = CASE WHEN :scope = 'trash' THEN 1 ELSE 0 END
