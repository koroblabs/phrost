use std::fmt;

/// Identifies one source file for the whole lifetime of an index generation.
///
/// Everything downstream of discovery keys by this instead of by path or URI. Two reasons: a
/// `u32` is cheap to store in the hundreds of thousands of index rows, and a path string that
/// travels between layers eventually gets normalized differently in two places.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(u32);

impl FileId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn as_raw(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FileId({})", self.0)
    }
}

/// Where a file came from, which decides how much of it is worth analyzing.
///
/// This is the switch behind the memory contract: user code is indexed fully, everything else is
/// skeleton-only. It is a primitive rather than a project-layer detail because the index, the
/// analysis layer, and the test harness all need to reason about it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum SourceOrigin {
    /// First-party project code. Fully indexed.
    #[default]
    User,
    /// A composer dependency. Skeleton only, loaded lazily.
    Vendor,
    /// A stub describing a built-in or extension API. Skeleton only.
    Stub,
}

impl SourceOrigin {
    /// Whether bodies, flow, and diagnostics are worth computing for this file.
    pub fn wants_full_analysis(self) -> bool {
        matches!(self, Self::User)
    }
}
