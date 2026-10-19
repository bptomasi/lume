SELECT target, body, baseline, initial_kind
FROM note_drafts
ORDER BY updated_at DESC, key
LIMIT 64;
