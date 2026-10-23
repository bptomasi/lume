SELECT
    count(*) FILTER (WHERE a.deleted = 0),
    count(*) FILTER (WHERE a.deleted = 0 AND a.unread = 1),
    count(*) FILTER (WHERE a.deleted = 0 AND a.unread = 0),
    count(*) FILTER (WHERE a.deleted = 0 AND a.starred = 1),
    count(*) FILTER (WHERE a.deleted = 0 AND a.queued = 1),
    count(*) FILTER (WHERE a.deleted = 1),
    count(*) FILTER (WHERE :annotated AND a.deleted = 0 AND EXISTS (
        SELECT 1 FROM comments AS c WHERE c.article_id = a.id
    )),
    count(*) FILTER (WHERE :continuing AND a.deleted = 0 AND (a.progress > 0 OR EXISTS (
        SELECT 1 FROM article_documents AS d
        WHERE d.article_id = a.id AND d.url = a.url AND d.progress > 0
    ))),
    count(DISTINCT CASE WHEN :grouped AND a.deleted = 0 THEN
        CASE WHEN a.canonical_key = '' THEN 'id:' || a.id
            ELSE 'url:' || a.canonical_key END END)
FROM articles AS a;
