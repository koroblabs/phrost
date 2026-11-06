//! Reaching for a declaration that says not to.
//!
//! The finding is about *callers*, which is what the two neighbourhood rules are for. A deprecated
//! declaration may use deprecated things -- that is what a shim is -- and a class keeping its own
//! deprecated member working is not a caller of one: Laravel's `OutputStyle::write()` assigns
//! `$this->newLineWritten` on every line it prints, which is how the property stays true for
//! whoever reads it. Without those two rules, WordPress reported 194 uses where 118 are somebody's.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_caller_is_told_and_the_declaration_itself_is_not() {
    check(
        r#"
//- /src/old.php
<?php
namespace App;

/**
 * @deprecated 2.0 use Fresh instead
 */
class Old
{
    /** @deprecated */
    public const LIMIT = 10;

    /** @deprecated */
    public int $count = 0;

    #[\Deprecated(message: 'use open()', since: '2.0')]
    public function lock(): bool
    {
        return true;
    }

    public function fine(): bool
    {
        // Its own business: a class keeping a deprecated member working is the shim.
        return $this->count > 0 && $this->lock();
    }
}

class Fresh {}

/** @deprecated use open() */
function legacy(): bool
{
    // A shim may reach for what it replaced.
    return (new Old())->lock();
}

function current(): bool
{
    return true;
}

//- /src/uses.php
<?php
namespace App;

/**
 * A sentence naming {@see Old} is prose about the deprecation, not a use of it.
 */
class Caller
{
    public function run(): bool
    {
        $old = new Old();

        return $old->lock()
            && $old->count > 0
            && Old::LIMIT > 0
            && legacy()
            && current();
    }
}
"#,
        expect![[r#"
            11:20-11:23 warning [deprecated-use] The class App\Old is deprecated
            13:22-13:26 warning [deprecated-use] The method App\Old::lock is deprecated
            14:22-14:27 warning [deprecated-use] The property App\Old::count is deprecated
            15:16-15:19 warning [deprecated-use] The class App\Old is deprecated
            15:21-15:26 warning [deprecated-use] The constant App\Old::LIMIT is deprecated
            16:16-16:22 warning [deprecated-use] The function App\legacy is deprecated
        "#]],
    );
}

/// An inherited member is still somebody else's to change, and a subclass is somebody else.
#[test]
fn a_subclass_reaching_for_an_inherited_deprecated_member_is_told() {
    check(
        r#"
//- /src/base.php
<?php
namespace App;

class Base
{
    /** @deprecated */
    protected array $json = [];

    protected array $fresh = [];
}

//- /src/child.php
<?php
namespace App;

class Child extends Base
{
    public function fill(): void
    {
        $this->json['a'] = 1;
        $this->fresh['a'] = 1;
    }
}
"#,
        expect![[r#"
            8:16-8:20 warning [deprecated-use] The property App\Base::json is deprecated
        "#]],
    );
}
