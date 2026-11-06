use expect_test::expect;

use super::{Query, check};

#[test]
fn goto_and_hover_agree_about_a_type_hint() {
    // The check a per-feature test cannot make: both features start from the same projection, so
    // they cannot describe one thing and jump to another.
    check(
        r#"
//- /src/User.php
<?php
namespace App;

/** A user. */
class User extends Person implements Stringable
{
    public function name(): string {}
}

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(int $id): ?Us@target@er {}
}
"#,
        &[
            Query::goto("goto", "target"),
            Query::hover("hover", "target"),
        ],
        expect![[r#"
            goto
            - class App\User @ src/User.php 5:7-5:11 (full 5:1-8:2)
            hover
            - code: <?php
            namespace App;

            class User extends Person implements Stringable
            - ---
            - doc: A user.
        "#]],
    );
}

#[test]
fn hover_on_a_member_says_which_class_it_belongs_to() {
    // A bare signature does not say whose method it is, and a hover that does not say is one the
    // reader has to go and check.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function fi@member@nd(int $id, ?string $name = null): ?User {}
    private static array $ca@prop@che = [];
}
"#,
        &[
            Query::hover("hover method", "member"),
            Query::hover("hover property", "prop"),
        ],
        expect![[r#"
            hover method
            - code: <?php
            namespace App;

            class Repo
            {
                public function find(int $id, ?string $name = null): ?User
            }
            hover property
            - code: <?php
            namespace App;

            class Repo
            {
                private static array $cache
            }
        "#]],
    );
}

#[test]
fn goto_reaches_a_member_declared_in_a_trait_in_another_file() {
    check(
        r#"
//- /src/Timestamps.php
<?php
namespace App;

trait Timestamps
{
    public function touch(): void {}
}

//- /src/Repo.php
<?php
namespace App;

class Repo
{
    use Time@trait_use@stamps;
}
"#,
        &[
            Query::goto("goto trait", "trait_use"),
            Query::hover("hover trait", "trait_use"),
        ],
        expect![[r#"
            goto trait
            - trait App\Timestamps @ src/Timestamps.php 4:7-4:17 (full 4:1-7:2)
            hover trait
            - code: <?php
            namespace App;

            trait Timestamps
        "#]],
    );
}

#[test]
fn a_vendor_class_is_navigable_and_describable() {
    // Vendor files are skeleton-only, and this is what proves the skeleton kept enough: a name
    // range to jump to, and a rendered header to show.
    check(
        r#"
//- /vendor/acme/src/Model.php
<?php
namespace Acme\Orm;

abstract class Model implements Countable
{
    public function save(): bool {}
}

//- /src/User.php
<?php
namespace App;

use Acme\Orm\Model;

class User extends Mo@parent@del {}
"#,
        &[
            Query::goto("goto vendor", "parent"),
            Query::hover("hover vendor", "parent"),
        ],
        expect![[r#"
            goto vendor
            - class Acme\Orm\Model @ vendor/acme/src/Model.php 4:16-4:21 (full 4:1-7:2)
            hover vendor
            - code: <?php
            namespace Acme\Orm;

            abstract class Model implements Countable
        "#]],
    );
}

#[test]
fn an_unresolvable_name_produces_nothing_rather_than_a_guess() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo extends Not@missing@Installed {}
"#,
        &[
            Query::goto("goto", "missing"),
            Query::hover("hover", "missing"),
        ],
        expect![[r#"
            goto
            - (nothing to go to)
            hover
            - (nothing to describe)
        "#]],
    );
}

#[test]
fn a_cursor_on_nothing_in_particular_answers_nothing() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void { $x = 1@inside@; }
}
"#,
        &[
            Query::goto("goto", "inside"),
            Query::hover("hover", "inside"),
        ],
        expect![[r#"
            goto
            - (nothing to go to)
            hover
            - (nothing to describe)
        "#]],
    );
}

#[test]
fn hover_on_a_classes_own_member_says_nothing_about_origin() {
    // "own" is what a reader already assumes, so saying it is a line they learn to skip. The
    // interesting case -- a method that arrived from two classes up or from a trait -- is reached by
    // hovering a *call site*, which needs expression bodies; until then the origin is covered at the
    // view layer, where member_location reports the declaring class.
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    public function inherited(): void {}
}

//- /src/Timestamps.php
<?php
namespace App;

trait Timestamps
{
    public function touch(): void {}
}

//- /src/Repo.php
<?php
namespace App;

class Repo extends Base
{
    use Timestamps;

    public function ow@own@n(): void {}
}
"#,
        &[Query::hover("own member", "own")],
        expect![[r#"
            own member
            - code: <?php
            namespace App;

            class Repo
            {
                public function own(): void
            }
        "#]],
    );
}

#[test]
fn hover_shows_a_docblock_with_its_prose_and_its_tags_apart() {
    // Tags go in a code block, and not for looks: `array<int, Foo>` is full of angle brackets, which
    // markdown reads as raw HTML and swallows. Prose is the author's markup and passes through.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * Finds users by identifier.
     *
     * Consults the cache first, and falls back to the *database*.
     *
     * @param array<int, string> $ids
     *   The identifiers to look for.
     * @return list<User>
     * @throws NotFound
     */
    public function fi@member@nd(array $ids): array {}
}
"#,
        &[Query::hover("hover", "member")],
        expect![[r#"
            hover
            - code: <?php
            namespace App;

            class Repo
            {
                public function find(array<int, string> $ids): array<int, User>
            }
            - ---
            - doc: Finds users by identifier.

            Consults the cache first, and falls back to the *database*.
            - tag: Parameter $ids array<int, string> The identifiers to look for.
            - tag: Returns list<User> 
            - tag: Throws NotFound
        "#]],
    );
}

#[test]
fn inheritdoc_takes_the_description_from_the_declaration_it_overrides() {
    // Nearly every implementation of a Drupal interface documents itself with nothing else, so
    // without this a hover on the method people actually call says `{@inheritdoc}` and no more.
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

interface Base
{
    /**
     * Finds users by identifier.
     *
     * @param array<int, string> $ids
     * @return list<User>
     */
    public function find(array $ids): array;
}

//- /src/Repo.php
<?php
namespace App;

class Repo implements Base
{
    /**
     * {@inheritdoc}
     */
    public function fi@member@nd(array $ids): array {}
}
"#,
        &[Query::hover("hover", "member")],
        expect![[r#"
            hover
            - code: <?php
            namespace App;

            class Repo
            {
                public function find(array $ids): array<int, User>
            }
            - ---
            - doc: Finds users by identifier.
            - tag: Parameter $ids array<int, string> 
            - tag: Returns list<User>
        "#]],
    );
}

#[test]
fn an_inline_inheritdoc_is_substituted_where_it_was_written() {
    // The tag means "the inherited text goes here". Appending it instead would reorder what the
    // author wrote, and the note after it is usually the part they cared about.
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

interface Base
{
    /** Finds users. */
    public function find(): void;
}

//- /src/Repo.php
<?php
namespace App;

class Repo implements Base
{
    /**
     * {@inheritDoc}
     *
     * Caches the result.
     */
    public function fi@member@nd(): void {}
}
"#,
        &[Query::hover("hover", "member")],
        expect![[r#"
            hover
            - code: <?php
            namespace App;

            class Repo
            {
                public function find(): void
            }
            - ---
            - doc: Finds users.

            Caches the result.
        "#]],
    );
}

#[test]
fn an_override_that_documents_itself_keeps_its_own_tags() {
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

interface Base
{
    /**
     * Finds users.
     *
     * @return list<User>
     */
    public function find(): array;
}

//- /src/Repo.php
<?php
namespace App;

class Repo implements Base
{
    /**
     * {@inheritdoc}
     *
     * @return list<Admin>
     */
    public function fi@member@nd(): array {}
}
"#,
        &[Query::hover("hover", "member")],
        expect![[r#"
            hover
            - code: <?php
            namespace App;

            class Repo
            {
                public function find(): array<int, Admin>
            }
            - ---
            - doc: Finds users.
            - tag: Returns list<Admin>
        "#]],
    );
}

#[test]
fn inheritdoc_is_followed_through_more_than_one_level() {
    // The abstract class in the middle documents itself with the tag too, which is the usual shape:
    // an interface says what a method is for, a base class implements part of it, and the leaf says
    // nothing at all.
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

interface Contract
{
    /** Finds users. */
    public function find(): void;
}

abstract class Base implements Contract
{
    /**
     * {@inheritdoc}
     */
    public function find(): void {}
}

//- /src/Repo.php
<?php
namespace App;

class Repo extends Base
{
    /**
     * {@inheritdoc}
     */
    public function fi@member@nd(): void {}
}
"#,
        &[Query::hover("hover", "member")],
        expect![[r#"
            hover
            - code: <?php
            namespace App;

            class Repo
            {
                public function find(): void
            }
            - ---
            - doc: Finds users.
        "#]],
    );
}

#[test]
fn an_inheritdoc_with_nothing_above_it_is_left_as_written() {
    // Wrong in the source, and saying so by showing the tag is better than showing an empty hover:
    // the reader can see what the docblock actually says.
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    /**
     * {@inheritdoc}
     */
    public function fi@member@nd(): void {}
}
"#,
        &[Query::hover("hover", "member")],
        expect![[r#"
            hover
            - code: <?php
            namespace App;

            class Repo
            {
                public function find(): void
            }
            - ---
            - doc: {@inheritdoc}
        "#]],
    );
}

#[test]
fn a_declaration_without_a_docblock_shows_no_documentation_section() {
    check(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function fi@member@nd(): void {}
}
"#,
        &[Query::hover("hover", "member")],
        expect![[r#"
            hover
            - code: <?php
            namespace App;

            class Repo
            {
                public function find(): void
            }
        "#]],
    );
}

#[test]
fn a_parent_that_shares_a_short_name_is_written_in_full() {
    // Real code, and the reason its file aliases the parent. Shortened, the card would read
    // `class AccountProxy extends AccountProxy` -- a class extending itself, which is a false
    // statement rather than a terse one.
    check(
        r#"
//- /src/core.php
<?php
namespace Core\Session;

class AccountProxy {}

//- /src/app.php
<?php
namespace App\Session;

use Core\Session\AccountProxy as CoreProxy;

class Account@here@Proxy extends CoreProxy {}
"#,
        &[Query::hover("the class", "here")],
        expect![[r#"
            the class
            - code: <?php
            namespace App\Session;

            class AccountProxy extends \Core\Session\AccountProxy
        "#]],
    );
}
