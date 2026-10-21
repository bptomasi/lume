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
    )
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
