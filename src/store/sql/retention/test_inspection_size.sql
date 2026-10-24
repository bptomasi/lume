SELECT (SELECT count(*) FROM temp.retention_sources),
       (SELECT count(*) FROM temp.retention_candidates),
       (SELECT count(*) FROM temp.retention_smart);
