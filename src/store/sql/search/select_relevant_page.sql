{candidates},
matching AS MATERIALIZED (
    SELECT ranked.* FROM scored AS ranked
    {order}
    LIMIT :limit OFFSET :offset
)
{metadata}
JOIN matching AS ranked ON ranked.id = a.id
{order}
