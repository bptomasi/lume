UPDATE settings SET value = json_set(value, '$.article', NULL)
WHERE key = 'workspace'
    AND json_extract(value, '$.article') IN (SELECT id FROM temp.purge_articles);

-- Keep the reading cursor at the same surviving item, including completed plans.
UPDATE settings SET value = json_set(value,
    '$.position', (
        SELECT count(*) FROM json_each(settings.value, '$.items') AS item
        WHERE CAST(item.key AS INTEGER) < json_extract(settings.value, '$.position')
            AND json_extract(item.value, '$.article')
                NOT IN (SELECT id FROM temp.purge_articles)
    ),
    '$.items', json((
        SELECT json_group_array(json(item.value) ORDER BY CAST(item.key AS INTEGER))
        FROM json_each(settings.value, '$.items') AS item
        WHERE json_extract(item.value, '$.article')
            NOT IN (SELECT id FROM temp.purge_articles)
    )))
WHERE key = 'reading_session' AND EXISTS (
    SELECT 1 FROM json_each(settings.value, '$.items') AS item
    WHERE json_extract(item.value, '$.article') IN (SELECT id FROM temp.purge_articles)
);

UPDATE settings SET value = json_set(value, '$.items', json((
    SELECT json_group_array(json(item.value) ORDER BY CAST(item.key AS INTEGER))
    FROM json_each(settings.value, '$.items') AS item
    WHERE json_extract(item.value, '$.article')
        NOT IN (SELECT id FROM temp.purge_articles)
)))
WHERE key = 'offline_package' AND EXISTS (
    SELECT 1 FROM json_each(settings.value, '$.items') AS item
    WHERE json_extract(item.value, '$.article') IN (SELECT id FROM temp.purge_articles)
);
