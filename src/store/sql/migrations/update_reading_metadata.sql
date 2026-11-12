
            UPDATE articles
            SET reading_minutes = ?1,
                canonical_key = ?2
            WHERE id = ?3
