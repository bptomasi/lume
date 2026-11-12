
            SELECT EXISTS (
                SELECT 1 FROM configuration_bindings
                WHERE kind = ?1 AND entity = ?2
            )
