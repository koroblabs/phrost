//! A call written on a value whose type is a union of classes, and the call after it.
//!
//! `@var Thing|MockObject` is how every PHPUnit mock is declared and it is not a test idiom only:
//! a union of two classes that share no ancestor is an ordinary PHP type. Asking it for a member
//! *worked* -- goto and hover both answered, because `owner_of_member` walks a four-rung ladder and
//! its first rung is "the operand that declares this name".
//!
//! What did not work is the hop after it. Two other readers had their own copy of that ladder with
//! two of the four rungs -- a single class, and an intersection's first declaring operand -- so a
//! union whose operands share no ancestor reduced to a common ancestor, found none, and gave up.
//! The member resolved and the chain died one step later.
//!
//! `$mock->expects($this->any())->method('x')->willReturn($y)` is the shape, and it stopped after
//! `expects`. Against intelephense on a real checkout it was **11 of the 17 sites** where that
//! server answered and this one did not, in goto and in hover alike; closing it took both readers
//! to `owner_of_member` and left 6.

use expect_test::expect;

use super::{Query, check_view};

const PROJECT: &str = r#"
//- /src/Mocks.php
<?php
namespace App;

interface Thing
{
    public function name(): string;
}

class Stub
{
    public function step(): Chained {}
}

class Chained
{
    public function last(): Thing {}
}

interface Mocker
{
    public function expects(): Stub;
}

class Test
{
    /**
     * Two classes sharing no ancestor, which is what a mock is declared as.
     *
     * @var \App\Thing|\App\Mocker
     */
    protected $both;

    /**
     * One class, the shape that always worked.
     *
     * @var \App\Mocker
     */
    protected $only;

    public function run(Thing|Mocker $native): void
    {
        $viaDocblock = $this->both->expects()->step();
        $viaNative = $native->expects()->step();
        $viaOne = $this->only->expects()->step();
        $twoHops = $this->both->expects()->step()->last();
        $theOperandsOwnMember = $this->both->name();

        $v@viaDocblock@iaDocblock;
        $v@viaNative@iaNative;
        $v@viaOne@iaOne;
        $t@twoHops@woHops;
        $t@theOperandsOwnMember@heOperandsOwnMember;
    }
}
"#;

#[test]
fn a_call_on_a_union_carries_its_return_into_the_next_call() {
    check_view(
        PROJECT,
        &[
            Query::variable_type("through a docblock union", "viaDocblock"),
            Query::variable_type("through a native union", "viaNative"),
            Query::variable_type("through one class, unchanged", "viaOne"),
            Query::variable_type("two hops past the union", "twoHops"),
            Query::variable_type("a member of the other operand", "theOperandsOwnMember"),
        ],
        expect![[r#"
            through a docblock union
            - Chained
            through a native union
            - Chained
            through one class, unchanged
            - Chained
            two hops past the union
            - Thing
            a member of the other operand
            - string
        "#]],
    );
}
