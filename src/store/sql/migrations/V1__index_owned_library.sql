CREATE INDEX IF NOT EXISTS articles_feed
    ON articles(feed_id, published DESC, id DESC);

CREATE INDEX IF NOT EXISTS members_article
    ON members(article_id, collection_id);
