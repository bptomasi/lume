//! A source download survives crop changes; obsolete sources stop before decode.
use crate::media::ImageKey;
use anyhow::{Result, anyhow};
use std::sync::{Arc, Mutex};

pub(super) struct Request {
    pub(super) url: String,
    pub(super) generated: Option<Arc<[u8]>>,
    state: Mutex<State>,
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
            state: Mutex::new(State {
                key: Some(key),
                finished: false,
            }),
        }
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
        if state.finished {
            return Ok(false);
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
