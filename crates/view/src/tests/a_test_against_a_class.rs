//! A conditional return that tests whether an argument is an instance of one class.
//!
//! Symfony's config builders hand back the builder when given a closure and an `ExprBuilder` to chain
//! on when given nothing -- `@return ($closure is \Closure ? $this : ExprBuilder<T>)` -- and every
//! config tree calls `->validate()` the second way. WordPress's `sanitize_post()` hands back what it
//! was given, `WP_Post` or `stdClass` or an array. A bare class was a target the test vocabulary had
//! no word for, so both read as every branch at once.
//!
//! An operand is judged the way a declaration is: the class or one below it is an instance, an
//! unrelated class with nothing below it to bridge them is not, and an interface may be either.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn an_instance_test_reads_what_the_argument_is() {
    check_view(
        r#"
//- /stubs/core.php  stub
<?php
final class Closure {}
class stdClass {}

//- /src/app.php
<?php
class WP_Post {}
class ExprBuilder {}
class NodeBuilder
{
    /** @return ($closure is \Closure ? $this : ExprBuilder) */
    public function validate(?\Closure $closure = null): ExprBuilder|static {}
}

/**
 * @phpstan-return ($post is WP_Post ? WP_Post : ($post is stdClass ? stdClass : array<string, mixed>))
 */
function sanitize_post( $post ) {}

function run( NodeBuilder $node, WP_Post $post, stdClass $row, array $fields, Countable $counted ) {
    $a = $node->validate();
    $@a@a;
    $b = $node->validate( function () {} );
    $@b@b;
    $c = sanitize_post( $post );
    $@c@c;
    $d = sanitize_post( $row );
    $@d@d;
    $e = sanitize_post( $fields );
    $@e@e;
    $f = sanitize_post( $counted );
    $@f@f;
}
"#,
        &[
            Query::variable_type("no closure, so the default null", "a"),
            Query::variable_type("a closure is an instance of Closure", "b"),
            Query::variable_type("the class itself", "c"),
            Query::variable_type("an unrelated class, past the first link", "d"),
            Query::variable_type("an array is no instance", "e"),
            Query::variable_type("an interface may be either", "f"),
        ],
        expect![[r#"
            no closure, so the default null
            - ExprBuilder
            a closure is an instance of Closure
            - NodeBuilder
            the class itself
            - WP_Post
            an unrelated class, past the first link
            - stdClass
            an array is no instance
            - array<string, mixed>
            an interface may be either
            - WP_Post|stdClass|array<string, mixed>
        "#]],
    );
}

/// `class-string<Foo>` names a class *of* `Foo`, and a call naming another class fails it. Read as
/// "names a class" it passed for every one: Symfony's `create(TextType::class)` is `($type is
/// class-string<FormFlowTypeInterface> ? FormFlowInterface : FormInterface)`, and 196 of them read
/// as the flow. A template there is no bound -- `app(Foo::class)` still takes its branch.
#[test]
fn a_class_string_test_compares_the_class_with_what_it_is_written_over() {
    check_view(
        r#"
//- /src/app.php
<?php
interface FormInterface {}
interface FormFlowInterface extends FormInterface {}
interface FormTypeInterface {}
interface FormFlowTypeInterface extends FormTypeInterface {}
class TextType implements FormTypeInterface {}
class FlowType implements FormFlowTypeInterface {}

interface Factory
{
    /** @return ($type is class-string<FormFlowTypeInterface> ? FormFlowInterface : FormInterface) */
    public function create( string $type ): FormInterface;
}

/**
 * @template T
 * @param class-string<T>|null $abstract
 * @return ($abstract is class-string<T> ? T : mixed)
 */
function app( $abstract = null ) {}

function run( Factory $factory ) {
    $a = $factory->create( TextType::class );
    $@a@a;
    $b = $factory->create( FlowType::class );
    $@b@b;
    $c = app( TextType::class );
    $@c@c;
}
"#,
        &[
            Query::variable_type("a class outside the bound", "a"),
            Query::variable_type("a class below it", "b"),
            Query::variable_type("a template is no bound", "c"),
        ],
        expect![[r#"
            a class outside the bound
            - FormInterface
            a class below it
            - FormFlowInterface
            a template is no bound
            - TextType
        "#]],
    );
}
