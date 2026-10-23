//! Personal mutations invalidate only destinations whose predicates observe them.
use super::Expression;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Changes(u8);

impl Changes {
    pub(crate) const READ: Self = Self(1);
    pub(crate) const FAVORITE: Self = Self(2);
    pub(crate) const QUEUE: Self = Self(4);
    pub(crate) const TAGS: Self = Self(8);
    pub(crate) const NOTES: Self = Self(16);
    pub(crate) const MEMBERSHIPS: Self = Self(32);

    pub(crate) fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub(crate) fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub(crate) fn bulk(edit: &crate::model::BulkEdit) -> Self {
        use crate::model::BulkEdit;
        match edit {
            BulkEdit::Unread(_) => Self::READ,
            BulkEdit::Favorite(_) => Self::FAVORITE,
            BulkEdit::Queue(_) => Self::QUEUE,
            BulkEdit::Tag(_) => Self::TAGS,
            BulkEdit::Folder(_) => Self::MEMBERSHIPS,
        }
    }
}

impl Expression {
    pub(crate) fn depends_on(&self, changes: Changes) -> bool {
        match self {
            Self::All => false,
            Self::Term { field, value, .. } => {
                let dependency = match (field.as_str(), value.as_str()) {
                    ("is", "read" | "unread") => Changes::READ,
                    ("is", "favorite") => Changes::FAVORITE,
                    ("is", "queued") => Changes::QUEUE,
                    ("tag", _) => Changes::TAGS,
                    ("" | "topic" | "notes" | "highlight-tag", _)
                    | ("is", "annotated") => Changes::NOTES,
                    _ => Changes::default(),
                };
                dependency.intersects(changes)
            }
            Self::And(left, right) | Self::Or(left, right) => {
                left.depends_on(changes) || right.depends_on(changes)
            }
            Self::Not(inner) => inner.depends_on(changes),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boolean_dependencies_preserve_negation_and_independent_tag_owners()
    -> anyhow::Result<()> {
        for (text, yes, no) in [
            ("NOT is:favorite", Changes::FAVORITE, Changes::READ),
            (
                "is:queued OR is:read",
                Changes::QUEUE.union(Changes::READ),
                Changes::TAGS,
            ),
            ("tag:rust AND title:systems", Changes::TAGS, Changes::NOTES),
            (
                "notes:queue OR highlight-tag:todo",
                Changes::NOTES,
                Changes::TAGS,
            ),
            ("queue", Changes::NOTES, Changes::QUEUE),
        ] {
            let expression = super::super::parse(text)?;
            assert!(expression.depends_on(yes));
            assert!(!expression.depends_on(no));
        }
        Ok(())
    }
}
