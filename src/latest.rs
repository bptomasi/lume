//! Latest-request workers for replaceable reads and ephemeral projections.
//!
//! One active job, one queued job and one deferred replacement bound work.
//! Durable mutations must not use this owner: replacing a read is valid, losing a write
//! is not. Dropping disconnects channels without joining an expensive job.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};

use crate::cancellation::Cancellation;
use anyhow::{Result, anyhow};

struct Job<I> {
    generation: u64,
    input: I,
}

pub(crate) struct Latest<I, O> {
    requests: mpsc::SyncSender<Job<I>>,
    results: mpsc::Receiver<(u64, Result<O>)>,
    generation: Arc<AtomicU64>,
    deferred: Option<Job<I>>,
    pending: bool,
    stopped: bool,
}

impl<I: Send + 'static, O: Send + 'static> Latest<I, O> {
    pub(crate) fn start(
        name: &str,
        mut operation: impl FnMut(I) -> Result<O> + Send + 'static,
    ) -> Result<Self> {
        Self::start_cancellable(name, move |input, _| operation(input))
    }

    pub(crate) fn start_cancellable(
        name: &str,
        operation: impl FnMut(I, Cancellation) -> Result<O> + Send + 'static,
    ) -> Result<Self> {
        let (requests, receiver) = mpsc::sync_channel::<Job<I>>(1);
        let (sender, results) = mpsc::sync_channel(1);
        let generation = Arc::new(AtomicU64::new(0));
        let current = generation.clone();
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || run(receiver, sender, current, operation))?;
        Ok(Self {
            requests,
            results,
            generation,
            deferred: None,
            pending: false,
            stopped: false,
        })
    }

    pub(crate) fn submit(&mut self, input: I) {
        // Wrapping cannot alias an outstanding job without 2^64 submissions.
        let generation = self
            .generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        self.deferred = Some(Job { generation, input });
        self.pending = true;
        self.admit();
    }

    pub(crate) fn cancel(&mut self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.deferred = None;
        self.pending = false;
    }

    fn admit(&mut self) {
        let Some(job) = self.deferred.take() else {
            return;
        };
        match self.requests.try_send(job) {
            Ok(()) => {}
            Err(mpsc::TrySendError::Full(job)) => self.deferred = Some(job),
            Err(mpsc::TrySendError::Disconnected(_)) => self.stopped = true,
        }
    }

    pub(crate) fn poll(&mut self) -> Option<Result<O>> {
        self.admit();
        loop {
            match self.results.try_recv() {
                Ok((generation, result))
                    if generation
                        == self.generation.load(Ordering::Acquire) =>
                {
                    self.pending = false;
                    return Some(result);
                }
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => self.stopped = true,
            }
            if self.stopped {
                break;
            }
        }
        if self.stopped && self.pending {
            self.pending = false;
            self.deferred = None;
            return Some(Err(anyhow!("Background worker disconnected")));
        }
        None
    }

    pub(crate) fn pending(&self) -> bool {
        self.pending
    }
}

fn run<I, O>(
    receiver: mpsc::Receiver<Job<I>>,
    sender: mpsc::SyncSender<(u64, Result<O>)>,
    current: Arc<AtomicU64>,
    mut operation: impl FnMut(I, Cancellation) -> Result<O>,
) {
    while let Ok(job) = receiver.recv() {
        if current.load(Ordering::Acquire) != job.generation {
            continue;
        }
        let cancel = Cancellation::Generation(current.clone(), job.generation);
        let result = operation(job.input, cancel);
        if current.load(Ordering::Acquire) == job.generation
            && sender.send((job.generation, result)).is_err()
        {
            break;
        }
    }
}

impl<I, O> Drop for Latest<I, O> {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn replacement_and_drop_cancel_the_active_generation() -> Result<()> {
        let (started, ready) = mpsc::sync_channel(1);
        let mut worker = Latest::start_cancellable(
            "cancel-active",
            move |value, cancel| {
                if value == 1 {
                    started.send(cancel.clone())?;
                    while !cancel.is_cancelled() {
                        std::thread::yield_now();
                    }
                    cancel.check()?;
                }
                Ok(value)
            },
        )?;
        worker.submit(1);
        let cancel = ready.recv_timeout(Duration::from_secs(5))?;
        worker.submit(2);
        assert!(cancel.is_cancelled());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = worker.poll() {
                assert_eq!(result?, 2);
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        worker.submit(1);
        let cancel = ready.recv_timeout(Duration::from_secs(5))?;
        drop(worker);
        assert!(cancel.is_cancelled());
        Ok(())
    }

    #[test]
    fn blocked_work_is_replaceable_and_drop_does_not_join() -> Result<()> {
        let (release, permission) = mpsc::channel();
        let (started, starts) = mpsc::channel();
        let mut worker =
            Latest::<i32, i32>::start("latest-test", move |value| {
                started.send(value)?;
                if value == 1 {
                    permission.recv_timeout(Duration::from_secs(5))?;
                }
                Ok(value * 10)
            })?;
        worker.submit(1);
        assert_eq!(starts.recv_timeout(Duration::from_secs(5))?, 1);
        for value in 2..100 {
            worker.submit(value);
        }
        release.send(())?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = worker.poll() {
                assert_eq!(result?, 990);
                break;
            }
            assert!(Instant::now() < deadline, "Latest result did not arrive");
            std::thread::yield_now();
        }
        assert!(!worker.pending());
        Ok(())
    }
}
