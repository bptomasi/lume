SELECT c.id
FROM note_identities AS i
LEFT JOIN comments AS c ON c.id = i.id
WHERE i.id = ?1;
