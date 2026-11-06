//! A `&` argument naming a property that cannot hold what the callee may write.
//!
//! The other direction of the argument check. An ordinary argument is judged
//! value-against-parameter because the call reads it; a `&` parameter is also written, so the same
//! pair has a second question with the sides swapped. Nothing is wrong with the argument going
//! *in* -- an `array` is a fine `?array` -- which is why the read-side rule accepts it and why this
//! needs a code of its own.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_reference_parameter_wider_than_the_property_it_is_given() {
    check(
        r#"
//- /src/Holder.php
<?php
namespace App;

class Holder
{
    private array $tight = [];

    private ?array $loose = NULL;

    private function fill(?array &$data): void
    {
        $data = NULL;
    }

    private function fillTight(array &$data): void
    {
        $data = [];
    }

    public function run(): void
    {
        $this->fill($this->tight);
        $this->fill($this->loose);
        $this->fillTight($this->tight);
    }
}
"#,
        expect![[r#"
            22:21-22:33 error [incompatible-reference] App\Holder::$tight takes array, and &$data may write ?array into it
        "#]],
    );
}

#[test]
fn a_free_function_the_project_declares_is_judged_like_any_other() {
    // The stub exclusion is about what a signature *means*, not about the shape of the call: a
    // stub's `&` is the engine's calling convention, and code anybody wrote says with the same `&`
    // what it intends to put there. So a free function declared here is judged exactly as a method
    // is -- which is what keeps the exclusion from quietly becoming "free functions are exempt".
    //
    // The exclusion itself cannot be tested at this layer: a fixture has no standard library, so
    // `reset($this->items)` is an undefined function here rather than a stub by-reference. What it
    // is measured against is the corpora, where reading those signatures as writes was 57 of
    // WordPress's 58 findings.
    check(
        r#"
//- /src/helpers.php
<?php
namespace App;

function fill(?array &$data): void
{
    $data = NULL;
}

//- /src/Holder.php
<?php
namespace App;

class Holder
{
    private array $tight = [];

    public function run(): void
    {
        fill($this->tight);
    }
}
"#,
        expect![[r#"
            10:14-10:26 error [incompatible-reference] App\Holder::$tight takes array, and &$data may write ?array into it
        "#]],
    );
}
