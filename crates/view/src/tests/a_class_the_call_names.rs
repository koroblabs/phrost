//! What a call gets back when the declaration names the *argument* that decides, not the class.
//!
//! `@return T` bound to a `@param class-string<T>` is the factory shape, and the three things a tag
//! can say about the result are all written in the tag and none is in the declaration, which says
//! `object` or `array` or `?Node`: whether it is the class, the class or null, or a container of it.
//!
//! Each of the three was refused for a reason that had expired.
//!
//! # The null
//!
//! `@return ?T` was refused whole, on the argument that a null says more than the argument can
//! answer. It does not: the null belongs to the *declaration*, exactly as `@return ?T` beside a
//! native hint already does, and `class-string<T>` says which class and nothing about whether one was
//! found. `NodeFinder::findFirstInstanceOf($stmts, FuncCall::class)` was a `?Node`, and `$first?->name`
//! was then reported as an undefined property on code that runs -- `FuncCall` declares `$name`.
//!
//! Three spellings and a real library writes each: `?T`, `T|null`, and the `null|TNode` `NodeFinder`
//! writes, because mago's docblock union is binary and the null sits on whichever side the author put
//! it.
//!
//! # The container
//!
//! `findInstanceOf` sits beside `findFirstInstanceOf` in that same class and differs only in this,
//! and Doctrine's `findAll()` beside its `find()`. The declaration says `array` either way.
//!
//! # And a union in a neighbouring parameter is innocent
//!
//! `findFirstInstanceOf` also takes `@param Node|Node[] $nodes`, which looks like the culprit and is
//! not: what decided was the nullable return. The matrix below is why both axes are varied together
//! rather than one at a time.

use expect_test::expect;

use super::{Query, check_view};

const PROJECT: &str = r#"
//- /src/Rows.php
<?php
namespace App;

class Row {}
class SubRow extends Row {}

class Finder
{
    /**
     * @template T of Row
     *
     * @param Row|Row[] $nodes
     * @param class-string<T> $class
     *
     * @return T
     */
    public function one(Row|array $nodes, string $class): Row {}

    /**
     * @template T of Row
     *
     * @param Row|Row[] $nodes
     * @param class-string<T> $class
     *
     * @return ?T
     */
    public function maybe(Row|array $nodes, string $class): ?Row {}

    /**
     * @template T of Row
     *
     * @param class-string<T> $class
     *
     * @return null|T
     */
    public function nullFirst(string $class): ?Row {}

    /**
     * @template T of Row
     *
     * @param class-string<T> $class
     *
     * @return T[]
     */
    public function many(string $class): array {}

    /**
     * @template T of Row
     *
     * @param class-string<T> $class
     *
     * @phpstan-return list<T>
     */
    public function listed(string $class): array {}

    /**
     * @template T of Row
     *
     * @param class-string<T> $class
     *
     * @return ?T[]
     */
    public function maybeMany(string $class): ?array {}
}

class Reader
{
    public function run(Finder $finder, Row $row): void
    {
        $one = $finder->one($row, SubRow::class);
        $maybe = $finder->maybe($row, SubRow::class);
        $nullFirst = $finder->nullFirst(SubRow::class);
        $many = $finder->many(SubRow::class);
        $listed = $finder->listed(SubRow::class);
        $maybeMany = $finder->maybeMany(SubRow::class);

        $o@one@ne;
        $ma@maybe@ybe;
        $nu@nullFirst@llFirst;
        $man@many@y;
        $lis@listed@ted;
        $may@maybeMany@beMany;
    }
}
"#;

#[test]
fn the_tag_decides_the_shape_and_the_call_decides_the_class() {
    check_view(
        PROJECT,
        &[
            Query::variable_type("the class itself", "one"),
            Query::variable_type("the class or null, written `?T`", "maybe"),
            Query::variable_type("the same, with the null written first", "nullFirst"),
            Query::variable_type("a container of it", "many"),
            Query::variable_type("the same, spelled `list<T>` in a prefixed tag", "listed"),
            Query::variable_type(
                "a container, or null -- never a container of nulls",
                "maybeMany",
            ),
        ],
        expect![[r#"
            the class itself
            - SubRow
            the class or null, written `?T`
            - ?SubRow
            the same, with the null written first
            - ?SubRow
            a container of it
            - SubRow[]
            the same, spelled `list<T>` in a prefixed tag
            - SubRow[]
            a container, or null -- never a container of nulls
            - ?SubRow[]
        "#]],
    );
}
