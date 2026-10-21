//! Real definition edits, comment-preserving publication and verified rereads.

use anyhow::{Context, Result, ensure};
use lume::{
    config::{self, Session},
    model::SmartFilter,
    source::discovery::Candidate,
    store::Store,
};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let count = if stress { 5000 } else { 250 };
    let scenario = Scenario::prepare(directory.path(), count, stress)?;
    let mut sequence = 0;
    super::measure(
        &format!("declarative publication + reread ({count} feeds)"),
        iterations,
        || {
            sequence += 1;
            scenario.edit_and_verify(sequence)
        },
    )?;
    scenario.verify_pending_checks(iterations)?;
    scenario.verify_formatting(iterations)
}

struct Scenario {
    store: Store,
    session: Session,
    feed: lume::model::Feed,
    folder: i64,
    key: String,
    original: config::Configuration,
}

impl Scenario {
    fn prepare(
        path: &std::path::Path,
        count: usize,
        stress: bool,
    ) -> Result<Self> {
        let store = Store::open(&path.join("library.sqlite3"))?;
        seed(&store, count, if stress { 256 } else { 32 })?;
        let feed = store.feeds()?.remove(0);
        let folder = store.create_collection("Reading")?;
        let path = path.join("config.toml");
        std::fs::write(
            &path,
            "# Keep this user comment\nschema_version = 1\n",
        )?;
        let session = Session::open(&store, &path)?;
        let original = config::read_document(&path)?;
        let key = original
            .feeds
            .iter()
            .find(|item| item.url == feed.url)
            .context("Benchmark subscription absent from configuration")?
            .id
            .clone();
        Ok(Self {
            store,
            session,
            feed,
            folder,
            key,
            original,
        })
    }

    fn verify_pending_checks(&self, iterations: usize) -> Result<()> {
        let connection = rusqlite::Connection::open(
            self.session.path().with_file_name("library.sqlite3"),
        )?;
        self.store.create_collection("Pending checkpoint")?;
        ensure!(
            pending(&connection, self.session.path())?,
            "Pending edit lost"
        );
        self.session.flush()?;
        super::measure("200 configuration revision checks", iterations, || {
            for _ in 0..200 {
                ensure!(
                    !pending(&connection, self.session.path())?,
                    "Published edit remained pending"
                );
            }
            Ok(())
        })
    }

    fn verify_formatting(&self, iterations: usize) -> Result<()> {
        let path = self.session.path();
        let original = config::read_document(path)?;
        let compact = format!(
            "# Keep this user comment\n{}",
            toml_edit::ser::to_document(&original)?
        );
        super::measure("compact rewrite + format + reread", iterations, || {
            std::fs::write(path, &compact)?;
            config::format_file(path)?;
            let text = std::fs::read_to_string(path)?;
            ensure!(config::decode(&text)? == original, "Definitions changed");
            ensure!(
                text.contains("# Keep this user comment")
                    && text.matches("[[feeds]]").count()
                        == original.feeds.len()
                    && text.matches("[[smart_lists]]").count()
                        == original.smart_lists.len(),
                "Comments or record layout lost"
            );
            Ok(())
        })
    }

    fn edit_and_verify(&self, sequence: usize) -> Result<()> {
        let name = format!("Reading {sequence}");
        self.store.rename_collection(self.folder, &name)?;
        if sequence.is_multiple_of(2) {
            self.store.subscribe(&self.feed.url)?;
        } else {
            self.store.unsubscribe(self.feed.id)?;
        }
        self.session.flush()?;
        let actual = config::read_document(self.session.path())?;
        ensure!(
            actual.feeds.len() == self.original.feeds.len(),
            "Configuration lost subscriptions"
        );
        let selected = actual
            .feeds
            .iter()
            .find(|item| item.id == self.key)
            .context("Stable source ID changed")?;
        ensure!(
            selected.enabled == (sequence.is_multiple_of(2)),
            "Source state was not published"
        );
        ensure!(
            actual.folders[0].name == name,
            "Folder rename was not published"
        );
        ensure!(
            actual.smart_lists == self.original.smart_lists,
            "Queries changed"
        );
        ensure!(
            std::fs::read_to_string(self.session.path())?
                .contains("# Keep this user comment"),
            "Comment lost"
        );
        Ok(())
    }
}

fn seed(store: &Store, count: usize, lists: usize) -> Result<()> {
    let feeds = (0..count)
        .map(|index| Candidate {
            title: format!("Publication {index}"),
            url: format!("https://publication-{index}.example/feed"),
        })
        .collect::<Vec<_>>();
    store.import_feeds(&feeds)?;
    let sources = store
        .feeds()?
        .iter()
        .take(8)
        .map(|feed| feed.id)
        .collect::<Vec<_>>();
    for index in 0..lists {
        store.save_smart_list(
            &format!("Subject {index}"),
            &SmartFilter {
                text: format!("science{index} OR astronomy"),
                feeds: sources.clone(),
                ..Default::default()
            },
        )?;
    }
    Ok(())
}

fn pending(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
) -> Result<bool> {
    let (owner, pending): (String, bool) = connection
        .prepare_cached(include_str!(
            "../../src/store/sql/configuration/select_pending.sql"
        ))?
        .query_row([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    ensure!(
        Some(owner.as_str()) == path.to_str(),
        "Configuration owner changed"
    );
    Ok(pending)
}
