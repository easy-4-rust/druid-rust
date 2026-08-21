/// Reason why a telemetry snapshot could not be obtained.
///
/// `Busy` indicates a non-blocking attempt found the datasource locked;
/// callers may retry or skip. `Closed` indicates the datasource has been
/// shut down and will never produce snapshots again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotUnavailable {
    /// The datasource is currently busy (e.g. lock held); retry later.
    Busy,
    /// The datasource has been closed; no further snapshots are possible.
    Closed,
}

impl std::fmt::Display for SnapshotUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnapshotUnavailable::Busy => write!(f, "datasource busy, try later"),
            SnapshotUnavailable::Closed => write!(f, "datasource closed"),
        }
    }
}

impl std::error::Error for SnapshotUnavailable {}
