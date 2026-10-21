INSERT INTO configuration_bindings (kind, entity, key)
SELECT 'feed', record.id, 'feed-' || lower(hex(randomblob(12)))
FROM feeds AS record
WHERE NOT EXISTS (
    SELECT 1
    FROM configuration_bindings AS binding
    WHERE binding.kind = 'feed'
        AND binding.entity = record.id
);

INSERT INTO configuration_bindings (kind, entity, key)
SELECT 'folder', record.id, 'folder-' || lower(hex(randomblob(12)))
FROM collections AS record
WHERE NOT EXISTS (
    SELECT 1
    FROM configuration_bindings AS binding
    WHERE binding.kind = 'folder'
        AND binding.entity = record.id
);

INSERT INTO configuration_bindings (kind, entity, key)
SELECT 'smart', record.id, 'smart-' || lower(hex(randomblob(12)))
FROM smart_lists AS record
WHERE NOT EXISTS (
    SELECT 1
    FROM configuration_bindings AS binding
    WHERE binding.kind = 'smart'
        AND binding.entity = record.id
);

INSERT INTO configuration_bindings (kind, entity, key)
SELECT 'notes', record.id, 'notes-' || lower(hex(randomblob(12)))
FROM note_collections AS record
WHERE NOT EXISTS (
    SELECT 1
    FROM configuration_bindings AS binding
    WHERE binding.kind = 'notes'
        AND binding.entity = record.id
);
