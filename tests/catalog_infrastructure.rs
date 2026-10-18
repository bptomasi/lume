//! Curated infrastructure groups and topic queries use real storage contracts.

use anyhow::{Result, anyhow};
use lume::{
    catalog,
    model::{ArticleQuery, Feed, IncomingArticle, SmartFilter, View},
    store::Store,
};

fn installed() -> Result<(tempfile::TempDir, Store)> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("infrastructure.db"))?;
    catalog::install(&store)?;
    Ok((directory, store))
}

fn query(view: View) -> ArticleQuery {
    ArticleQuery {
        view,
        text: String::new(),
        offset: 0,
        limit: 100,
        retained: None,
    }
}

fn publish(store: &mut Store, feed: &Feed, body: &str) -> Result<()> {
    store.ingest(
        feed,
        &feed.title,
        &[IncomingArticle {
            body_origin: lume::model::FeedOrigin::Feed,
            external_id: "original-scenario".into(),
            title: "A new mechanism".into(),
            url: String::new(),
            author: String::new(),
            published: 2_000_000_000,
            body: body.into(),
        }],
    )?;
    Ok(())
}

#[test]
fn infrastructure_asset_adds_distinct_primary_sources() -> Result<()> {
    let sources = catalog::infrastructure_feeds()?;
    assert_eq!(sources.len(), 12);
    for url in [
        "https://backstage.io/blog/rss.xml",
        "https://blog.crossplane.io/rss/",
        "https://fluxcd.io/blog/index.xml",
        "https://tekton.dev/blog/index.xml",
        "https://opencost.io/blog/rss.xml",
        "https://www.flatcar.org/blog/index.xml",
        "https://bootlin.com/feed/",
        "https://people.kernel.org/read/feed/",
        "https://iximiuz.com/feed.atom",
        "https://ebpf.io/blog/rss.xml",
        "https://blog.apnic.net/feed/",
        "https://blog.ipspace.net/atom.xml",
    ] {
        assert!(sources.iter().any(|feed| feed.url == url), "Missing {url}");
    }
    Ok(())
}

#[test]
fn source_groups_include_future_posts_and_exclude_unrelated_sources()
-> Result<()> {
    let (_directory, mut store) = installed()?;
    for feed in store.feeds()? {
        publish(&mut store, &feed, "No topic keywords are needed.")?;
    }
    for (name, count, representative) in [
        (catalog::PLATFORM_LIST, 8, "Crossplane"),
        (catalog::KERNEL_LIST, 7, "Bootlin"),
        (catalog::NETWORK_LIST, 8, "Cilium"),
        (catalog::DEVOPS_LIST, 8, "Tekton"),
    ] {
        let list = store
            .smart_lists()?
            .into_iter()
            .find(|list| list.name == name)
            .ok_or_else(|| anyhow!("Missing source group {name}"))?;
        let page = store.query_articles(&query(View::Smart(list.id)))?;
        assert_eq!(page.total, count, "Incorrect membership for {name}");
        assert!(page.articles.iter().any(|a| a.feed == representative));
        assert!(page.articles.iter().all(|a| {
            a.feed != "OpenAI" && a.feed != "Google Project Zero"
        }));
    }
    Ok(())
}

#[test]
fn focused_infrastructure_topics_match_without_a_source_restriction()
-> Result<()> {
    let (_directory, mut store) = installed()?;
    let feed = store.feeds()?.remove(0);
    publish(
        &mut store,
        &feed,
        "eBPF, Cilium, CNI and network policy. Container runtimes, Terraform, \
         kernel programming, BGP, controllers, filesystems, schedulers and \
         software supply chain engineering.",
    )?;
    let topics = store
        .smart_lists()?
        .into_iter()
        .filter(|list| list.name.get(..2).is_some_and(|prefix| prefix >= "35"));
    let mut matched = 0;
    for topic in topics {
        let page = store.query_articles(&query(View::Smart(topic.id)))?;
        assert_eq!(page.total, 1, "Topic did not match: {}", topic.name);
        matched += 1;
    }
    assert_eq!(matched, 12);
    Ok(())
}

#[test]
fn custom_group_criteria_survive_reinstallation_and_combine_with_membership()
-> Result<()> {
    let (_directory, mut store) = installed()?;
    let list = store
        .smart_lists()?
        .into_iter()
        .find(|list| list.name == catalog::NETWORK_LIST)
        .ok_or_else(|| anyhow!("Missing networking group"))?;
    let filter = SmartFilter {
        text: "bgp".into(),
        unread: true,
        ..list.filter
    };
    store.save_smart_list(&list.name, &filter)?;
    let feeds = store.feeds()?;
    for feed in &feeds {
        let body = if feed.url == "https://blog.ipspace.net/atom.xml" {
            "Unrelated to the selected term."
        } else {
            "BGP routing research."
        };
        publish(&mut store, feed, body)?;
    }

    let report = catalog::install(&store)?;
    let page = store.query_articles(&query(View::Smart(list.id)))?;

    assert_eq!(report.added_searches, 0);
    assert_eq!(page.total, 7);
    for article in page.articles {
        store.set_unread(article.id, false)?;
    }
    assert_eq!(store.query_articles(&query(View::Smart(list.id)))?.total, 0);
    let saved = store.smart_lists()?.into_iter().find(|s| s.id == list.id);
    assert_eq!(saved.map(|s| s.filter), Some(filter));
    Ok(())
}
