SELECT f.id, b.key, f.url, f.enabled, f.article_selector
FROM feeds AS f
JOIN configuration_bindings AS b ON b.kind = 'feed' AND b.entity = f.id
WHERE b.present = 1
ORDER BY b.key;
