DELETE FROM note_search_history
WHERE query NOT IN (
    SELECT query
    FROM note_search_history
    ORDER BY sequence DESC, query
    LIMIT 30
);
