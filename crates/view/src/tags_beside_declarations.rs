//! The `@property` and `@method` tags that name a member the class also declares in PHP.
//!
//! A class that serves a protected property through `__get` says what comes back with a tag above
//! the class: `@property-read string $scheme` over `protected $scheme`. WordPress's `Iri` does it
//! eleven times. The member table holds one row for the name and the declaration is the one that
//! wins it, so the tag was nobody's: a search for the property left it out, and a rename rewrote
//! the declaration and every read and left the tag naming a property that no longer existed --
//! which the next reader of the docblock takes for a magic one.

use phrost_model::TextRange;
use phrost_sema::MemberKind;

use crate::view::IndexView;

impl IndexView<'_> {
    /// Where the current file's docblock tags name the member `class` declares under `name`.
    ///
    /// Empty when the file does not hold the class. A tag that is the member's only declaration
    /// is found here too; it is the same range the member's own location gives.
    pub fn tags_naming(&self, class: &str, kind: MemberKind, name: &str) -> Vec<TextRange> {
        let Some(skeleton) = self
            .current_file()
            .and_then(|file| self.skeleton.file(file))
        else {
            return Vec::new();
        };
        skeleton
            .classes()
            .iter()
            .filter(|declared| declared.fqn().as_str().eq_ignore_ascii_case(class))
            .flat_map(|declared| declared.members())
            .filter(|member| member.from_docblock && member.kind == kind)
            .filter(|member| match kind {
                // Folded, as PHP compares a method's name; a property's is compared as written.
                MemberKind::Method => member.name.as_str().eq_ignore_ascii_case(name),
                _ => member.name.as_str() == name,
            })
            .map(|member| member.name_range)
            .collect()
    }
}
