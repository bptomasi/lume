//! SQLite contention is classified at the persistence boundary.

/// Whether a failure can be retried after its owning transaction has rolled back.
///
/// Inspect every source, including SQLite errors nested under application context
/// or conversion errors. Callers still own retry limits and idempotence.
pub(crate) fn is_contention(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause
                .downcast_ref::<rusqlite::Error>()
                .and_then(rusqlite::Error::sqlite_error_code),
            Some(
                rusqlite::ErrorCode::DatabaseBusy
                    | rusqlite::ErrorCode::DatabaseLocked
            )
        )
    })
}

#[cfg(test)]
mod tests;
