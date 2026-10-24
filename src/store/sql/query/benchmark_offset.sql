SELECT a.id FROM articles AS a
JOIN candidates AS candidate ON candidate.id = a.id
ORDER BY a.published DESC, a.id DESC LIMIT 100 OFFSET ?1
