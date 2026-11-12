
            EXPLAIN QUERY PLAN
            SELECT count(*) FROM articles
            WHERE deleted = 0 AND unread = 1
