//! PHPUnit: a framework that runs a project's code and catches everything it throws.
//!
//! The smallest provider here, and it contributes one fact. A `@throws` documents a contract for
//! *callers*, and a test method has no caller in the project: PHPUnit invokes each one by
//! reflection and catches every `Throwable` -- that catch is not error handling, it is how a failed
//! assertion becomes a failed test. `Assert::assertSame()` is documented
//! `@throws ExpectationFailedException` for exactly that reason.
//!
//! So `doc-missing-throws` demanded, on every test method of a project whose standard puts a
//! docblock on every method, a tag describing the test runner's own control flow: **1 841 of 2 655
//! findings on one real checkout, 1 513 of them naming a PHPUnit class**. The rule is right and the
//! subject was wrong.
//!
//! Not a list of exception classes, which is a policy a project has to choose and which PHPStan
//! and PhpStorm both make configuration. One base class, and what it means is a fact about PHPUnit.

use std::path::Path;

use crate::{FrameworkProvider, rule::StandInMethod};

#[derive(Debug)]
pub struct PhpUnit;

impl FrameworkProvider for PhpUnit {
    fn id(&self) -> &'static str {
        "phpunit"
    }

    /// The installed package, not a `phpunit.xml`.
    ///
    /// A configuration file is a statement of intent and a `vendor/` directory is a statement of
    /// fact -- and the fact is what matters, because the contribution below is about a class that
    /// has to be *resolvable* for anything to inherit it. A project with a `phpunit.xml.dist` and
    /// no install has no `TestCase` for this to be about.
    fn detect(&self, root: &Path) -> bool {
        root.join("vendor/phpunit/phpunit").is_dir()
    }

    /// Every class PHPUnit runs inherits `TestCase`, which is where the runner's contract is.
    ///
    /// One name and not four: `Assert` is a base of `TestCase` rather than a thing projects extend
    /// on its own, and the mock and event exceptions arrive through calls made *from* a test method
    /// rather than from a class of their own.
    fn runs_and_catches(&self) -> &'static [&'static str] {
        &["PHPUnit\\Framework\\TestCase"]
    }

    /// The fluent builder, which is split across interfaces the concrete class implements together.
    ///
    /// `Stub::method()` declares it returns `InvocationStubber`, and `with()` is on `MethodNameMatch`
    /// -- two interfaces of one `InvocationMocker`. So `->method('x')->with(...)` is how every
    /// PHPUnit test in the world is written and is `undefined-method` by the declarations alone: 38
    /// of them in Laravel's test suite the moment a mock receiver became typed at all.
    ///
    /// The interfaces and not `InvocationMocker`, because what a caller holds is whichever one the
    /// signature named.
    fn dynamic_members(&self) -> &'static [&'static str] {
        &[
            "PHPUnit\\Framework\\MockObject\\Stub",
            "PHPUnit\\Framework\\MockObject\\Stub\\Stub",
            "PHPUnit\\Framework\\MockObject\\Builder\\InvocationStubber",
            "PHPUnit\\Framework\\MockObject\\InvocationStubber",
        ]
    }

    /// `$mock->method('find')` and `$mock->expects($this->once())->method('find')`: the string is a
    /// method of whatever the mock stands in for.
    ///
    /// Gated on the value rather than on the call, because the call is not declared anywhere a
    /// reader can find: PHPUnit 12 adds `method()` to each generated mock through a trait, and
    /// `Stub` itself declares nothing. What every mock's type does say is `Stub`, beside the subject.
    fn stand_in_methods(&self) -> &'static [StandInMethod] {
        &[StandInMethod {
            stand_in: "PHPUnit\\Framework\\MockObject\\Stub",
            method: "method",
            at: 0,
            through: &["expects"],
        }]
    }
}
