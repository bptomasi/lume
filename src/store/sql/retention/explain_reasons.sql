SELECT reason FROM protected WHERE article_id = ?1
UNION SELECT 'matches a protected smart list' FROM temp.retention_smart WHERE article_id = ?1;
