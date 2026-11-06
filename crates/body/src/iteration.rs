//! Every `foreach` in a file, and what it is iterating.
//!
//! The sibling of [`subscript_sites`](crate::subscript_sites). A subscript needs the presence tests
//! taken out as well, because `isset($rows['k'])` is willing to find nothing there; a `foreach`
//! header reads its subject and nothing else. And there is no reason to keep one site per variable
//! -- two loops over one value are two places a guard has to go.
//!
//! What both need is the **built** set. `$out = null; $out[] = $pattern;` is how PHP code makes an
//! array out of nothing, and the variable's *type* never learns it: an element write does not
//! change what the assignment said. So a `foreach` over `$out` looked like a loop over `null` and
//! was reported as one, on code that has been shipping in WordPress for fifteen years. A name the
//! file ever builds that way is not judged here at all.

use phrost_hir::{FileHir, body::StmtData};
use phrost_model::TextRange;

use crate::{
    path::{ValuePath, path_of},
    scope::Scope,
    subscripts::write_targets,
};

/// One `foreach` header, as far as a reader asking "can this be iterated" is concerned.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IteratedSubject {
    /// The subject as written, which is what a report underlines and what a guard goes around.
    pub range: TextRange,
    /// The path to it, for the layer that can resolve one.
    pub subject: ValuePath,
}

/// Every `foreach` subject in the file, in source order.
pub fn iterated_subjects(hir: &FileHir) -> Vec<IteratedSubject> {
    let written = write_targets(hir);
    let mut found = Vec::new();
    for scope in Scope::every_scope(hir) {
        let body = scope.body();
        crate::walk::visit_every_statement(hir, hir.body(body).statements, &mut |hir, id| {
            let StmtData::Foreach { subject, .. } = hir.stmt(id) else {
                return;
            };
            let Some(path) = path_of(hir, *subject) else {
                return;
            };
            // A value this file ever fills a slot of holds an array by the time the loop runs,
            // whatever its assignment said -- see the module note.
            if written.builds(body, &path) {
                return;
            }
            found.push(IteratedSubject {
                range: hir.expr(*subject).range(),
                subject: path,
            });
        });
    }
    found.sort_by_key(|site| site.range.start());
    found
}
