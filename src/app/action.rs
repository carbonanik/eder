use crate::domain::reading_position::ReadingPosition;

/// Local actions are ordinary main-thread calls, not channel messages.
pub enum Action {
    Navigate(u32),
    ViewportPosition(ReadingPosition),
}
