use crate::{
    config::retention::{Age, Policy, Retention},
    model::{ArticleQuery, FeedOrigin, IncomingArticle, SmartFilter, View},
    retention,
    store::Store,
};
use anyhow::Result;
use std::sync::atomic::AtomicBool;

const NOW: i64 = 1_800_000_000;

struct Fixture {
    directory: tempfile::TempDir,
    store: Store,
}

impl Fixture {
    fn publish_image_source(&mut self, body: &str) -> Result<()> {
        let id = self.store.subscribe("https://other.example/feed")?;
        let feed = self
            .store
            .feeds()?
            .into_iter()
            .find(|feed| feed.id == id)
            .ok_or_else(|| anyhow::anyhow!("Shared source missing"))?;
        self.store.ingest(
            &feed,
            "Unlimited source",
            &[IncomingArticle {
                external_id: "shared".into(),
                title: "Shared image".into(),
                url: "https://other.example/article".into(),
                author: String::new(),
                published: 1_000_000_000,
                body: body.into(),
                body_origin: FeedOrigin::Feed,
            }],
        )
    }

    fn reopen(&self) -> Result<Store> {
        Store::open(&self.directory.path().join("library.sqlite3"))
    }

    fn unrelated_images(&self) -> Result<[std::path::PathBuf; 2]> {
        let root = self.directory.path().join("images");
        std::fs::create_dir_all(root.join("offline"))?;
        let pinned = root.join("offline/pinned.img");
        let unrelated = root.join("personal.txt");
        std::fs::write(&pinned, b"Pinned copy")?;
        std::fs::write(&unrelated, b"Unrelated file")?;
        Ok([pinned, unrelated])
    }

    fn saved_page(&self) -> Result<i64> {
        let saved =
            self.store.save_web_document(&crate::model::WebDocument {
                url: "https://example.org/kept".into(),
                title: "Kept page".into(),
                body: "An explicit saved page with durable readable content."
                    .into(),
            })?;
        for folder in self.store.article_memberships(saved)? {
            self.store.remove_member(folder, saved)?;
        }
        self.store.clear_history()?;
        self.store.connection.execute(
            include_str!("../sql/test_age_retention.sql"),
            (saved, 1_000_000_000, 1_000_000_000),
        )?;
        Ok(saved)
    }

    fn offline_article(&self, article: i64) -> Result<()> {
        self.store.save_offline_state(&crate::model::OfflineState {
            name: "Keep offline".into(),
            total: 1,
            limits: Default::default(),
            running: false,
            cancelled: false,
            items: vec![crate::model::OfflineItem {
                article,
                url: self.store.article(article)?.url,
                title: "Offline".into(),
                status: crate::model::OfflineStatus::Ready,
                bytes: 10,
                images: 0,
                error: String::new(),
            }],
        })
    }

    fn image(&self, address: &str, old: bool) -> Result<std::path::PathBuf> {
        let root = self.directory.path().join("images");
        std::fs::create_dir_all(&root)?;
        let path = crate::media::cache::path(&root, address);
        std::fs::write(&path, b"cached fixture image")?;
        if old {
            std::fs::File::open(&path)?.set_modified(
                std::time::UNIX_EPOCH
                    + std::time::Duration::from_secs(1_000_000_000),
            )?;
        }
        Ok(path)
    }

    fn new() -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let store = Store::open(&directory.path().join("library.sqlite3"))?;
        store
            .connection
            .execute_batch(include_str!("../sql/test_setup_retention.sql"))?;
        Ok(Self { directory, store })
    }

    fn policy(&self, policy: Policy) -> Result<()> {
        self.store.save_retention_policy(&Retention {
            defaults: policy,
            ..Default::default()
        })
    }

    fn preview(&self) -> Result<retention::Report> {
        retention::preview(
            &self.store,
            &self.directory.path().join("images"),
            NOW,
        )
    }

    fn apply(&self) -> Result<retention::Report> {
        retention::apply(
            &self.store,
            &self.directory.path().join("images"),
            NOW,
            &AtomicBool::new(false),
            None,
        )
    }
}

