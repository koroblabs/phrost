/// Whether the index can answer cross-file questions yet.
///
/// Explicit rather than implicit, because the implicit version is a bug this project has already
/// paid for: while a background scan filled the catalog, requests saw whatever happened to be in
/// it, so goto and completion worked for some symbols and not others depending on timing. The
/// symptom was "it works sometimes", which is far harder to diagnose than "not ready yet".
///
/// So a cross-file request during [`IndexState::Loading`] gets an honest "still indexing" rather
/// than a partial answer, and features never appear to switch themselves on gradually.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum IndexState {
    /// The catalog is incomplete. Cross-file answers are not trustworthy yet.
    #[default]
    Loading,
    /// Every discovered file has been ingested at least once.
    Ready,
}

impl IndexState {
    pub fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready => "ready",
        }
    }
}
