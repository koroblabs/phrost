//! A base class calling something its subclasses implement.
//!
//! The template method, written the way PHP 4 had to write it: there was no `abstract`, so the
//! pattern is a plain class with `$this->_exec()` in it and the declaration one level down.
//! WordPress still ships it -- `ftp_base` is **62 of that checkout's 68** `undefined-method`
//! reports, and every one of them is a call that runs, because `$this` there is an `ftp_pure` or
//! an `ftp_sockets`.
//!
//! Two halves keep it from being an exemption. **Only through `$this`**, `self::` or `static::`:
//! a *variable* typed as the base is a value whose class nothing here saw chosen, and widening to
//! that would silence the check on every extensible class there is. **And only where a subtype
//! really declares the name**: a typo has none, so it is still reported -- which is what turned
//! that WordPress file from 63 reports into the one its own source calls a bug
//! (`// TODO: This should probably be glob_regexp()`).

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_this_call_a_subclass_declares_is_not_missing_and_a_typo_still_is() {
    check(
        r#"
//- /src/Pure.php
<?php
namespace App;

class Pure extends Base
{
    public function exec(string $command): bool
    {
        return TRUE;
    }
}

//- /src/Base.php
<?php
namespace App;

class Base
{
    public function run(): bool
    {
        // Declared by `Pure` below, which is what `$this` is whenever this runs.
        return $this->exec('TYPE I');
    }

    public function mistyped(): bool
    {
        return $this->exce('TYPE I');
    }
}
"#,
        expect![[r#"
            14:23-14:27 error [undefined-method] Undefined method App\Base::exce()
        "#]],
    );
}

#[test]
fn a_variable_typed_as_the_base_is_not_covered_by_it() {
    // The half that keeps the check alive. A value typed `Base` is some class nothing here saw
    // chosen, and reading a subtype's declaration as its own would excuse every extensible class.
    check(
        r#"
//- /src/Pure.php
<?php
namespace App;

class Pure extends Base
{
    public function exec(string $command): bool
    {
        return TRUE;
    }
}

//- /src/Base.php
<?php
namespace App;

class Base
{
}

//- /src/Runner.php
<?php
namespace App;

class Runner
{
    public function go(Base $base): bool
    {
        return $base->exec('TYPE I');
    }
}
"#,
        expect![[r#"
            8:23-8:27 error [undefined-method] Undefined method App\Base::exec()
        "#]],
    );
}