#[test]
fn expired_images_respect_shared_references_fresh_downloads_and_pinned_copies()
-> Result<()> {
    let fixture = Fixture::new()?;
    fixture.policy(Policy {
        image_days: Age(Some(30)),
        ..Default::default()
    })?;
    fixture.store.set_starred(2, true)?;
    fixture.store.clear_history()?;
    for (id, address) in [(1, "shared"), (2, "shared"), (3, "expired")] {
        fixture.store.connection.execute(
            include_str!("../sql/test_set_retention_body.sql"),
            (id, format!("![Image](https://images.example/{address})")),
        )?;
    }
    let shared = fixture.image("https://images.example/shared", true)?;
    let expired = fixture.image("https://images.example/expired", true)?;
    let orphan = fixture.image("https://images.example/orphan", true)?;
    let fresh = fixture.image("https://images.example/fresh", false)?;
    let root = fixture.directory.path().join("images");
    let [pinned, unrelated] = fixture.unrelated_images()?;
    let now = chrono::Utc::now().timestamp();
    assert_eq!(retention::preview(&fixture.store, &root, now)?.images, 2);
    assert!(expired.exists());
    assert_eq!(
        retention::apply(
            &fixture.store,
            &root,
            now,
            &AtomicBool::new(false),
            None
        )?
        .images,
        2
    );
    assert!(!expired.exists() && !orphan.exists());
    assert!(
        [shared, fresh, pinned, unrelated]
            .iter()
            .all(|path| path.exists())
    );
    Ok(())
}

#[test]
fn image_and_article_budgets_are_independent_and_resume_remaining_work()
-> Result<()> {
    let fixture = Fixture::new()?;
    let policy = Retention {
        max_images_per_run: 1,
        max_articles_per_run: 1,
        defaults: Policy {
            image_days: Age(Some(1)),
            ..Default::default()
        },
        ..Default::default()
    };
    fixture.store.save_retention_policy(&policy)?;
    fixture.image("https://images.example/one", true)?;
    fixture.image("https://images.example/two", true)?;
    assert_eq!(fixture.preview()?.images, 2);
    assert_eq!(fixture.apply()?.images, 1);
    assert_eq!(fixture.apply()?.images, 1);
    assert_eq!(fixture.apply()?.images, 0);
    Ok(())
}

#[test]
fn marking_an_old_article_unread_grants_a_fresh_unread_lifetime() -> Result<()>
{
    let fixture = Fixture::new()?;
    fixture.policy(Policy {
        unread_days: Age(Some(1)),
        ..Default::default()
    })?;
    fixture.store.set_unread(1, true)?;
    fixture.store.clear_history()?;
    assert_eq!(
        retention::preview(
            &fixture.store,
            fixture.directory.path(),
            chrono::Utc::now().timestamp()
        )?
        .articles,
        0
    );
    Ok(())
}

#[test]
fn unlimited_defaults_do_not_remove_any_content() -> Result<()> {
    let fixture = Fixture::new()?;
    assert_eq!(fixture.preview()?, retention::Report::default());
    assert_eq!(fixture.apply()?, retention::Report::default());
    assert_eq!(fixture.store.view_count(&View::All)?, 24);
    assert!(
        !fixture
            .store
            .cached_document(1)?
            .is_none_or(|cache| cache.body.is_empty())
    );
    Ok(())
}

#[test]
fn full_cache_expiry_preserves_feed_text_and_replaces_search() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.policy(Policy {
        full_text_days: Age(Some(30)),
        ..Default::default()
    })?;
    let preview = fixture.preview()?;
    assert_eq!((preview.articles, preview.full_texts), (0, 24));
    assert!(
        !fixture
            .store
            .cached_document(1)?
            .is_none_or(|cache| cache.body.is_empty())
    );
    assert_eq!(fixture.apply()?.full_texts, 24);
    assert_eq!(fixture.store.body(1)?, "Original feed body 1");
    let query = ArticleQuery {
        view: View::All,
        text: "body:quartzcache".into(),
        offset: 0,
        limit: 10,
        retained: None,
    };
    assert_eq!(fixture.store.query_articles(&query)?.total, 0);
    assert_eq!(fixture.store.view_count(&View::All)?, 24);
    Ok(())
}

