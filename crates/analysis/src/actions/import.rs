//! Adding the `use` line a name is missing.
//!
//! The commonest thing anybody wants done to a PHP file, and the one the index is already set up
//! for: completion has been writing these since it learned to offer a short name, so the whole of
//! the judgement about *where* a `use` goes and *when* one is safe lives in the view already.
//!
//! What is added here is the other direction. Completion knows the class and needs the line;
//! this knows a name that resolves to nothing and has to find the classes it could have meant.

use phrost_model::short_name;
use phrost_view::ImportKind;

use super::{ActionKind, ActionSubject, CodeAction, Offer, SubjectSites, TextEdit};
use crate::diagnostics::name::UNDEFINED_TYPE;

pub(super) struct ImportClass;

/// At most this many offers for one name.
///
/// `Node` is declared eight times over in a Drupal checkout -- core's entity, a test double, two
/// contrib modules -- and a menu longer than a glance is one nobody reads. The list is ranked, so
/// the ones cut are the ones furthest from the file asking.
const CANDIDATES: usize = 8;

/// How far down the ranked search to look for exact short-name matches.
///
/// The search is by *prefix*, so asking for `Node` also returns `NodeType`, `NodeAccessControl` and
/// the rest. Only the exact matches can be imported under the name as written, so the scan has to
/// be wider than the number of offers it produces.
const SCANNED: usize = 200;

impl Offer for ImportClass {
    fn kind(&self) -> ActionKind {
        ActionKind::QuickFix
    }

    fn subject(&self) -> ActionSubject {
        ActionSubject::WrittenName
    }

    /// A `use` line with one candidate is what the person would have typed; with several, nothing
    /// is preferred and so nothing is applied.
    fn unattended(&self) -> bool {
        true
    }

    fn run(&self, sites: &SubjectSites<'_, '_>, out: &mut Vec<CodeAction>) {
        let view = sites.view();
        // A parse break above the name means recovery may have dropped the `use` block, and then
        // every import in the file looks missing -- so this would offer to add one that is already
        // written, and adding it twice is a fatal error. An offer that is asked for can afford to
        // check; a diagnostic that is pushed does the same thing positionally.
        let broke_at = view
            .syntax_problems()
            .iter()
            .map(|problem| problem.range.start())
            .min();

        for reference in sites.names() {
            if reference.resolved.is_some() || reference.in_docblock {
                continue;
            }
            if broke_at.is_some_and(|broke| broke < reference.range.end()) {
                continue;
            }
            // A qualified name needs its path rewritten as well as a line added -- `Entity\Node`
            // wants `use Drupal\node\Entity;` and the written text left alone, which is a different
            // edit with a different way of being wrong. Only a bare name is offered.
            if reference.path.as_str().contains('\\') {
                continue;
            }

            let mut offers = Vec::new();
            for location in view.search_symbols(reference.path.as_str(), SCANNED) {
                if offers.len() == CANDIDATES {
                    break;
                }
                if !location.kind.is_class_like() {
                    continue;
                }
                // The search matches a prefix; only an exact short name can be imported and then
                // written as it already is.
                if !short_name(location.fqn.as_str()).eq_ignore_ascii_case(reference.path.as_str())
                {
                    continue;
                }
                let Some(edit) = view.import_for(
                    reference.range.start(),
                    location.fqn.as_str(),
                    ImportKind::ClassLike,
                ) else {
                    continue;
                };

                offers.push(CodeAction {
                    // The fully qualified name, because that is the whole of what distinguishes one
                    // offer from another: eight entries reading "Import Node" is one entry.
                    title: format!("Import {}", location.fqn),
                    edits: vec![TextEdit::insert(edit.at, edit.text)],
                    preferred: false,
                    kind: ActionKind::QuickFix,
                    fixes: Some(UNDEFINED_TYPE),
                });
            }

            // Shallowest namespace first, which is the presentation decision this layer owns. The
            // search ranks project code above dependencies and then alphabetically, and among four
            // classes called `NodeType` alphabetical order puts a contrib module's plugin above the
            // entity everybody means. A shorter qualified name is not a guess about intent -- it is
            // the more central declaration, and a menu has to be in some order.
            offers.sort_by(|left, right| {
                left.title
                    .len()
                    .cmp(&right.title.len())
                    .then_with(|| left.title.cmp(&right.title))
            });

            // Preferred only when there is nothing to choose between. A keystroke that applies the
            // single fix should apply it; one that picks the first of eight classes named `Node`
            // has picked at random.
            if let [only] = offers.as_mut_slice() {
                only.preferred = true;
            }
            out.append(&mut offers);
        }
    }
}
