{}
            WHERE a.deleted = 0 AND c.id IN (
                SELECT {related} FROM note_references WHERE {owner} = ?1
            )
            ORDER BY c.id
            LIMIT 128