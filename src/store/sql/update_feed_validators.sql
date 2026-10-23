UPDATE feeds SET etag = ?1, modified = ?2, error = ''
WHERE id = ?3 AND (etag IS NOT ?1 OR modified IS NOT ?2 OR error <> '');
