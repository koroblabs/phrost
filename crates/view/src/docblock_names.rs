//! How a file's docblocks spell the classes they name.
//!
//! Two conventions, and a project keeps to one. Drupal and Laravel write every class in a docblock
//! from the root -- `@param \Drupal\node\NodeInterface $node` -- and Drupal's standard requires it.
//! Symfony, WordPress and anything php-cs-fixer's Symfony set has been run over write the name the
//! code writes: `@throws LockStorageException` under a `use` line, `@param WP_Post $post` in a file
//! with no namespace at all.
//!
//! Completion wrote the first everywhere, which is right for the project it was measured on and
//! wrong for the other half of PHP. Asked at every name a docblock writes, cut in half: 97 of 104
//! in WordPress came back with a `\` the file never writes, and 18 of 22 in Symfony with a whole
//! path beside an import of the same class.
//!
//! Nothing declares which convention a project keeps, and the file says it. So the question is
//! asked of the file being written, by what its docblocks already hold -- and a file that says
//! nothing gets the rooted spelling, which is the one that is right wherever it lands.
//!
//! Measured after, on the same sites: WordPress 96 -> 7 and Symfony 18 -> 5 written otherwise than
//! the file had them, Laravel and Drupal unchanged at 2 and 8. What is left is the first class a
//! file's docblocks ever name, where the file has not said yet. The project as a whole has, and a
//! tally the index holds would answer for it; nothing has needed that.

use phrost_hir::{ImportKind, NameAnchor};

use crate::{comments::CommentPlace, view::IndexView};

impl IndexView<'_> {
    /// Whether the open file's docblocks name classes the way its code does, rather than from the
    /// root: more of them written as a name the file resolves than written with a leading `\`.
    ///
    /// A bare name that resolves to nothing is no evidence. A `@template` parameter is one, and so
    /// is the half-typed word this is being asked about. Nor is a name outside a type: the trait
    /// of `@use HasFactory<...>` repeats the `use` statement under it and is short in every
    /// Laravel model, whose types are all written from the root.
    pub fn docblocks_name_classes_short(&self) -> bool {
        let Some(open) = self.current() else {
            return false;
        };
        let written = || {
            open.hir()
                .name_refs()
                .map(|(_, written)| written)
                .filter(|written| written.in_docblock && written.kind == ImportKind::ClassLike)
        };
        // The rooted ones first, which is a count and reads nothing -- so the resolving below
        // stops at the first name past it.
        let rooted = written()
            .filter(|written| written.anchor == NameAnchor::FullyQualified)
            .count();
        written()
            .filter(|written| written.anchor != NameAnchor::FullyQualified)
            .filter(|written| {
                matches!(
                    self.comment_at(written.range.start()),
                    Some(CommentPlace::TypeTag)
                )
            })
            .filter(|written| {
                self.resolve_name(
                    written.range.start(),
                    written.path.as_str(),
                    written.anchor,
                    ImportKind::ClassLike,
                )
                .is_some()
            })
            .nth(rooted)
            .is_some()
    }
}
