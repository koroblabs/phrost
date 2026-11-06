//! What one class name means where another is expected.
//!
//! The pure half of this judgement is tested in `phrost-ty` against a stub hierarchy. What is tested
//! here is the real one: whether the project's resolved relations produce the right verdict, and --
//! more importantly -- whether the cases that must stay silent do. A wrong rejection here becomes a
//! wrong "expects Foo, Bar given" on somebody's framework code.
//!
//! Every case runs through `with_open_scopes`, so each also asserts the answer does not depend on
//! which files are open. That matters more here than usual: a hierarchy walk that read an open
//! buffer would answer one way in the editor and another in CI.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_model::BuiltinTy;
use phrost_testkit::FixtureSpec;
use phrost_ty::{TypeId, TypeRef};

use crate::IndexView;

/// The verdict for each `value -> expected` pair, lenient file then strict file.
///
/// Both settings always. The directive is the thing most likely to end up wired to the wrong file,
/// and a table with one column would hide exactly that.
fn check(fixture: &str, pairs: &[(&str, &str)], expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);

    let rendered = with_open_scopes(&world, None, |open| {
        let mut out = String::new();
        for (value, expected) in pairs {
            let mut verdicts = String::new();
            for file in ["lenient.php", "strict.php"] {
                let view = IndexView::in_file(world.skeleton(), open, world.file_id(file));
                let (from, to) = (named(&view, value), named(&view, expected));
                let _ = write!(verdicts, "  {:?}", view.fit(from, to));
            }
            let _ = writeln!(out, "{value} -> {expected}{verdicts}");
        }
        out
    });

    expect.assert_eq(&rendered);
}

/// The id an already-interned type has.
///
/// A scan rather than a lookup, because interning takes `&mut` and a view holds the table by
/// reference. Every type asked about here is written in the fixture, which is what put it in the
/// table -- a panic means the fixture forgot to mention it, not that the projection is wrong.
fn named(view: &IndexView<'_>, written: &str) -> TypeId {
    let types = view.skeleton.types();
    let wanted = match BuiltinTy::from_name(written) {
        Some(builtin) => TypeRef::Builtin(builtin),
        None => TypeRef::Named(
            view.skeleton
                .interner()
                .lookup_name(written)
                .unwrap_or_else(|| panic!("the fixture never writes `{written}`")),
        ),
    };
    types
        .nodes()
        .iter()
        .position(|node| node == &wanted)
        .map(|at| TypeId::from_raw(u32::try_from(at).expect("the table is small")))
        .unwrap_or_else(|| panic!("`{written}` is not in the type table"))
}

/// One lenient file and one strict one, so each column is a real file's answer.
const FILES: &str = r#"
//- /lenient.php
<?php
$x = 1;

//- /strict.php
<?php
declare(strict_types=1);
$x = 1;
"#;

