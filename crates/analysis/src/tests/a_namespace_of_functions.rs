//! `use Psl\Crypto\Internal;` where the namespace holds no class.
//!
//! A namespace is declared by nothing of its own -- what says one exists is that something sits
//! under it -- and the check asked only about *classes*. A library written as namespaced functions
//! puts none there: `Internal\call_sodium(...)` beside `use Psl\Crypto\Internal;` is how PHP spells
//! a module in that style, and every such import read as a class that does not exist.
//!
//! 211 findings on one such library, all of them `undefined-type` on correct code, and none on four
//! framework corpora -- a framework writes `use function` instead, which is why it took a fifth
//! dialect to see it.
//!
//! The import of a namespace that really holds nothing stays a finding: the `use` line then names
//! neither a class nor a namespace, and that is the moved-class defect the check exists for. The
//! call through it is reported too, by the check that owns that question.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_namespace_is_recognised_by_whatever_is_declared_under_it() {
    check(
        r#"
//- /src/Crypto/Internal/call.php
<?php
namespace App\Crypto\Internal;

function call(): void {}

//- /src/Crypto/Holder.php
<?php
namespace App\Crypto;

class Holder {}

//- /src/Use.php
<?php
namespace App;

use App\Crypto;
use App\Crypto\Internal;
use App\Nothing\Here;

class Reader
{
    public function run(): void
    {
        Internal\call();
        new Crypto\Holder();
        Here\anything();
    }
}
"#,
        expect![[r#"
            6:5-6:21 error [undefined-type] Undefined type App\Nothing\Here
            14:9-14:22 error [undefined-function] Undefined function Here\anything()
        "#]],
    );
}