#[test]
fn read_and_unread_expiry_use_separate_clocks_and_exact_boundaries()
-> Result<()> {
    let fixture = Fixture::new()?;
    fixture.policy(Policy {
        read_days: Age(Some(90)),
        ..Default::default()
    })?;
    fixture.store.connection.execute(
        include_str!("../sql/test_age_retention.sql"),
        (1, NOW - 100 * 86_400, NOW - 90 * 86_400),
    )?;
    fixture.store.set_unread(2, true)?;
    fixture.store.clear_history()?;
    fixture.store.connection.execute(
        include_str!("../sql/test_age_retention.sql"),
        (2, 1_000_000_000, 1_000_000_000),
    )?;
    assert_eq!(fixture.preview()?.articles, 22);
    assert_eq!(fixture.apply()?.articles, 22);
    assert!(fixture.store.article(1).is_ok());
    assert!(fixture.store.article(2)?.unread);
    fixture.policy(Policy {
        unread_days: Age(Some(120)),
        ..Default::default()
    })?;
    assert_eq!(fixture.preview()?.articles, 1);
    assert_eq!(fixture.apply()?.articles, 1);
    Ok(())
}

#[test]
fn personal_annotations_folders_tags_drafts_and_undo_protect_both_actions()
-> Result<()> {
    let fixture = Fixture::new()?;
    fixture
        .store
        .connection
        .execute_batch(include_str!("../sql/test_protect_retention.sql"))?;
    fixture.policy(Policy {
        full_text_days: Age(Some(1)),
        read_days: Age(Some(1)),
        ..Default::default()
    })?;
    assert_eq!(fixture.preview()?.articles, 15);
    assert_eq!(fixture.apply()?.articles, 15);
    for id in 1..=9 {
        assert!(fixture.store.article(id).is_ok());
        assert!(
            !fixture
                .store
                .cached_document(id)?
                .is_none_or(|cache| cache.body.is_empty())
        );
    }
    assert_eq!(fixture.store.comments(5)?[0].body, "Private note");
    assert_eq!(fixture.store.note_drafts()?[0].body, "Unsaved thought");
    Ok(())
}

#[test]
fn smart_list_matches_are_protected_until_explicitly_excluded() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.store.save_smart_list(
        "Keep article",
        &SmartFilter {
            text: "title:\"Article 1\"".into(),
            ..Default::default()
        },
    )?;
    fixture.policy(Policy {
        read_days: Age(Some(1)),
        ..Default::default()
    })?;
    let config = fixture.store.capture_configuration()?;
    assert_eq!(fixture.preview()?.articles, 23);
    let mut policy = fixture.store.retention_policy()?;
    policy
        .unprotected_smart_lists
        .push(config.smart_lists[0].id.clone());
    fixture.store.save_retention_policy(&policy)?;
    assert!(fixture.apply().is_err());
    assert_eq!(fixture.preview()?.articles, 24);
    Ok(())
}

#[test]
fn changing_policy_requires_another_preview_and_cancelled_runs_do_no_work()
-> Result<()> {
    let fixture = Fixture::new()?;
    fixture.policy(Policy {
        read_days: Age(Some(90)),
        ..Default::default()
    })?;
    assert!(fixture.apply().is_err());
    fixture.preview()?;
    fixture.policy(Policy {
        read_days: Age(Some(30)),
        ..Default::default()
    })?;
    assert!(fixture.apply().is_err());
    fixture.preview()?;
    let report = retention::apply(
        &fixture.store,
        fixture.directory.path(),
        NOW,
        &AtomicBool::new(true),
        None,
    )?;
    assert!(report.cancelled);
    assert_eq!(fixture.store.view_count(&View::All)?, 24);
    Ok(())
}

