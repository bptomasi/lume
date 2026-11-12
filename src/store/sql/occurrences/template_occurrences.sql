{}
            WHERE a.deleted = 0
                AND a.canonical_key = ?1
            ORDER BY a.published DESC, a.id DESC
            LIMIT 500
