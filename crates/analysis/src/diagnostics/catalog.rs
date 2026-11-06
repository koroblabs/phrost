//! An id nothing registers.
//!
//! `\Drupal::service('not')` is a fatal error the moment it runs, and it is the kind of mistake
//! nothing else can catch: the string is not a name PHP has, so no compiler, no linter and no type
//! checker has anything to say about it. Until a catalog existed, neither did this.
//!
//! # Only where the catalog is complete
//!
//! A missing key means two different things, and only whoever filled the catalog knows which. Drupal
//! loads its container from a list of files that can be read in full, so an id missing from it does
//! not exist -- measured at 372 of 372 service ids on a real project. Its route names are not like
//! that: `entity.node.canonical` is generated at runtime from an entity type's link templates and
//! declared in no file, so 19 of 101 route literals are absent from a catalog that read every
//! `*.routing.yml` there is. Reporting those would light up correct code nineteen times.
//!
//! So the report is gated on the catalog saying it holds everything, and that claim is made at the
//! framework seam where the loading rule is known.
//!
//! # And where it is not, a near miss still is a mistake
//!
//! Five of the eight catalogs are open, and between them they hold most of the keys a project
//! writes: 1 073 routes, 763 extensions, 422 fields, 262 permissions, 82 render elements. Saying
//! nothing about any of them left the mistakes that hurt most unreported, and they hurt because they
//! are *runtime* mistakes -- a misspelt route is a white screen, a misspelt permission is access
//! silently refused, a misspelt `#type` is an exception while a form is built.
//!
//! What separates the two cases is that **a typo nearly hits**. `entity.node.canonicl` is one edit
//! from a real route; a route generated at runtime from an entity type's link templates is near
//! nothing, and neither is a field a bundle attached or a project's own `'#type' => 'Float'`
//! borrowing the `#` convention. So an open catalog reports a key that is a plausible misspelling of
//! one it holds, and stays silent about a key that is merely absent -- see
//! `ReturnContracts::nearest_catalog_key` for the bound and why it scales with length.
//!
//! A warning rather than an error there, and the difference is honest: a closed catalog *proves* the
//! line cannot run, and this says the line looks like a mistake.

use phrost_view::IndexView;

use super::{Check, Diagnostic, Severity};

pub(crate) const UNKNOWN_CATALOG_KEY: &str = "unknown-catalog-key";

pub(super) struct UnknownCatalogKey;

impl Check for UnknownCatalogKey {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        out.extend(view.catalog_uses().iter().filter_map(reported));
    }
}

/// What to say about one key a catalog does not hold, and whether to say anything.
///
/// Its own function because a key is written in two languages. A framework declares its services,
/// routes and permissions in configuration files and *reads* them from PHP, and both spellings are
/// the same mistake with the same three answers -- so the decision is made once and the second
/// caller passes what it found. Two constructors of one rule is how one of them comes to be missing
/// what the other has.
pub(crate) fn reported(used: &phrost_view::CatalogUse) -> Option<Diagnostic> {
    if used.known {
        return None;
    }

    let (severity, message) = match (used.closed, &used.nearest) {
        // The container throws on an unregistered id, so this is a line that cannot run
        // rather than one that might -- whether or not anything is near it.
        (true, None) => (
            Severity::Error,
            format!("Nothing registers the {} `{}`", used.catalog, used.key),
        ),
        (true, Some(near)) => (
            Severity::Error,
            format!(
                "Nothing registers the {} `{}`. Did you mean `{near}`?",
                used.catalog, used.key
            ),
        ),
        // An open catalog cannot say a key does not exist. It can say this one looks like a
        // misspelling of one that does, which is a different claim and a weaker one.
        (false, Some(near)) => (
            Severity::Warning,
            format!(
                "No {} is registered as `{}`. Did you mean `{near}`?",
                used.catalog, used.key
            ),
        ),
        (false, None) => return None,
    };

    Some(Diagnostic {
        range: used.range,
        severity,
        code: UNKNOWN_CATALOG_KEY,
        message,
        unnecessary: false,
        deprecated: false,
    })
}
