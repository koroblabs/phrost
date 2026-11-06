//! What an `@` typed where a docblock tag starts offers: the tags what it documents can hold.
//!
//! Nothing was offered at all -- `@ret`, `@par` and `{@inh` answered an empty list, while goto on a
//! written `{@inheritdoc}` already went to the declaration above.

use expect_test::expect;

use crate::tests::{Query, check};

const CONTRACT: &str = r#"
//- /src/Contract.php
<?php
namespace App;

interface Contract
{
    public function find(int $id): ?object;
}
"#;

fn with_contract(client: &str, queries: &[Query], expect: expect_test::Expect) {
    check(&format!("{CONTRACT}\n{client}"), queries, expect);
}

#[test]
fn a_method_offers_its_tags_and_inheritdoc_where_there_is_something_to_inherit() {
    with_contract(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo implements Contract
{
    /**
     * @@here@
     */
    public function find(int $id): ?object {}
}
"#,
        &[Query::completion("overriding method", "here")],
        expect![[r#"
            overriding method
            - {@inheritdoc} [keyword]
            - @param [keyword]
            - @return [keyword]
            - @throws [keyword]
            - @template [keyword]
            - @global [keyword]
            - @phpstan-param [keyword]
            - @phpstan-return [keyword]
            - @phpstan-assert [keyword]
            - @phpstan-assert-if-true [keyword]
            - @phpstan-assert-if-false [keyword]
            - @phpstan-impure [keyword]
            - @phpstan-pure [keyword]
            - @psalm-param [keyword]
            - @psalm-return [keyword]
            - @deprecated [keyword]
            - @see [keyword]
            - @internal [keyword]
            - @since [keyword]
            - @todo [keyword]
            - @link [keyword]
            - ... and the client must ask again
            - accepting the first leaves `* {@inheritdoc}`
        "#]],
    );
}

#[test]
fn a_method_overriding_nothing_is_not_offered_inheritdoc() {
    with_contract(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * \@in@here@
     */
    public function find(int $id): ?object {}
}
"#,
        &[Query::completion("own method", "here")],
        expect![[r#"
            own method
            - @internal [keyword]
            - ... and the client must ask again
            - accepting the first leaves `* @internal`
        "#]],
    );
}

#[test]
fn a_tag_the_docblock_already_holds_once_is_not_offered_again() {
    with_contract(
        r#"
//- /src/Repo.php
<?php
namespace App;

/**
 * @return int
 * \@@here@
 */
function count_rows(int $id, int $limit): int {}
"#,
        &[Query::completion("second tag", "here")],
        expect![[r#"
            second tag
            - @param [keyword]
            - @throws [keyword]
            - @template [keyword]
            - @global [keyword]
            - @phpstan-param [keyword]
            - @phpstan-return [keyword]
            - @phpstan-assert [keyword]
            - @phpstan-assert-if-true [keyword]
            - @phpstan-assert-if-false [keyword]
            - @phpstan-impure [keyword]
            - @phpstan-pure [keyword]
            - @psalm-param [keyword]
            - @psalm-return [keyword]
            - @deprecated [keyword]
            - @see [keyword]
            - @internal [keyword]
            - @since [keyword]
            - @todo [keyword]
            - @link [keyword]
            - ... and the client must ask again
            - accepting the first leaves `* @param`
        "#]],
    );
}

#[test]
fn a_property_offers_var() {
    with_contract(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * \@v@here@
     */
    private array $rows = [];
}
"#,
        &[Query::completion("property", "here")],
        expect![[r#"
            property
            - @var [keyword]
            - ... and the client must ask again
            - accepting the first leaves `* @var`
        "#]],
    );
}

#[test]
fn a_class_offers_the_tags_that_describe_a_class() {
    with_contract(
        r#"
//- /src/Repo.php
<?php
namespace App;

/**
 * \@prop@here@
 */
class Repo {}
"#,
        &[Query::completion("class", "here")],
        expect![[r#"
            class
            - @property [keyword]
            - @property-read [keyword]
            - @property-write [keyword]
            - ... and the client must ask again
            - accepting the first leaves `* @property`
        "#]],
    );
}

#[test]
fn an_inline_tag_keeps_the_brace_the_editor_closed() {
    with_contract(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo implements Contract
{
    /**
     * {\@inh@here@}
     */
    public function find(int $id): ?object {}
}
"#,
        &[Query::completion("inline", "here")],
        expect![[r#"
            inline
            - {@inheritdoc} [keyword]  insert `{@inheritdoc`
            - ... and the client must ask again
            - accepting the first leaves `* {@inheritdoc}`
        "#]],
    );
}

#[test]
fn an_at_sign_inside_a_sentence_is_not_a_tag() {
    with_contract(
        r#"
//- /src/Repo.php
<?php
namespace App;

/**
 * Mail admin\@ret@here@ about it.
 */
function notify(): void {}
"#,
        &[Query::completion("prose", "here")],
        expect![[r#"
            prose
            - (nothing offered)
        "#]],
    );
}

/// A property overrides as a method does, and Drupal writes `{@inheritdoc}` over every one --
/// `protected string $key` on each notification class.
#[test]
fn an_overriding_property_is_offered_inheritdoc() {
    check(
        r#"
//- /src/Notification.php
<?php
namespace App;

abstract class Notification
{
    /** The machine name. */
    protected string $key = '';
}

class Expiring extends Notification
{
    /**
     * {\@inh@here@}
     */
    protected string $key = 'expiring';
}
"#,
        &[Query::completion("property", "here")],
        expect![[r#"
            property
            - {@inheritdoc} [keyword]  insert `{@inheritdoc`
            - ... and the client must ask again
            - accepting the first leaves `* {@inheritdoc}`
        "#]],
    );
}
