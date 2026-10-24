//! One lane owns each hostname for discovery, publisher feeds and entry pages.
use super::*;

#[derive(Default)]
struct Timing {
    starts: BTreeMap<String, Instant>,
    retries: BTreeMap<String, i64>,
}

pub(super) fn run(
    jobs: Vec<Job>,
    stopped: &AtomicBool,
    sender: mpsc::SyncSender<Event>,
    discover: &(impl Fn(&Server) -> Result<Vec<RemoteFeedCandidate>> + Sync),
    fetch: &(impl Fn(&Feed) -> Download + Sync),
) {
    let mut timing = Timing::default();
    for job in jobs {
        if stopped.load(Ordering::Relaxed) {
            break;
        }
        let feeds = match job {
            Job::Publisher(feed) => vec![feed],
            Job::Server(server) => {
                let Some(feeds) =
                    timing.discover(server, stopped, &sender, discover)
                else {
                    return;
                };
                feeds
            }
        };
        for feed in feeds {
            let Some(downloaded) = timing.download(feed, stopped, fetch) else {
                return;
            };
            if stopped.load(Ordering::Relaxed)
                || sender.send(Event::Download(Box::new(downloaded))).is_err()
            {
                return;
            }
        }
    }
}

impl Timing {
    fn discover(
        &mut self,
        server: Server,
        stopped: &AtomicBool,
        sender: &mpsc::SyncSender<Event>,
        discover: &(impl Fn(&Server) -> Result<Vec<RemoteFeedCandidate>> + Sync),
    ) -> Option<Vec<Feed>> {
        if self.deadline(&server.url) > chrono::Utc::now().timestamp() {
            return Some(Vec::new());
        }
        let result = discover(&server);
        if stopped.load(Ordering::Relaxed) {
            return None;
        }
        if let Err(error) = &result {
            self.remember(
                &server.url,
                super::super::super::miniflux::retry_deadline(error),
            );
        }
        let (reply, receiver) = mpsc::sync_channel(1);
        if sender
            .send(Event::Discovery {
                server,
                result,
                reply,
            })
            .is_err()
        {
            return None;
        }
        receiver.recv().ok()
    }

    fn deadline(&self, address: &str) -> i64 {
        self.retries.get(&domain(address)).copied().unwrap_or(0)
    }

    fn remember(&mut self, address: &str, deadline: i64) {
        if deadline > 0 {
            self.retries
                .entry(domain(address))
                .and_modify(|old| *old = (*old).max(deadline))
                .or_insert(deadline);
        }
    }

    fn download(
        &mut self,
        feed: Feed,
        stopped: &AtomicBool,
        fetch: &(impl Fn(&Feed) -> Download + Sync),
    ) -> Option<Download> {
        if stopped.load(Ordering::Relaxed) {
            return None;
        }
        let host = domain(&feed.url);
        let deadline = self.deadline(&feed.url);
        let downloaded = if deadline > chrono::Utc::now().timestamp() {
            Download {
                feed,
                result: Err(anyhow::anyhow!(
                    "Domain Retry-After until {deadline}"
                )),
                retry_at: deadline,
            }
        } else {
            if feed.server.is_none()
                && let Some(previous) = self.starts.get(&host)
            {
                thread::sleep(
                    Duration::from_millis(500)
                        .saturating_sub(previous.elapsed()),
                );
            }
            if stopped.load(Ordering::Relaxed) {
                return None;
            }
            self.starts.insert(host, Instant::now());
            fetch(&feed)
        };
        self.remember(&downloaded.feed.url, downloaded.retry_at);
        Some(downloaded)
    }
}
