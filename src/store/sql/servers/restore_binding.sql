UPDATE configuration_bindings SET present = 1
WHERE kind = 'server' AND entity = ?1;