#[test]
fn tombstones_block_reimport_and_new_articles_never_reuse_removed_ids()
-> Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.policy(Policy {
        read_days: Age(Some(1)),
        ..Default::default()
    })?;
    fixture.preview()?;
    assert_eq!(fixture.apply()?.articles, 24);
    let feed = fixture.store.feeds()?.remove(0);
    let mut article = IncomingArticle {
        external_id: "entry:24".into(),
        title: "Old entry".into(),
        url: "https://journal.example/24".into(),
        author: String::new(),
        published: NOW,
        body: "Feed body".into(),
        body_origin: FeedOrigin::Feed,
    };
    fixture
        .store
        .ingest(&feed, "Journal", std::slice::from_ref(&article))?;
    assert_eq!(fixture.store.view_count(&View::All)?, 0);
    article.external_id = "changed-guid".into();
    fixture
        .store
        .ingest(&feed, "Journal", std::slice::from_ref(&article))?;
    assert_eq!(fixture.store.view_count(&View::All)?, 0);
    article.url = "https://journal.example/new".into();
    fixture.store.ingest(&feed, "Journal", &[article])?;
    assert!(fixture.store.library()?.articles[0].id > 24);
    Ok(())
}

#[test]
fn deletion_failure_rolls_back_the_batch_and_its_tombstones() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.policy(Policy {
        read_days: Age(Some(1)),
        ..Default::default()
    })?;
    fixture.preview()?;
    fixture
        .store
        .connection
        .execute_batch(include_str!("../sql/test_reject_retention.sql"))?;
    assert!(fixture.apply().is_err());
    assert!(fixture.store.article(1).is_ok());
    assert!(!super::removed(
        &fixture.store.connection,
        1,
        "entry:1",
        ""
    )?);
    Ok(())
}

#[test]
fn an_external_commit_interrupts_a_plan_before_a_stale_batch_can_delete()
-> Result<()> {
    let fixture = Fixture::new()?;
    fixture.policy(Policy {
        read_days: Age(Some(1)),
        ..Default::default()
    })?;
    let plan = fixture.store.prepare_retention(
        fixture.store.retention_policy()?,
        NOW,
        None,
    )?;
    let other = fixture.reopen()?;
    other.set_starred(1, true)?;
    assert!(
        fixture
            .store
            .retention_batch(&plan, super::Step::DeleteArticles, 0, 32, NOW)?
            .is_none()
    );
    assert_eq!(fixture.store.view_count(&View::All)?, 24);
    assert!(fixture.store.article(1)?.starred);
    Ok(())
}

#[test]
fn automatic_retention_requires_preview_and_obeys_persisted_intervals()
-> Result<()> {
    let fixture = Fixture::new()?;
    let mut policy = Retention {
        automatic: true,
        max_articles_per_run: 2,
        defaults: Policy {
            read_days: Age(Some(1)),
            ..Default::default()
        },
        ..Default::default()
    };
    fixture.store.save_retention_policy(&policy)?;
    let images = fixture.directory.path().join("images");
    let cancel = AtomicBool::new(false);
    assert!(
        retention::automatic(&fixture.store, &images, NOW, &cancel, None)?
            .is_none()
    );
    policy.automatic = false;
    fixture.store.save_retention_policy(&policy)?;
    fixture.preview()?;
    policy.automatic = true;
    fixture.store.save_retention_policy(&policy)?;
    let first =
        retention::automatic(&fixture.store, &images, NOW, &cancel, None)?;
    assert_eq!(first.map(|report| report.articles), Some(2));
    assert_eq!(
        retention::next_check(&fixture.store, NOW)?.as_secs(),
        86_400
    );
    let reopened = fixture.reopen()?;
    assert!(
        retention::automatic(&reopened, &images, NOW + 60, &cancel, None)?
            .is_none()
    );
    assert_eq!(
        retention::automatic(&reopened, &images, NOW + 86_400, &cancel, None)?
            .map(|report| report.articles),
        Some(2)
    );
    Ok(())
}

