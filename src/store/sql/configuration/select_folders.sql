SELECT b.key, c.name
FROM collections AS c
JOIN configuration_bindings AS b ON b.kind = 'folder' AND b.entity = c.id
WHERE c.enabled = 1 AND b.present = 1
ORDER BY b.key;
