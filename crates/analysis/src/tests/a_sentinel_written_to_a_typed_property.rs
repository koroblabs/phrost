//! A value that may be `false` written into a typed property that does not take one.
//!
//! `$this->content = fread($handle, 8)` -- a `string|false` into a `string`. Weak mode writes `""`,
//! so PHP accepts it and `incompatible-assignment`, which asks what PHP rejects, accepted it too;
//! the argument side has had `possibly-false-argument` all along, and PHPStan reports the write at
//! level 8 as `assign.propertyType`. A null is the judgement's report and is not said twice. A
//! property that takes `false`, a guarded write and an `@property` tag (written to `__set`, which
//! PHP enforces nothing about) are silent.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_sentinel_written_to_a_property_that_cannot_take_it() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

/** @return string|false */
function readIt() {}

function maybe(): ?string { return null; }

/** @property string $magic */
class Stream
{
    private string $content = '';
    private ?string $label = null;
    private string|false $raw = false;

    public function fill(): void
    {
        $this->content = readIt();
        $this->content = false;
        $this->content = maybe();
        $this->label = maybe();
        $this->raw = readIt();
        $guarded = readIt();
        if ($guarded !== false) {
            $this->content = $guarded;
        }
        $this->magic = readIt();
    }
}
"#,
        expect![[r#"
            18:26-18:34 warning [possibly-false-assignment] App\Stream::$content takes string, and this may be false
            19:26-19:31 warning [false-assignment] Cannot write false to App\Stream::$content, which takes string
            20:26-20:33 error [incompatible-assignment] App\Stream::$content takes string, and this is ?string
        "#]],
    );
}