#[test]
fn a_source_can_restore_unlimited_age_without_losing_other_inherited_fields()
-> Result<()> {
    let fixture = Fixture::new()?;
    let config = fixture.store.capture_configuration()?;
    let mut policy = Retention {
        defaults: Policy {
            read_days: Age(Some(90)),
            ..Default::default()
        },
        ..Default::default()
    };
    policy.sources.push(crate::config::retention::Source {
        id: config.feeds[0].id.clone(),
        full_text_days: Some(Age(Some(30))),
        image_days: None,
        read_days: Some(Age(None)),
        unread_days: None,
    });
    fixture.store.save_retention_policy(&policy)?;
    let preview = fixture.preview()?;
    assert_eq!((preview.articles, preview.full_texts), (0, 24));
    assert_eq!(fixture.apply()?.full_texts, 24);
    assert_eq!(fixture.store.view_count(&View::All)?, 24);
    Ok(())
}

#[test]
fn active_reader_and_saved_pages_are_protected_without_manual_folder_membership()
-> Result<()> {
    let fixture = Fixture::new()?;
    let saved = fixture.saved_page()?;
    fixture.policy(Policy {
        read_days: Age(Some(1)),
        unread_days: Age(Some(1)),
        full_text_days: Age(Some(1)),
        ..Default::default()
    })?;
    fixture.preview()?;
    let report = retention::apply(
        &fixture.store,
        fixture.directory.path(),
        NOW,
        &AtomicBool::new(false),
        Some(24),
    )?;
    assert_eq!(report.articles, 23);
    assert!(fixture.store.article(saved).is_ok());
    assert!(
        !fixture
            .store
            .cached_document(24)?
            .is_none_or(|cache| cache.body.is_empty())
    );
    Ok(())
}

#[test]
fn offline_packages_and_remaining_reading_sessions_keep_their_captured_articles()
-> Result<()> {
    let fixture = Fixture::new()?;
    fixture.offline_article(1)?;
    fixture
        .store
        .save_reading_session(&crate::model::ReadingSession {
            budget: 5,
            position: 0,
            items: vec![crate::model::SessionItem {
                article: 2,
                feed: 1,
                url: fixture.store.article(2)?.url,
                canonical: "https://journal.example/2".into(),
                title: "Session".into(),
                minutes: 2,
                score: 1,
                topic: String::new(),
            }],
        })?;
    fixture.policy(Policy {
        read_days: Age(Some(1)),
        ..Default::default()
    })?;
    assert_eq!(fixture.preview()?.articles, 22);
    assert_eq!(fixture.apply()?.articles, 22);
    assert!(fixture.store.article(1).is_ok());
    assert!(fixture.store.article(2).is_ok());
    Ok(())
}

#[test]
fn a_failed_later_batch_preserves_prior_commits_and_never_overruns_the_budget()
-> Result<()> {
    let fixture = Fixture::new()?;
    let policy = Retention {
        batch_size: 1,
        max_articles_per_run: 3,
        defaults: Policy {
            read_days: Age(Some(1)),
            ..Default::default()
        },
        ..Default::default()
    };
    fixture.store.save_retention_policy(&policy)?;
    fixture.preview()?;
    fixture
        .store
        .connection
        .execute_batch(include_str!("../sql/test_reject_retention.sql"))?;
    assert!(fixture.apply().is_err());
    assert!(fixture.store.article(1).is_err());
    assert!(fixture.store.article(2).is_ok());
    assert!(super::removed(&fixture.store.connection, 1, "entry:1", "")?);
    assert!(!super::removed(
        &fixture.store.connection,
        1,
        "entry:2",
        ""
    )?);
    Ok(())
}

