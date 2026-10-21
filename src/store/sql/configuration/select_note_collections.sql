SELECT b.key, c.name, c.query
FROM note_collections AS c
JOIN configuration_bindings AS b ON b.kind = 'notes' AND b.entity = c.id
WHERE c.enabled = 1 AND b.present = 1
ORDER BY b.key;
