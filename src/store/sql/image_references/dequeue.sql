UPDATE image_reference_dirty SET dirty = 0
WHERE article_id = ?1 AND generation = ?2 AND dirty = 1;
