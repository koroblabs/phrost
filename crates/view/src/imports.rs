//! Where a `use` line for a name would go.
//!
//! What completion needs to offer a short name instead of a fully qualified one. Until now it wrote
//! the qualified form -- `\App\Nested\Repo` in the middle of a statement -- which is always correct
//! and never what anybody types.
//!
//! The judgement here is when *not* to offer one. An import changes what a short name means for the
//! whole file, so adding `use App\Repo` where `Repo` already means something else silently rewrites
//! every existing use of it. That case has to be recognised and refused, and refusing is what makes
//! the qualified spelling still worth keeping as the fallback.

use phrost_hir::{ImportKind, NameAnchor};
use phrost_model::{TextSize, short_name};

use crate::view::IndexView;

/// A `use` line to add when a completion item is accepted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportEdit {
    /// Where to insert, always at the end of an existing line so nothing already written moves.
    pub at: TextSize,
    /// The text to insert, its own leading newline included.
    pub text: String,
}

impl IndexView<'_> {
    /// The `use` line that would make `fqn` reachable by its short name at `offset`.
    ///
    /// `None` when the file needs no import -- the short name already means this -- and also when it
    /// cannot have one, because the short name already means something else. The two are different
    /// situations with the same answer here, and the caller tells them apart by asking what the
    /// short name resolves to.
    pub fn import_for(&self, offset: TextSize, fqn: &str, kind: ImportKind) -> Option<ImportEdit> {
        let short = short_name(fqn);

        // Anything the short name already resolves to settles it: this symbol, and no import is
        // needed; or another one, and no import is possible.
        if self
            .resolve_name(offset, short, NameAnchor::Relative, kind)
            .is_some()
        {
            return None;
        }
        // And anything a `use` line already binds it to, declared or not. An import of a class the
        // index cannot see -- a dependency not installed, an extension with no stub -- resolves to
        // nothing above and still owns the name: a second line for it is PHP's "cannot use ...
        // because the name is already in use", a fatal error for the whole file. Measured on
        // Symfony's own source with no `vendor/`: `extends Test|` under
        // `use PHPUnit\Framework\TestCase;` offered the framework bundle's `TestCase` with a
        // second `use` line for it, at eight of twenty-one `extends` sites.
        if self.binds_by_import(offset, short, kind) {
            return None;
        }
        self.import_placement(offset, fqn, kind)
    }

    /// Whether a `use` line in the namespace at `offset` binds `local` in the table `kind` names.
    ///
    /// By the line and not by what it resolves to: the name is taken as soon as it is written.
    fn binds_by_import(&self, offset: TextSize, local: &str, kind: ImportKind) -> bool {
        let Some(open) = self.current() else {
            return false;
        };
        let hir = open.hir();
        let namespace = hir.namespace_at(offset);
        hir.imports()
            .filter(|(_, import)| import.namespace == namespace && import.kind == kind)
            .any(|(_, import)| match kind {
                // Classes and functions are named without regard to case, and so is what a `use`
                // line binds them to.
                ImportKind::ClassLike | ImportKind::Function => {
                    import.local.as_str().eq_ignore_ascii_case(local)
                }
                ImportKind::Constant => import.local.as_str() == local,
            })
    }

    /// The same line, placed, without asking whether the file needs one.
    ///
    /// Split out because one caller knows something this cannot: a file rename is told what the
    /// project will look like *after* the move, and every question about what a short name
    /// resolves to is answered against the index as it is now. `Repo` written bare beside
    /// `class Repo` resolves today and stops resolving the moment the class leaves the namespace,
    /// which is exactly the import that has to be written and exactly the one the guard above
    /// refuses.
    pub fn import_placement(
        &self,
        offset: TextSize,
        fqn: &str,
        kind: ImportKind,
    ) -> Option<ImportEdit> {
        let open = self.current()?;
        let hir = open.hir();
        let short = short_name(fqn);

        // A name in the global namespace has no `use` line to write: `use Throwable;` is legal and
        // pointless, and in the global namespace itself it is not even that.
        if short == fqn {
            return None;
        }

        let namespace = hir.namespace_at(offset);
        let text = open.text();

        // After the last import of the same kind, so class imports stay together and a `use
        // function` line does not land in the middle of them. Then after any import at all, then
        // after the namespace declaration, then after the opening tag.
        let anchor = hir
            .imports()
            .filter(|(_, import)| import.namespace == namespace)
            .filter(|(_, import)| import.kind == kind)
            .map(|(_, import)| import.full_range.end())
            .max()
            .or_else(|| {
                hir.imports()
                    .filter(|(_, import)| import.namespace == namespace)
                    .map(|(_, import)| import.full_range.end())
                    .max()
            })
            .or_else(|| {
                namespace
                    .map(|id| hir.namespace(id).name_range.end())
                    .map(|end| line_end(text, end))
            })
            .unwrap_or_else(|| line_end(text, TextSize::from(0)));

        Some(ImportEdit {
            at: anchor,
            text: format!("\n{}", use_line(fqn, kind)),
        })
    }

    /// Whether the file already writes a `use` line for exactly this name.
    ///
    /// Asked before one is added, and it cannot be replaced by asking what the short name
    /// resolves to: in the same namespace both answer yes, and only one of them survives the
    /// class moving out of it.
    pub fn imports_path(&self, fqn: &str, kind: ImportKind) -> bool {
        let fqn = fqn.trim_start_matches('\\');
        self.current().is_some_and(|open| {
            open.hir()
                .imports()
                .any(|(_, import)| import.kind == kind && import.path.as_str() == fqn)
        })
    }
}

/// The statement to write, in the symbol table the name belongs to.
fn use_line(fqn: &str, kind: ImportKind) -> String {
    match kind {
        ImportKind::ClassLike => format!("use {fqn};"),
        ImportKind::Function => format!("use function {fqn};"),
        ImportKind::Constant => format!("use const {fqn};"),
    }
}

/// The end of the line `at` sits on, so an insertion lands after it and moves nothing.
fn line_end(text: &str, at: TextSize) -> TextSize {
    let from = usize::from(at).min(text.len());
    let end = text[from..]
        .find('\n')
        .map_or(text.len(), |offset| from + offset);
    TextSize::try_from(end).unwrap_or(at)
}
