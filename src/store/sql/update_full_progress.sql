UPDATE article_documents SET progress = ?1
WHERE article_id = ?2 AND revision = ?3 AND progress IS NOT ?1
    AND url = (SELECT url FROM articles WHERE id = ?2);
