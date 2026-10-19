SELECT
    rowid,
    snippet(note_search, -1, char(30), char(31), '…', 24)
FROM note_search
WHERE note_search MATCH ?1
