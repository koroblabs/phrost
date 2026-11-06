//! Mockery: a library whose values answer names nothing declares.
//!
//! One fact, and the smallest provider here. `Mockery\MockInterface` extends `LegacyMockInterface`
//! and between them they declare a handful of methods; what a test calls on a mock is whatever the
//! *mocked* class has, answered by a generated subclass at runtime. `__call` is PHP's own way of
//! saying that and it cannot be used here: it is a class mechanism, and what a signature hands a
//! caller is the interface.
//!
//! So `$mock->shouldReceive('x')` resolved and `$mock->is(...)` did not, and the difference was
//! which of the two the interface happened to declare -- 22 findings in Laravel's test suite the
//! moment a mock receiver became typed at all. PHPStan ships `phpstan-mockery` for the same reason.

use std::path::Path;

use crate::FrameworkProvider;

#[derive(Debug)]
pub struct Mockery;

impl FrameworkProvider for Mockery {
    fn id(&self) -> &'static str {
        "mockery"
    }

    /// The installed package, for the reason the PHPUnit provider gives: the contribution below is
    /// about a class that has to be resolvable for anything to be typed as it.
    fn detect(&self, root: &Path) -> bool {
        root.join("vendor/mockery/mockery").is_dir()
    }

    fn dynamic_members(&self) -> &'static [&'static str] {
        &[
            "Mockery\\MockInterface",
            "Mockery\\LegacyMockInterface",
            "Mockery\\ExpectationInterface",
            "Mockery\\HigherOrderMessage",
        ]
    }
}
