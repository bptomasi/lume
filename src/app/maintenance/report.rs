use crate::model::{OfflineState, OfflineStatus, ReadingSession};

pub(super) fn offline(state: &OfflineState) -> String {
    let ready = state
        .items
        .iter()
        .filter(|item| item.status == OfflineStatus::Ready)
        .count();
    let bytes = state.items.iter().map(|item| item.bytes).sum::<u64>();
    let mut text = format!(
        r#"{} · {ready}/{} fully prepared · {} matched
{bytes}/{} logical bytes · running/interrupted={} · cancelled={}

"#,
        state.name,
        state.items.len(),
        state.total,
        state.limits.bytes,
        state.running,
        state.cancelled
    );
    for item in &state.items {
        text.push_str(&format!(
            "{:?} · {} · {} images · {} bytes\n{}\n\n",
            item.status, item.title, item.images, item.bytes, item.error
        ));
    }
    text.push_str(
        r#":offline resume|cancel|release|status
Restart with lume --offline to forbid all HTTP.
A partial entry is not fully available offline.
"#,
    );
    text
}

pub(super) fn session(session: &ReadingSession) -> String {
    let total = session
        .items
        .iter()
        .map(|item| u32::from(item.minutes))
        .sum::<u32>();
    let mut text = format!(
        "{} minute budget · {total} estimated minutes · {} articles\n\n",
        session.budget,
        session.items.len()
    );
    for (index, item) in session.items.iter().enumerate() {
        let marker = if index == session.position { '>' } else { ' ' };
        text.push_str(&format!(
            "{marker} {}. {} · {} min\n",
            index + 1,
            item.title,
            item.minutes
        ));
    }
    if session.items.is_empty() {
        text.push_str(
            r#"No cached unread recommendation fits. Configure recommendation
interests, allow classification to finish, or increase the budget.
"#,
        );
    } else if session.position == session.items.len() {
        text.push_str("\nSession complete.\n");
    }
    text.push_str(
        r#"
:session status resumes · :session next advances · :session end releases
Estimates are approximate. Planning/advancing does not mark articles read.
"#,
    );
    text
}
