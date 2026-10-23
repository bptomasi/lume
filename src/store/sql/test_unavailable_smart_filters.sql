-- A cached sidebar must remain usable even while a catalog read would fail.
UPDATE smart_lists SET definition = '{unavailable';
