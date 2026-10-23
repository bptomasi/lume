SELECT entity FROM configuration_bindings
WHERE kind = 'smart' AND key IN (SELECT value FROM json_each(?1));
