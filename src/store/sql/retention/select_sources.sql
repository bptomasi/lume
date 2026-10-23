SELECT f.id, b.key, sb.key, sf.remote_id
FROM feeds AS f
LEFT JOIN configuration_bindings AS b ON b.kind = 'feed' AND b.entity = f.id
LEFT JOIN server_feeds AS sf ON sf.feed_id = f.id
LEFT JOIN configuration_bindings AS sb ON sb.kind = 'server' AND sb.entity = sf.server_id;
