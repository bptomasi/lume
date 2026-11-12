//! Closed personal flags shared by individual and captured batch edits.
//!
//! SQL is selected by the owner, never interpolated from column spellings.

#[derive(Clone, Copy)]
pub(super) enum PersonalFlag {
    Unread,
    Starred,
    Queued,
}

impl PersonalFlag {
    pub(super) fn update_one(self) -> &'static str {
        match self {
            Self::Unread => include_str!("sql/update_personal_unread.sql"),
            Self::Starred => include_str!("sql/update_personal_starred.sql"),
            Self::Queued => include_str!("sql/update_personal_queued.sql"),
        }
    }

    pub(super) fn update_batch(self) -> &'static str {
        match self {
            Self::Unread => include_str!("sql/update_bulk_unread.sql"),
            Self::Starred => include_str!("sql/update_bulk_starred.sql"),
            Self::Queued => include_str!("sql/update_bulk_queued.sql"),
        }
    }
}
