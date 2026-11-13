//! Typed HTTP failure retains server retry authority across redaction.

#[derive(Debug)]
pub(super) struct HttpFailure {
    pub(super) status: u16,
    pub(super) retry_at: i64,
}

impl std::fmt::Display for HttpFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Miniflux returned HTTP {}", self.status)
    }
}

impl std::error::Error for HttpFailure {}

pub(super) fn retry_deadline(error: &anyhow::Error) -> i64 {
    error
        .downcast_ref::<HttpFailure>()
        .map_or(0, |error| error.retry_at)
}
