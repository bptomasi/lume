-- A logical sequence preserves explicit submission order within one second.
INSERT INTO note_search_history (query, sequence)
VALUES (?1, (SELECT coalesce(max(sequence), 0) + 1 FROM note_search_history))
ON CONFLICT (query) DO UPDATE SET sequence = excluded.sequence;
