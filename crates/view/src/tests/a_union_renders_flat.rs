//! A union renders as one flat list, and a `?` never appears inside one.
//!
//! Both grammars that write a `|` are binary, so `string|int|null` is parsed and lowered as
//! `string|(int|null)`. That is the same type and not the same node, and read back it produced
//! `string|?int` -- a spelling PHP has no way to write and no reader can act on.
//!
//! Two spellings, because a union arrives as an interned type when the declaration is in another
//! file and as a written one when it is in this one, and only one of the two paths would have been
//! covered by either fixture alone.
//!
//! The nullable *built* around a union is the third place the rule has to hold, and it is a different
//! path from either spelling above: nothing declared it, an element read out of an array added it.
//! `array<string, string|array<int, string>>` indexed with `??` rendered `?string|array<int, string>`
//! -- a `?` bound to the first operand of a union it was meant to wrap. And a nullable *around a
//! nullable* is the fourth: `?array` reached through a `Held::Stored` and wrapped again read `??array`,
//! two markers where the language allows one.
//!
//! The boolean literals are the other half of the same rule, and PHP is stricter about them than
//! about the nested null: `?false` is not a type it can parse at all, so `false|null` is the only
//! spelling that reads back as what it means. `true|false` goes further and folds to `bool`, which
//! is what the two of them together *are*.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn an_interned_union_puts_its_null_at_the_end() {
    check_view(
        r#"
//- /src/Nodey.php
<?php
namespace App;

interface Nodey
{
    /** @return string|int|null */
    public function id();

    /** @return string|int|bool */
    public function tag();

    /** @return ?Nodey */
    public function parent();

    /** @return Nodey|null */
    public function next();
}

//- /src/Use.php
<?php
namespace App;

class Reader
{
    public function probe(Nodey $node): void
    {
        $id = $node->id();
        $i@id@d;
        $tag = $node->tag();
        $t@tag@ag;
        $up = $node->parent();
        $u@up@p;
        $next = $node->next();
        $ne@next@xt;
    }
}
"#,
        &[
            Query::variable_type("three with a null", "id"),
            Query::variable_type("three without one", "tag"),
            Query::variable_type("a bare nullable stays as written", "up"),
            Query::variable_type("the union spelling of the same", "next"),
        ],
        expect![[r#"
            three with a null
            - string|int|null
            three without one
            - string|int|bool
            a bare nullable stays as written
            - ?Nodey
            the union spelling of the same
            - ?Nodey
        "#]],
    );
}

#[test]
fn a_union_written_in_this_file_renders_the_same_way() {
    check_view(
        r#"
//- /src/Reader.php
<?php
namespace App;

class Node {}

class Reader
{
    /**
     * @param string|int|null $id
     * @param string|int|bool $tag
     * @param ?Node $up
     */
    public function probe($id, $tag, $up): void
    {
        $i@id@d;
        $t@tag@ag;
        $u@up@p;
    }
}
"#,
        &[
            Query::variable_type("three with a null", "id"),
            Query::variable_type("three without one", "tag"),
            Query::variable_type("a bare nullable stays as written", "up"),
        ],
        expect![[r#"
            three with a null
            - string|int|null
            three without one
            - string|int|bool
            a bare nullable stays as written
            - ?Node
        "#]],
    );
}

/// `?false` is not PHP and `true|false` is a spelling of `bool`. Both come out of a union
/// constructor, so both are checked in each of the two spellings a union arrives in.
#[test]
fn the_boolean_literals_are_spelled_the_way_php_spells_them() {
    check_view(
        r#"
//- /src/Nodey.php
<?php
namespace App;

interface Nodey
{
    /** @return false|null */
    public function missing();

    /** @return true|false */
    public function flag();

    /** @return string|false|null */
    public function read();

    /** @return bool|true */
    public function absorbed();
}

//- /src/Use.php
<?php
namespace App;

class Reader
{
    public function probe(Nodey $node, bool $flag): void
    {
        $missing = $node->missing();
        $mi@missing@ssing;
        $either = $node->flag();
        $ei@either@ther;
        $read = $node->read();
        $re@read@ad;
        $absorbed = $node->absorbed();
        $ab@absorbed@sorbed;
        $ternary = $flag ? true : false;
        $te@ternary@rnary;
        $orNull = $flag ? false : null;
        $or@orNull@Null;
    }
}
"#,
        &[
            Query::variable_type("a false that may be absent", "missing"),
            Query::variable_type("the two literals together", "either"),
            Query::variable_type("a false inside a longer union", "read"),
            Query::variable_type("a bool that already covers one", "absorbed"),
            Query::variable_type("the ternary everybody writes", "ternary"),
            Query::variable_type("the null a ternary added", "orNull"),
        ],
        expect![[r#"
            a false that may be absent
            - false|null
            the two literals together
            - bool
            a false inside a longer union
            - string|false|null
            a bool that already covers one
            - bool
            the ternary everybody writes
            - bool
            the null a ternary added
            - false|null
        "#]],
    );
}

/// The same two rules over types written in the file being read, which is a different constructor
/// and a different renderer.
#[test]
fn the_boolean_literals_are_spelled_the_same_way_when_written_here() {
    check_view(
        r#"
//- /src/Reader.php
<?php
namespace App;

class Reader
{
    /**
     * @param false|null $missing
     * @param true|false $either
     * @param string|false|null $read
     */
    public function probe($missing, $either, $read): void
    {
        $mi@missing@ssing;
        $ei@either@ther;
        $re@read@ad;
    }
}
"#,
        &[
            Query::variable_type("a false that may be absent", "missing"),
            Query::variable_type("the two literals together", "either"),
            Query::variable_type("a false inside a longer union", "read"),
        ],
        expect![[r#"
            a false that may be absent
            - false|null
            the two literals together
            - bool
            a false inside a longer union
            - string|false|null
        "#]],
    );
}

#[test]
fn a_nullable_built_around_a_union_puts_its_null_at_the_end() {
    // Nothing declares this one: the union is the array's value type and the null is what reading a
    // key that may be absent adds. So it is the `Held` renderer's arm rather than either type
    // renderer, and `?` there was applied to the whole union while binding to its first operand.
    check_view(
        r#"
//- /src/Use.php
<?php
namespace App;

class Reader
{
    /** @param array<string, string|array<int, string>> $rows */
    public function probe(array $rows): void
    {
        $one = $rows['a'] ?? NULL;
        $o@one@ne;
        $two = $rows['b'];
        $t@two@wo;
    }
}
"#,
        &[
            Query::variable_type("a union the read made nullable", "one"),
            Query::variable_type("and the same union without it", "two"),
        ],
        expect![[r#"
            a union the read made nullable
            - string|array<int, string>|null
            and the same union without it
            - string|array<int, string>
        "#]],
    );
}

#[test]
fn a_nullable_around_an_already_nullable_type_keeps_one_marker() {
    // The inner nullable is *interned*, inside a `Held::Stored`, so the outer wrap cannot see it by
    // matching the variant -- which is why `Held::nullable` folds what it can and this is asked of
    // the rendered text instead. Every other reader strips a nullable by recursing, so the doubling
    // was only ever visible here.
    check_view(
        r#"
//- /src/Use.php
<?php
namespace App;

class Reader
{
    /** @var array<string, string|int>|null */
    private ?array $held = NULL;

    public function probe(bool $c): void
    {
        $one = $c ? $this->held : NULL;
        $o@one@ne;
    }
}
"#,
        &[Query::variable_type("nullable joined with null", "one")],
        expect![[r#"
            nullable joined with null
            - ?array<string, string|int>
        "#]],
    );
}

/// A type beside its own nullable is the nullable, and the render says so once.
///
/// `$c ? $this->maybe() : 'x'` joins an interned `?string` with a written `string`, two values the
/// union constructor cannot tell are one claim -- so the join carried both and the render read
/// `?string|string`, on a hover card and in the message of every check that names the type. The
/// weaker claim is the one kept, which is the rule the key comparison already states.
#[test]
fn a_type_beside_its_nullable_renders_as_the_nullable() {
    check_view(
        r#"
//- /src/app.php
<?php
class Doc {}
class Svc
{
    private function maybe(): ?string { return null; }
    private function find(): ?Doc { return null; }

    public function run(bool $c): void
    {
        $s = $c ? $this->maybe() : 'x';
        $@scalar@s;
        $d = $c ? $this->find() : new Doc();
        $@class@d;
        $r = $c ? 'x' : $this->maybe();
        $@reversed@r;
    }
}
"#,
        &[
            Query::variable_type("a scalar beside its nullable", "scalar"),
            Query::variable_type("a class beside its nullable", "class"),
            Query::variable_type("the nullable written second", "reversed"),
        ],
        expect![[r#"
            a scalar beside its nullable
            - ?string
            a class beside its nullable
            - ?Doc
            the nullable written second
            - ?string
        "#]],
    );
}
