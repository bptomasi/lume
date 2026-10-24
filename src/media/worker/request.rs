//! A source download survives crop changes; obsolete sources stop before decode.
use crate::media::ImageKey;
use anyhow::{Result, anyhow};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

pub(super) struct Request {
    pub(super) url: String,
    pub(super) generated: Option<Arc<[u8]>>,
    state: Mutex<State>,
    revision: Arc<AtomicU64>,
    pub(super) canceled: Arc<AtomicBool>,
}

struct State {
    key: Option<ImageKey>,
    finished: bool,
}

impl Request {
    pub(super) fn new(key: ImageKey, generated: Option<Arc<[u8]>>) -> Self {
        Self {
            url: key.url.clone(),
            generated,
            revision: Arc::new(AtomicU64::new(0)),
            canceled: Arc::new(AtomicBool::new(false)),
            state: Mutex::new(State {
                key: Some(key),
                finished: false,
            }),
        }
    }

    pub(super) fn snapshot(
        &self,
    ) -> Result<Option<(ImageKey, crate::cancellation::Cancellation)>> {
        let state = self
            .state
            .lock()
            .map_err(|_| anyhow!("Image request stopped"))?;
        let revision = self.revision.load(Ordering::Acquire);
        Ok(state.key.clone().map(|key| {
            (
                key,
                crate::cancellation::Cancellation::Generation(
                    self.revision.clone(),
                    revision,
                ),
            )
        }))
    }

    pub(super) fn key(&self) -> Result<Option<ImageKey>> {
        Ok(self
            .state
            .lock()
            .map_err(|_| anyhow!("Image request stopped"))?
            .key
            .clone())
    }

    pub(super) fn retarget(&self, key: Option<ImageKey>) -> Result<bool> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("Image request stopped"))?;
        if state.finished || (state.key.is_none() && key.is_some()) {
            return Ok(false);
        }
        if key.is_none() {
            self.canceled.store(true, Ordering::Release);
        }
        if state.key != key {
            self.revision.fetch_add(1, Ordering::AcqRel);
        }
        state.key = key;
        Ok(true)
    }

    pub(super) fn finish(&self) -> Result<bool> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("Image request stopped"))?;
        state.finished = true;
        Ok(state.key.is_some())
    }
}
