//! Twig, the template engine: what its extension API says about the filters, functions and tests
//! a template calls by name.
//!
//! A library rather than a framework, and a provider because what it contributes is a fact about
//! the library that holds in every project using it -- Drupal, Symfony, Craft, or one that builds its
//! own `Twig\Environment`. An extension declares each callable by constructing it in a method Twig
//! asks for -- `new TwigFilter('upper', 'strtoupper')` in `getFilters()` -- and that construction is
//! the only statement anywhere that the filter exists and what runs it. Twig's own are in its
//! `CoreExtension`, under `vendor/`, which is why lowering reads these methods in every file.

use std::path::Path;

use crate::{
    FrameworkProvider,
    rule::{ConstructedEntry, TwigCallables},
};

/// The catalogs, by names this provider chose.
const FILTERS: &str = "twig filter";
const FUNCTIONS: &str = "twig function";
const TESTS: &str = "twig test";

static CONSTRUCTED: &[ConstructedEntry] = &[
    ConstructedEntry {
        method: "getFilters",
        class: "Twig\\TwigFilter",
        catalog: FILTERS,
    },
    ConstructedEntry {
        method: "getFunctions",
        class: "Twig\\TwigFunction",
        catalog: FUNCTIONS,
    },
    ConstructedEntry {
        method: "getTests",
        class: "Twig\\TwigTest",
        catalog: TESTS,
    },
];

#[derive(Debug)]
pub struct Twig;

impl FrameworkProvider for Twig {
    fn id(&self) -> &'static str {
        "twig"
    }

    /// The installed package: an extension's classes have to be there for anything to construct
    /// them.
    fn detect(&self, root: &Path) -> bool {
        root.join("vendor/twig/twig").is_dir()
    }

    fn constructed_entries(&self) -> &'static [ConstructedEntry] {
        CONSTRUCTED
    }

    fn twig_callables(&self) -> Option<TwigCallables> {
        Some(TwigCallables {
            filters: FILTERS,
            functions: FUNCTIONS,
            tests: TESTS,
        })
    }
}
