//! Pest: PHPUnit's runner under a syntax of functions and closures.
//!
//! A Pest test is `test('...', function () { $this->get('/'); })`, and `$this` there is a test case
//! Pest generates for the file, extending whatever class the suite configured -- a project's own
//! `Tests\TestCase`, nearly always, which is where `get()`, `actingAs()` and the project's helpers
//! are. `test()` declares `@param-closure-this TestCase` and can say no more, because which class is
//! configuration: `uses(TestCase::class)` in the file, or `uses(TestCase::class)->in('Feature')` --
//! `pest()->extend(...)` since Pest 3 -- in `tests/Pest.php`. Read as PHPUnit's own `TestCase`,
//! every such call in a suite was a missing method: 467 of Laravel.io's 686.

use std::path::Path;

use crate::{FrameworkProvider, rule::ConfiguredClosureThis};

#[derive(Debug)]
pub struct Pest;

impl FrameworkProvider for Pest {
    fn id(&self) -> &'static str {
        "pest"
    }

    /// The installed package, for the reason PHPUnit's provider gives.
    fn detect(&self, root: &Path) -> bool {
        root.join("vendor/pestphp/pest").is_dir()
    }

    fn configured_closure_this(&self) -> &'static [ConfiguredClosureThis] {
        &[ConfiguredClosureThis {
            declared: "PHPUnit\\Framework\\TestCase",
            functions: &["uses", "pest"],
            methods: &["extend", "extends", "use", "uses"],
            within: "in",
        }]
    }
}
