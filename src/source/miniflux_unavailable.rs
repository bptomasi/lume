//! Transport is omitted while stored server data remains portable.
use std::sync::atomic::AtomicBool;

use anyhow::{Result, bail};

use crate::model::{Feed, RemoteFeedCandidate, Server, ServerFeed};

pub(super) fn discover(
    _client: &ureq::Agent,
    _server: &Server,
    _stopped: &AtomicBool,
) -> Result<Vec<RemoteFeedCandidate>> {
    bail!("Miniflux unavailable in this build; enable the miniflux feature")
}

pub(super) fn fetch_entries(
    _client: &ureq::Agent,
    feed: &Feed,
    _server: &ServerFeed,
    _stopped: &AtomicBool,
) -> super::download::Download {
    super::download::Download {
        feed: feed.clone(),
        result: Err(anyhow::anyhow!(
            "Miniflux unavailable in this build; enable the miniflux feature"
        )),
        retry_at: 0,
    }
}
