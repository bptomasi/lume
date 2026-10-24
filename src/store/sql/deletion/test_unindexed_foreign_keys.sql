SELECT s.name, fk."from" FROM sqlite_schema AS s,
    pragma_foreign_key_list(s.name) AS fk
WHERE s.type = 'table'
    AND NOT EXISTS (SELECT 1 FROM pragma_table_info(s.name) AS column
        WHERE column.name = fk."from" AND column.pk = 1)
    AND NOT EXISTS (
        SELECT 1 FROM pragma_index_list(s.name) AS idx,
            pragma_index_info(idx.name) AS column
        WHERE idx.partial = 0 AND column.seqno = 0 AND column.name = fk."from"
    );
