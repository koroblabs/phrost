//! Enforces the workspace's allowed dependency edges.
//!
//! This is the mechanism the architecture rests on. If it is failing, do not weaken it to get
//! green — either the new edge is wrong, or it is right and belongs in the matrix with a reason.

#[test]
fn workspace_respects_declared_dependency_edges() {
    if let Err(report) = phrost_checks::dependency_rules::check() {
        panic!("{report}");
    }
}
