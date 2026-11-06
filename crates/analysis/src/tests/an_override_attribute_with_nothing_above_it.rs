//! PHP 8.3's `#[\Override]`, and what it counts as something to override.
//!
//! The attribute exists for one bug: a parent method renamed underneath a child. PHP refuses to
//! *load* the class when there is nothing above the declaration, so this is a fatal rather than a
//! lint, and the declaration it has to match is in another file.
//!
//! What counts was asked of PHP 8.5.10 rather than read off the RFC, and four of the answers are
//! not what the wording suggests: a concrete method in a used trait does not count while an
//! **abstract** one does; a `private` method on the parent does not; a constructor never does,
//! whatever the parent declares; and `__toString` always does, because PHP 8 makes every class
//! that declares it implement `Stringable` whether it says so or not.
//!
//! Measured after: 3 541 `#[\Override]` in psl, Symfony and Laravel, and **nothing reported**.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn what_php_refuses_to_load_is_reported() {
    check(
        r#"
//- /src/attributes.php
<?php

/** PHP's own, declared here because the fixture harness installs no stubs. */
final class Override {}

interface Stringable
{
    public function __toString(): string;
}

//- /src/app.php
<?php
namespace App;

class Parented
{
    public function real(): void {}

    private function hidden(): void {}
}

trait Gives
{
    public function given(): void {}
}

class Overrider extends Parented
{
    use Gives;

    #[\Override]
    public function notThere(): void {}

    #[\Override]
    public function given(): void {}

    #[\Override]
    public function hidden(): void {}

    #[\Override]
    public function __construct() {}
}
"#,
        expect![[r#"
            21:21-21:29 error [nothing-to-override] notThere() is #[\Override] and nothing above it declares that name
            24:21-24:26 error [nothing-to-override] given() is #[\Override] and nothing above it declares that name
            27:21-27:27 error [nothing-to-override] hidden() is #[\Override] and nothing above it declares that name
            30:21-30:32 error [nothing-to-override] __construct() is #[\Override] and a constructor, which PHP never counts as overriding
        "#]],
    );
}

/// And the four shapes PHP loads, which must stay silent.
#[test]
fn what_php_loads_is_left_alone() {
    check(
        r#"
//- /src/attributes.php
<?php

/** PHP's own, declared here because the fixture harness installs no stubs. */
final class Override {}

interface Stringable
{
    public function __toString(): string;
}

//- /src/app.php
<?php
namespace App;

interface Contract
{
    public function fromIface(): void;
}

class Parented
{
    public function real(): void {}
}

trait Needs
{
    abstract protected function required(): void;
}

class Overrider extends Parented implements Contract
{
    use Needs;

    #[\Override]
    public function real(): void {}

    #[\Override]
    public function fromIface(): void {}

    #[\Override]
    protected function required(): void {}

    #[\Override]
    public function __toString(): string
    {
        return '';
    }
}
"#,
        expect![""],
    );
}

/// A `#[\Override]` written inside a trait is not reported at all: whether it has something to
/// override depends on the class that uses the trait, and the declaration alone cannot say.
#[test]
fn an_override_inside_a_trait_is_judged_by_nobody_here() {
    check(
        r#"
//- /src/attributes.php
<?php

/** PHP's own, declared here because the fixture harness installs no stubs. */
final class Override {}

interface Stringable
{
    public function __toString(): string;
}

//- /src/app.php
<?php
namespace App;

trait Replaces
{
    #[\Override]
    public function m(): void {}
}
"#,
        expect![""],
    );
}

/// PHP 8.5 lets a property carry the attribute, under the method's rule: a parent's private property
/// does not count, nor does the class's own trait, while a parent that took the property from a
/// trait and an interface's hooked property both do. Every case was run against 8.5.10.
#[test]
fn a_property_with_nothing_above_it_is_reported_by_the_same_rule() {
    check(
        r#"
//- /src/attributes.php
<?php

/** PHP's own, declared here because the fixture harness installs no stubs. */
final class Override {}

//- /src/app.php
<?php
namespace App;

trait Carries
{
    public int $carried = 1;
}

class Base
{
    use Carries;

    public int $value = 1;
    private int $hidden = 1;
}

interface Labelled
{
    public string $label { get; }
}

class Child extends Base implements Labelled
{
    use Carries;

    #[\Override]
    public int $value = 2;
    #[\Override]
    public int $carried = 1;
    #[\Override]
    public string $label = '';
    #[\Override]
    public int $hidden = 2;
    #[\Override]
    public int $other = 3;
}

class Alone
{
    use Carries;

    #[\Override]
    public int $carried = 1;
}
"#,
        expect![[r#"
            33:16-33:23 error [nothing-to-override] $hidden is #[\Override] and nothing above it declares that property
            35:16-35:22 error [nothing-to-override] $other is #[\Override] and nothing above it declares that property
            43:16-43:24 error [nothing-to-override] $carried is #[\Override] and nothing above it declares that property
        "#]],
    );
}
