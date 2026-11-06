use expect_test::expect;

use super::check_hir;

#[test]
fn union_and_intersection_are_flattened_from_binary_pairs() {
    // mago nests `A|B|C` as pairs. Flattening here is what lets a consumer ask for the operand
    // list instead of walking a tree.
    check_hir(
        r#"
//- /src/T.php
<?php
function f(int|string|null $a, Countable&Traversable $b) {}
"#,
        expect![[r#"
            function f(int|string|null $a, Countable&Traversable $b) @ 2:10-2:11
        "#]],
    );
}

#[test]
fn nullable_stays_a_wrapper_rather_than_becoming_a_union() {
    // `?T` is kept as written so hover and rename can reproduce the source; desugaring it to
    // `T|null` is the type layer's decision, not lowering's.
    check_hir(
        r#"
//- /src/T.php
<?php
function f(?User $u): ?string {}
"#,
        expect![[r#"
            function f(?User $u): ?string @ 2:10-2:11
        "#]],
    );
}

#[test]
fn disjunctive_normal_form_keeps_its_grouping() {
    check_hir(
        r#"
//- /src/T.php
<?php
function f((Countable&Traversable)|null $x) {}
"#,
        expect![[r#"
            function f((Countable&Traversable)|null $x) @ 2:10-2:11
        "#]],
    );
}

#[test]
fn reserved_type_names_never_become_class_references() {
    // If `int` were lowered as a class name, the def-map would look it up and report it undefined.
    check_hir(
        r#"
//- /src/T.php
<?php
function f(int $a, float $b, string $c, bool $d, array $e, object $g, callable $h, iterable $i, mixed $j): never {}
"#,
        expect![[r#"
            function f(int $a, float $b, string $c, bool $d, array $e, object $g, callable $h, iterable $i, mixed $j): never @ 2:10-2:11
        "#]],
    );
}

#[test]
fn self_static_and_parent_are_relative_not_named() {
    check_hir(
        r#"
//- /src/T.php
<?php
class C
{
    public function a(): self {}
    public function b(): static {}
    public function c(): parent {}
}
"#,
        expect![[r#"
            class C @ 2:7-2:8
              method a(): self  public @ 4:21-4:22
              method b(): static  public @ 5:21-5:22
              method c(): parent  public @ 6:21-6:22
        "#]],
    );
}

#[test]
fn a_name_records_how_it_was_anchored() {
    // The anchor decides whether the def-map consults imports at all, so losing it turns into
    // wrong resolution rather than no resolution.
    check_hir(
        r#"
//- /src/T.php
<?php
namespace App;

class C
{
    public function a(): Repo {}
    public function b(): \Other\Repo {}
    public function c(): Nested\Repo {}
    public function d(): namespace\Repo {}
}
"#,
        expect![[r#"
            namespace App @ 2:11-2:14
              class C @ 4:7-4:8
                method a(): Repo  public @ 6:21-6:22
                method b(): \Other\Repo  public @ 7:21-7:22
                method c(): Nested\Repo  public @ 8:21-8:22
                method d(): namespace\Repo  public @ 9:21-9:22
        "#]],
    );
}

#[test]
fn an_unannotated_return_type_is_absent_not_mixed() {
    // "not annotated" and "annotated as mixed" are different facts, and the type layer treats
    // them differently.
    check_hir(
        r#"
//- /src/T.php
<?php
function bare() {}
function explicit(): mixed {}
"#,
        expect![[r#"
            function bare() @ 2:10-2:14
            function explicit(): mixed @ 3:10-3:18
        "#]],
    );
}

#[test]
fn a_docblock_shape_keeps_its_keys_removed() {
    check_hir(
        r#"
//- /src/T.php
<?php
/**
 * @param array{id: int, name?: string} $row
 * @return array{app: Repo}
 */
function f(array $row): array {}
"#,
        expect![[r#"
            function f(array $row): array doc: array{app: Repo} @ 6:10-6:11  [doc]
        "#]],
    );
}
