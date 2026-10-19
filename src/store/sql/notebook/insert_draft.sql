INSERT INTO note_drafts (key, target, body, baseline, initial_kind)
VALUES (?1, ?2, ?3, ?4, ?5)
ON CONFLICT (key) DO UPDATE SET
    body = excluded.body,
    baseline = excluded.baseline,
    initial_kind = excluded.initial_kind,
    updated_at = unixepoch();
