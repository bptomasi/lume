SELECT body FROM articles
UNION ALL
SELECT body FROM article_documents WHERE body != '';
