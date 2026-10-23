//! Bounded headline labels survive reader motion; flags/styles stay live.

use chrono::{DateTime, Utc};
use std::collections::VecDeque;

use super::App;
use crate::model::Article;

#[derive(Clone)]
pub(crate) struct Labels {
    pub(crate) first: String,
    pub(crate) second: String,
    pub(crate) date: String,
}

struct Entry {
    id: i64,
    title: String,
    published: i64,
    width: u16,
    labels: Labels,
}

#[derive(Default)]
pub(super) struct Headlines(VecDeque<Entry>);

impl Headlines {
    fn labels(&mut self, article: &Article, width: u16) -> Labels {
        if let Some(entry) = self.0.iter().find(|entry| {
            entry.id == article.id
                && entry.title == article.title
                && entry.published == article.published
                && entry.width == width
        }) {
            return entry.labels.clone();
        }
        let wrapped = textwrap::wrap(
            &article.title,
            usize::from(width.saturating_sub(4).max(1)),
        );
        let labels = Labels {
            first: wrapped.first().map_or("", |line| line.as_ref()).into(),
            second: wrapped.get(1).map_or("", |line| line.as_ref()).into(),
            date: DateTime::<Utc>::from_timestamp(article.published, 0)
                .filter(|_| article.published != 0)
                .map_or_else(
                    || "undated".into(),
                    |date| date.format("%b %d").to_string(),
                ),
        };
        // A hostile oversized metadata field must not expand retained memory.
        if article.title.len() <= 4096 {
            self.0.push_front(Entry {
                id: article.id,
                title: article.title.clone(),
                published: article.published,
                width,
                labels: labels.clone(),
            });
            self.0.truncate(64);
        }
        labels
    }
}

impl App {
    pub(crate) fn headline_labels(
        &self,
        article: &Article,
        width: u16,
    ) -> Labels {
        self.headlines.borrow_mut().labels(article, width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn article() -> Article {
        Article {
            id: 1,
            feed_id: 1,
            title: "First title 界".into(),
            url: "https://journal.example/1".into(),
            feed: "Journal".into(),
            author: "Author".into(),
            published: 0,
            unread: true,
            starred: false,
            comments: 0,
            queued: false,
        }
    }

    #[test]
    fn labels_follow_title_width_and_date_without_retaining_personal_flags() {
        let mut cache = Headlines::default();
        let mut article = article();
        assert_eq!(cache.labels(&article, 80).first, "First title 界");
        assert_eq!(cache.labels(&article, 80).date, "undated");
        assert_eq!(cache.0.len(), 1);
        article.unread = false;
        article.starred = true;
        cache.labels(&article, 80);
        assert_eq!(cache.0.len(), 1);
        let narrow = cache.labels(&article, 10);
        assert_eq!(narrow.first, "First");
        assert_eq!(narrow.second, "title");
        article.title = "Publisher correction".into();
        article.published = 1_577_836_800;
        let changed = cache.labels(&article, 80);
        assert_eq!(changed.first, "Publisher correction");
        assert_eq!(changed.date, "Jan 01");
    }

    #[test]
    fn metadata_cache_has_entry_and_source_size_bounds() {
        let mut cache = Headlines::default();
        let mut article = article();
        for id in 0..100 {
            article.id = id;
            cache.labels(&article, 80);
        }
        assert_eq!(cache.0.len(), 64);
        article.title = "界".repeat(2000);
        cache.labels(&article, 80);
        assert!(cache.0.iter().all(|entry| entry.title.len() <= 4096));
    }
}
