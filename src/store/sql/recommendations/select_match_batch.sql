SELECT a.id
FROM articles AS a
WHERE a.id IN (SELECT value FROM json_each(:ids))
