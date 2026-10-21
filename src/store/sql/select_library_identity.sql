SELECT
    feeds.id, feeds.url, feeds.title, feeds.etag, feeds.modified,
    feeds.error, feeds.enabled,
    articles.id, articles.feed_id, articles.external_id, articles.title,
    articles.url, articles.author, articles.published, articles.body,
    articles.unread, articles.starred,
    collections.id, collections.name,
    members.collection_id, members.article_id,
    comments.id, comments.article_id, comments.body, comments.created
FROM feeds, articles, collections, members, comments
LIMIT 0;
