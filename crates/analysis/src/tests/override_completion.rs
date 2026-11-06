//! Declaring a method a supertype declared, from a completion.
//!
//! The item is the whole declaration, so the snapshot is the whole declaration: `\n` for a line
//! break and `|` for where the cursor lands. A stub is only worth offering if PHP can read it back,
//! and a snapshot that showed only a label would say nothing about that.
//!
//! Every fixture types a prefix. Not for the feature's sake -- an empty prefix offers the same
//! methods -- but so the whole result is a handful of rows rather than PHP's forty keywords and two
//! hundred class names, which is what a class body offers alongside them.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_class_body_offers_what_the_supertypes_left_to_write() {
    // Required first: the two PHP will not load the class without, then the one it would merely
    // replace. Alphabetical within each half, so an unrelated supertype gaining a method does not
    // move the rows a person is reading.
    check(
        r#"
//- /app.php
<?php
interface Runs
{
    public function run(): int;
}

abstract class Task implements Runs
{
    abstract protected function ready(): string;

    public function report(): string
    {
        return '';
    }
}

class Job extends Task
{
    r@here@
}
"#,
        &[Query::completion("what a class body offers", "here")],
        expect![[r#"
            what a class body offers
            - ready [method]  implement Task  insert `protected function ready(): string\n    {\n        |\n    }`
            - run [method]  implement Runs  insert `public function run(): int\n    {\n        |\n    }`
            - report [method]  override Task  insert `public function report(): string\n    {\n        |\n    }`
            - readonly [keyword]
            - require [keyword]
            - return [keyword]
            - Runs [interface]  Runs
            - ... and the client must ask again
            - accepting the first leaves `protected function ready(): string\n    {\n        |\n    }`
        "#]],
    );
}

#[test]
fn a_method_the_class_already_declares_is_not_offered() {
    // Declaring it twice is a fatal error, not an override -- and a trait's method is the class's
    // own as far as PHP is concerned, so `report` is out for the same reason `ready` is.
    check(
        r#"
//- /app.php
<?php
trait Reports
{
    public function report(): string
    {
        return '';
    }
}

abstract class Task
{
    abstract public function ready(): string;

    public function report(): string
    {
        return '';
    }
}

class Job extends Task
{
    use Reports;

    public function ready(): string
    {
        return '';
    }

    re@here@
}
"#,
        &[Query::completion("already written here", "here")],
        expect![[r#"
            already written here
            - readonly [keyword]
            - require [keyword]
            - return [keyword]
            - Reports [trait]  Reports
            - ... and the client must ask again
            - accepting the first leaves `readonly`
        "#]],
    );
}

#[test]
fn a_final_or_private_method_is_not_offered() {
    // `final` PHP refuses to let anyone replace. A `private` one was never inherited: a child
    // declaring the name is not overriding anything, and nothing about the parent's shape binds it.
    check(
        r#"
//- /app.php
<?php
class Task
{
    final public function reap(): void {}

    private function reset(): void {}

    public function reload(): void {}
}

class Job extends Task
{
    re@here@
}
"#,
        &[Query::completion("neither can be replaced", "here")],
        expect![[r#"
            neither can be replaced
            - reload [method]  override Task  insert `public function reload(): void\n    {\n        |\n    }`
            - readonly [keyword]
            - require [keyword]
            - return [keyword]
            - ... and the client must ask again
            - accepting the first leaves `public function reload(): void\n    {\n        |\n    }`
        "#]],
    );
}

/// A parameter with a default no longer takes the method out of the list.
///
/// The index kept that there *was* a default and not what it was, so an override completed from
/// here would have made an optional parameter required. The value is stored now -- 25 029 of a
/// checkout's 148 917 parameters carry one, 0.1 MB of text and 2.9 MB of index -- so the method is
/// offered and the default is written with it.
#[test]
fn a_parameter_with_a_default_is_offered_and_keeps_its_value() {
    // The index stores that there was a default, never what it was -- a default is an expression and
    // expressions live with bodies, which a closed file does not keep. Writing the parameter without
    // one makes it required, which PHP refuses to load, so the method is not offered at all.
    check(
        r#"
//- /app.php
<?php
abstract class Task
{
    abstract public function rank(int $depth = 3): int;

    abstract public function reach(int $depth): int;
}

class Job extends Task
{
    r@here@
}
"#,
        &[Query::completion(
            "both, each with its own default written",
            "here",
        )],
        expect![[r#"
            both, each with its own default written
            - rank [method]  implement Task  insert `public function rank(int $depth = 3): int\n    {\n        |\n    }`
            - reach [method]  implement Task  insert `public function reach(int $depth): int\n    {\n        |\n    }`
            - readonly [keyword]
            - require [keyword]
            - return [keyword]
            - ... and the client must ask again
            - accepting the first leaves `public function rank(int $depth = 3): int\n    {\n        |\n    }`
        "#]],
    );
}

#[test]
fn a_type_only_a_docblock_declared_is_not_written_as_a_hint() {
    // `list<Item>` is not a type hint and `Item[]` is not either. The native declaration is what the
    // runtime enforces and what the stub writes; the comment's refinement is dropped rather than
    // turned into a file that does not parse.
    check(
        r#"
//- /app.php
<?php
class Item {}

abstract class Task
{
    /**
     * @param list<Item> $items
     * @return array<string, Item>
     */
    abstract public function reduce(array $items): array;
}

class Job extends Task
{
    re@here@
}
"#,
        &[Query::completion("native types only", "here")],
        expect![[r#"
            native types only
            - reduce [method]  implement Task  insert `public function reduce(array $items): array\n    {\n        |\n    }`
            - readonly [keyword]
            - require [keyword]
            - return [keyword]
            - ... and the client must ask again
            - accepting the first leaves `public function reduce(array $items): array\n    {\n        |\n    }`
        "#]],
    );
}

#[test]
fn static_by_reference_variadics_and_class_names_are_written_back() {
    // Each of these is part of the signature PHP checks: dropping `&` or `...` is a fatal error, and
    // a class name written short would mean whatever the file it lands in imports. Anchored, so it
    // means the same thing wherever the stub goes.
    check(
        r#"
//- /app.php
<?php
namespace App;

class Buffer {}

abstract class Task
{
    abstract public static function render(
        Buffer &$into,
        ?Buffer $spare,
        string ...$parts
    ): int|false;
}

class Job extends Task
{
    re@here@
}
"#,
        &[Query::completion("the whole signature", "here")],
        expect![[r#"
            the whole signature
            - render [method]  implement Task  insert `public static function render(\App\Buffer &$into, ?\App\Buffer $spare, string ...$parts): int|false\n    {\n        |\n    }`
            - readonly [keyword]
            - require [keyword]
            - return [keyword]
            - ... and the client must ask again
            - accepting the first leaves `public static function render(\App\Buffer &$into, ?\App\Buffer $spare, string ...$parts): int|false\n    {\n        |\n    }`
        "#]],
    );
}

#[test]
fn self_is_kept_where_it_returns_and_refused_where_it_is_taken() {
    // Returning `self` from the child means the child, which is a subclass of what the parent
    // promised -- PHP allows that. *Accepting* `self` in the child narrows what the method takes,
    // which PHP refuses to load, so that method is not offered rather than offered broken.
    check(
        r#"
//- /base.php
<?php
abstract class Base
{
    abstract public function duplicate(): self;

    abstract public function relate(self $other): void;
}

//- /returns.php
<?php
class Returns extends Base
{
    dup@returns@
}

//- /takes.php
<?php
class Takes extends Base
{
    rel@takes@
}
"#,
        &[
            Query::completion("self as a return type", "returns"),
            Query::completion("self as a parameter type", "takes"),
        ],
        expect![[r#"
            self as a return type
            - duplicate [method]  implement Base  insert `public function duplicate(): self\n    {\n        |\n    }`
            - ... and the client must ask again
            - accepting the first leaves `public function duplicate(): self\n    {\n        |\n    }`
            self as a parameter type
            - (nothing offered)
        "#]],
    );
}

#[test]
fn a_method_body_offers_no_overrides() {
    // The same characters before the cursor, and the wrong place entirely: `report` inside a body is
    // a call somebody is writing, and a declaration there is a syntax error. The tree says which one
    // this is, because the text cannot.
    check(
        r#"
//- /app.php
<?php
class Task
{
    public function report(): string
    {
        return '';
    }
}

class Job extends Task
{
    public function go(): void
    {
        repo@here@
    }
}
"#,
        &[Query::completion("inside a body", "here")],
        expect![[r#"
            inside a body
            - (nothing offered)
        "#]],
    );
}

#[test]
fn a_declaration_already_begun_offers_no_overrides() {
    // What the item writes starts with `public function`, and those words are in the file already.
    // A completion replaces the identifier it was typed over and nothing else, so there is no honest
    // way to offer the whole declaration from here.
    check(
        r#"
//- /app.php
<?php
class Task
{
    public function report(): string
    {
        return '';
    }
}

class Job extends Task
{
    public function repo@here@
}
"#,
        &[Query::completion("after the keywords", "here")],
        expect![[r#"
            after the keywords
            - (nothing offered)
        "#]],
    );
}

#[test]
fn an_interface_body_offers_nothing() {
    // What an interface declares is a signature and no body, which is a different shape from every
    // stub here -- and an interface restating a method it already inherits is rare enough that a
    // second shape would be built for almost nobody.
    check(
        r#"
//- /app.php
<?php
interface Reads
{
    public function read(): string;
}

interface Writes extends Reads
{
    re@here@
}
"#,
        &[Query::completion("inside an interface", "here")],
        expect![[r#"
            inside an interface
            - readonly [keyword]
            - require [keyword]
            - return [keyword]
            - Reads [interface]  Reads
            - ... and the client must ask again
            - accepting the first leaves `readonly`
        "#]],
    );
}

#[test]
fn a_file_indented_with_tabs_gets_a_tab() {
    // Nothing in the index says what a project indents with, so the line the person is typing on is
    // the answer: a tab there means a tab in the body.
    check(
        "\n//- /app.php\n<?php\nabstract class Task\n{\n\tabstract public function ready(): void;\n}\n\nclass Job extends Task\n{\n\tre@here@\n}\n",
        &[Query::completion("tabs", "here")],
        expect![[r#"
            tabs
            - ready [method]  implement Task  insert `public function ready(): void\n\t{\n\t\t|\n\t}`
            - readonly [keyword]
            - require [keyword]
            - return [keyword]
            - ... and the client must ask again
            - accepting the first leaves `public function ready(): void\n\t{\n\t\t|\n\t}`
        "#]],
    );
}