#[test]
fn deleting_a_smart_list_removes_its_retention_exception_atomically()
-> Result<()> {
    let fixture = Fixture::new()?;
    fixture.store.save_smart_list(
        "Temporary",
        &SmartFilter {
            text: "Article".into(),
            ..Default::default()
        },
    )?;
    let config = fixture.store.capture_configuration()?;
    let mut policy = Retention::default();
    policy.defaults.read_days = Age(Some(1));
    policy
        .unprotected_smart_lists
        .push(config.smart_lists[0].id.clone());
    fixture.store.save_retention_policy(&policy)?;
    assert_eq!(fixture.preview()?.articles, 24);
    let id = fixture.store.smart_lists()?[0].id;
    fixture.store.delete_smart_list(id)?;
    let remaining = fixture.store.capture_configuration()?;
    assert!(remaining.smart_lists.is_empty());
    assert!(remaining.retention.unprotected_smart_lists.is_empty());
    assert!(fixture.apply().is_err());
    assert_eq!(fixture.store.view_count(&View::All)?, 24);
    Ok(())
}

#[test]
fn long_galleries_keep_protected_images_beyond_the_offline_download_limit()
-> Result<()> {
    let fixture = Fixture::new()?;
    fixture.policy(Policy {
        image_days: Age(Some(30)),
        ..Default::default()
    })?;
    let gallery = (0..96)
        .map(|id| format!("![Image](https://images.example/gallery/{id})\n\n"))
        .collect::<String>();
    fixture.store.connection.execute(
        include_str!("../sql/test_set_retention_body.sql"),
        (2, &gallery),
    )?;
    fixture.store.set_starred(2, true)?;
    let protected = fixture.image("https://images.example/gallery/95", true)?;
    let orphan = fixture.image("https://images.example/orphan", true)?;
    assert_eq!(fixture.preview()?.images, 1);
    assert_eq!(fixture.apply()?.images, 1);
    assert!(protected.is_file());
    assert!(!orphan.exists());
    Ok(())
}

#[test]
fn a_fresh_cache_never_scans_unneeded_historical_image_references() -> Result<()>
{
    let fixture = Fixture::new()?;
    fixture.policy(Policy {
        image_days: Age(Some(30)),
        ..Default::default()
    })?;
    fixture.store.connection.execute(
        include_str!("../sql/test_set_retention_body.sql"),
        (1, "![An invalid historical reference](file:///unavailable)"),
    )?;
    let fresh = fixture.image("https://images.example/fresh", false)?;
    let root = fixture.directory.path().join("images");
    let now = chrono::Utc::now().timestamp();
    assert_eq!(retention::preview(&fixture.store, &root, now)?.images, 0);
    assert!(fresh.is_file());
    Ok(())
}

#[test]
fn image_age_prefilter_honors_short_source_overrides_and_unlimited_shared_sources()
-> Result<()> {
    let mut fixture = Fixture::new()?;
    let key = fixture.store.capture_configuration()?.feeds[0].id.clone();
    let mut policy = Retention::default();
    policy.sources.push(crate::config::retention::Source {
        id: key,
        full_text_days: None,
        read_days: None,
        unread_days: None,
        image_days: Some(Age(Some(7))),
    });
    fixture.store.save_retention_policy(&policy)?;
    let body = "![Image](https://images.example/shared)";
    fixture.store.connection.execute(
        include_str!("../sql/test_set_retention_body.sql"),
        (1, body),
    )?;
    let image = fixture.image("https://images.example/shared", false)?;
    let now = chrono::Utc::now().timestamp();
    std::fs::File::open(&image)?.set_modified(
        std::time::UNIX_EPOCH
            + std::time::Duration::from_secs((now - 10 * 86_400) as u64),
    )?;
    let root = fixture.directory.path().join("images");
    assert_eq!(retention::preview(&fixture.store, &root, now)?.images, 1);
    fixture.publish_image_source(body)?;
    assert_eq!(retention::preview(&fixture.store, &root, now)?.images, 0);
    assert_eq!(fixture.apply()?.images, 0);
    assert!(image.is_file());
    Ok(())
}
