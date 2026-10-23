SELECT EXISTS (
    SELECT 1 FROM articles
    WHERE feed_id = ?1 AND external_id = ?2
        AND title IS ?3 AND url IS ?4 AND author IS ?5
        AND published IS ?6 AND body IS ?7 AND body_origin IS ?8
        -- Legacy rows with unknown canonical identity still need derivation.
        AND (url = '' OR canonical_key <> '')
);
