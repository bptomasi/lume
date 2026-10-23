UPDATE configuration_bindings SET present = 0
WHERE kind = 'server' AND entity = ?1;
