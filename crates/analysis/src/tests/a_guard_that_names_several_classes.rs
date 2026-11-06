//! What `$x instanceof A || $x instanceof B` proves, and where it proved nothing.
//!
//! The rule and its vocabulary both existed: `one_of_instanceof` reads the `||` whole,
//! `Proved::OneOfClasses` carries it, and the resolution filters a declared union down to the
//! alternatives the guard named. It worked for a value whose declaration already listed them, and
//! for every other value it produced nothing -- because filtering nothing leaves nothing, and a
//! `mixed` or an unhinted parameter lists nothing to filter.
//!
//! That is the commonest way the guard is written, and three fixtures of a borrowed corpus are that
//! one gap. The half that was missing is the one `also_of_class` has had all along for a single
//! class: *nothing named before, so the guard is the whole answer*. Two constructors of one rule,
//! and only one of them knowing it -- in two places at once, the helper and the resolution arm above
//! it, which is why the unhinted parameter needed both.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn an_or_of_two_instanceof_proves_the_union_whatever_the_declaration_said() {
    // Four declarations of one guard. The union is the case that always worked; `mixed` and the
    // unhinted parameter are the two that did not, and they failed for different reasons one layer
    // apart -- `mixed` resolves to something the filter cannot narrow, and `$plain` resolves to
    // nothing at all, which the arm treated as a reason to give up.
    check(
        r#"
//- /src/app.php
<?php

class Baz {}

class Boo {}

function declared(Baz|Boo|int $union, mixed $any, $plain): void
{
    if ($union instanceof Baz || $union instanceof Boo) {
        $un@union@ion;
    }
    if ($any instanceof Baz || $any instanceof Boo) {
        $a@any@ny;
    }
    if ($plain instanceof Baz || $plain instanceof Boo) {
        $pl@plain@ain;
    }
}
"#,
        &[
            Query::hover("a declaration that lists them", "union"),
            Query::hover("a declaration that says nothing", "any"),
            Query::hover("no declaration at all", "plain"),
        ],
        expect![[r#"
            a declaration that lists them
            - code: Baz|Boo $union
            - note: narrowed by a guard
            a declaration that says nothing
            - code: Baz|Boo $any
            - note: narrowed by a guard
            no declaration at all
            - code: Baz|Boo $plain
            - note: narrowed by a guard
        "#]],
    );
}

/// The same union reached through `assert`, and the single guard that must not become one.
///
/// `assert($f instanceof Baz || $f instanceof Boo)` is the other spelling of the same claim, and it
/// went through the same resolution -- so it was broken and fixed by the same change rather than by
/// a second one. The single guard is the control: one class stays a `Baz` and does not become a
/// union of one, which is the fold `Held::one_of` is relied on for and the distinction every
/// narrowing after this one needs to be able to see.
#[test]
fn an_assert_proves_the_same_union_and_one_class_stays_one_class() {
    check(
        r#"
//- /src/app.php
<?php

class Baz {}

class Boo {}

function through(mixed $f, mixed $one): void
{
    assert($f instanceof Baz || $f instanceof Boo);
    $as@asserted@serted = $f;

    if ($one instanceof Baz) {
        $o@single@ne;
    }
}
"#,
        &[
            Query::hover("asserted", "asserted"),
            Query::hover("one class only", "single"),
        ],
        expect![[r#"
            asserted
            - code: Baz|Boo $asserted
            - note: assigned
            one class only
            - code: Baz $one
            - note: narrowed by a guard
        "#]],
    );
}

/// The same disjunction written as PHP usually writes it: a negated conjunction that leaves.
///
/// `if (!$z instanceof One && !$z instanceof Two) { return; }` is the dominant early-return guard
/// with two alternatives, and by De Morgan it proves exactly what the `||` above does. One rule reads
/// both, because two collectors of one claim is the shape that drifts.
///
/// The `else` is the same statement read the other way and must answer the same. `!$z instanceof One
/// && !$flag` must not: the branch is reachable with `$flag` false and no `instanceof` having decided
/// anything, so a single operand of the wrong shape declines the whole thing.
///
/// `positiveOr` is here because it is what made the rule reach far enough. A receiver that already
/// names a class kept it, so a guard over a sealed hierarchy -- the ordinary case, where `Zero` is the
/// interface and `One` and `Two` are its implementations -- proved nothing at all. Every named class
/// being below what the value already names makes the guard strictly the more specific claim.
#[test]
fn a_negated_conjunction_that_leaves_proves_the_same_union() {
    check(
        r#"
//- /src/app.php
<?php

interface Zero {}

class One implements Zero {}

class Two implements Zero {}

function shapes(Zero $z, Zero $e, Zero $m, Zero $o, bool $flag): void
{
    if (!$z instanceof One && !$z instanceof Two) {
        return;
    }
    $z@leaving@;

    if (!$e instanceof One && !$e instanceof Two) {
    } else {
        $e@otherwise@;
    }

    if (!$m instanceof One && !$flag) {
        return;
    }
    $m@mixed@;

    if ($o instanceof One || $o instanceof Two) {
        $o@disjunction@;
    }
}
"#,
        &[
            Query::hover("after a leaving branch", "leaving"),
            Query::hover("in the else of the same test", "otherwise"),
            Query::hover("one operand is not an instanceof", "mixed"),
            Query::hover(
                "the plain disjunction, on a declared interface",
                "disjunction",
            ),
        ],
        expect![[r#"
            after a leaving branch
            - code: One|Two $z
            - note: narrowed by a guard
            in the else of the same test
            - code: One|Two $e
            - note: narrowed by a guard
            one operand is not an instanceof
            - code: Zero $m
            - note: parameter
            the plain disjunction, on a declared interface
            - code: One|Two $o
            - note: narrowed by a guard
        "#]],
    );
}
