SELECT b.key, s.name, s.definition
FROM smart_lists AS s
JOIN configuration_bindings AS b ON b.kind = 'smart' AND b.entity = s.id
WHERE s.enabled = 1 AND b.present = 1
ORDER BY b.key;
