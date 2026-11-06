//! Which files a sweep measures, and which positions inside them.

use std::path::PathBuf;

use phrost_model::SourceOrigin;

/// The files chosen for a sweep.
pub(crate) struct Sample {
    pub(crate) files: Vec<PathBuf>,
    /// How many user files were available to choose from, so the report can say what it skipped.
    pub(crate) considered: usize,
}

/// The largest user files, biggest first.
///
/// Largest rather than random, and user rather than vendor, because a budget is a claim about the
/// files a person actually edits and the ones that break it are the big ones. Deterministic too —
/// the same checkout picks the same files, so two runs are comparable and a regression is not a
/// resampling artefact.
pub(crate) fn largest_user_files(discovered: &[(PathBuf, SourceOrigin)], count: usize) -> Sample {
    let mut candidates = discovered
        .iter()
        .filter(|(_, origin)| origin.wants_full_analysis())
        .filter_map(|(path, _)| {
            let size = std::fs::metadata(path).ok()?.len();
            Some((size, path.clone()))
        })
        .collect::<Vec<_>>();

    let considered = candidates.len();
    // Path breaks size ties so the choice does not depend on directory order.
    candidates.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    candidates.truncate(count);

    Sample {
        files: candidates.into_iter().map(|(_, path)| path).collect(),
        considered,
    }
}