#[test]
fn a_subclass_reaches_its_ancestors_and_its_interfaces() {
    check(
        &format!(
            r#"{FILES}
//- /tree.php
<?php
interface Runs {{}}
class Base implements Runs {{}}
class Middle extends Base {{}}
final class Leaf extends Middle {{}}
class Other {{}}

function writes(Leaf $a, Middle $b, Base $c, Runs $d, Other $e): void {{}}
"#
        ),
        &[
            ("Leaf", "Leaf"),
            ("Leaf", "Middle"),
            ("Leaf", "Base"),
            ("Leaf", "Runs"),
        ],
        expect![[r#"
            Leaf -> Leaf  Accepted  Accepted
            Leaf -> Middle  Accepted  Accepted
            Leaf -> Base  Accepted  Accepted
            Leaf -> Runs  Accepted  Accepted
        "#]],
    );
}

#[test]
fn an_unrelated_interface_is_refused_whatever_could_be_below() {
    // `Base` is open, so a subclass of it may implement `Runs` -- and this used to be silence for
    // that reason. Nothing declared makes a `Base` one, though, and PHPStan and PHP's `TypeError`
    // both go by what is declared, so it is a rejection now just as the final `Leaf` is.
    check(
        &format!(
            r#"{FILES}
//- /tree.php
<?php
interface Runs {{}}
class Base {{}}
final class Leaf {{}}

function writes(Base $a, Leaf $b, Runs $c): void {{}}
"#
        ),
        &[("Base", "Runs"), ("Leaf", "Runs")],
        expect![[r#"
            Base -> Runs  Rejected  Rejected
            Leaf -> Runs  Rejected  Rejected
        "#]],
    );
}

#[test]
fn two_unrelated_classes_cannot_both_be_satisfied() {
    // Single inheritance is what makes this provable where the interface case was not: no class is
    // below two unrelated classes at once.
    check(
        &format!(
            r#"{FILES}
//- /tree.php
<?php
class Invoice {{}}
class Payment {{}}
class Refund extends Payment {{}}

function writes(Invoice $a, Payment $b, Refund $c): void {{}}
"#
        ),
        &[
            ("Invoice", "Payment"),
            ("Refund", "Payment"),
            ("Payment", "Refund"),
        ],
        expect![[r#"
            Invoice -> Payment  Rejected  Rejected
            Refund -> Payment  Accepted  Accepted
            Payment -> Refund  Unknown  Unknown
        "#]],
    );
}

#[test]
fn a_class_the_index_never_saw_is_never_a_finding() {
    // The silence that keeps an unresolved vendor name from becoming a finding on every call that
    // mentions it.
    check(
        &format!(
            r#"{FILES}
//- /tree.php
<?php
class Known {{}}

function writes(Known $a, \Vendor\Missing $b): void {{}}
"#
        ),
        &[("Known", "Vendor\\Missing"), ("Vendor\\Missing", "Known")],
        expect![[r#"
            Known -> Vendor\Missing  Unknown  Unknown
            Vendor\Missing -> Known  Unknown  Unknown
        "#]],
    );
}

#[test]
fn an_enum_is_closed_even_without_the_word_final() {
    check(
        &format!(
            r#"{FILES}
//- /tree.php
<?php
interface Runs {{}}
enum Status {{
    case Open;
}}

function writes(Status $a, Runs $b): void {{}}
"#
        ),
        &[("Status", "Runs")],
        expect![[r#"
            Status -> Runs  Rejected  Rejected
        "#]],
    );
}

#[test]
fn a_stringable_object_is_a_string_only_where_coercion_is_allowed() {
    // `__toString` is a conversion, and `strict_types=1` is precisely the switch that turns
    // conversions off -- so the same object is accepted in one file and a TypeError in the next.
    // The class declares no interface, which is how a project older than `Stringable` writes it.
    check(
        &format!(
            r#"{FILES}
//- /tree.php
<?php
class Label {{
    public function __toString(): string {{
        return '';
    }}
}}
class Plain {{}}

function writes(Label $a, Plain $b, string $c): void {{}}
"#
        ),
        &[("Label", "string"), ("Plain", "string")],
        expect![[r#"
            Label -> string  Accepted  Rejected
            Plain -> string  Unknown  Rejected
        "#]],
    );
}

#[test]
fn only_a_traversable_object_is_iterable() {
    // The chain is declared in the fixture rather than leant on: this harness indexes the fixture
    // and nothing else, so a real project's `IteratorAggregate extends Traversable` comes from the
    // stubs and is simply absent here. Which is itself the right behaviour -- a hierarchy the index
    // has not seen produces silence, not a verdict -- and `Plain` below is the case that proves it.
    check(
        &format!(
            r#"{FILES}
//- /tree.php
<?php
interface Traversable {{}}
interface IteratorAggregate extends Traversable {{}}

class Bag implements IteratorAggregate {{}}
class Plain {{}}

function writes(Bag $a, Plain $b, iterable $c): void {{}}
"#
        ),
        &[("Bag", "iterable"), ("Plain", "iterable")],
        expect![[r#"
            Bag -> iterable  Accepted  Accepted
            Plain -> iterable  Unknown  Unknown
        "#]],
    );
}
