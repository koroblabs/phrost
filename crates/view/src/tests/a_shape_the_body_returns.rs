//! The `@return` a declaration saying only `array` should have.
//!
//! Every declaration of the fixture in one table, because what decides this is which shapes are
//! *refused*: a table shows the accepted case beside the eight refusals, where a test per case
//! would show each refusal as an absence and read as coverage of something.
//!
//! The projection rather than the code action, for the reason §6 gives -- the rule about what a
//! body states is the index's, and the layout of the tag it ends up in is the action's.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;

use crate::IndexView;

/// Every method and free function of `app.php`, with what a generated `@return` would say.
fn check(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let file = world.file_id("app.php");

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let hir = view.current().expect("the file is open").hir();
        let mut out = String::new();
        let members = hir
            .classes()
            .flat_map(|(_, class)| hir.member_list(class.members).iter())
            .map(|&id| hir.member(id))
            .map(|member| (member.name.clone(), member.name_range));
        let functions = hir
            .functions()
            .map(|(_, function)| (function.name.clone(), function.name_range));
        for (name, at) in members.chain(functions) {
            let answer = view
                .documentable_at(at.start())
                .and_then(|found| found.returns)
                .unwrap_or_else(|| "(no tag)".to_owned());
            out.push_str(&format!("{name}() -> {answer}\n"));
        }
        out
    });

    expect.assert_eq(&rendered);
}

#[test]
fn a_keyed_literal_says_what_each_key_holds() {
    check(
        r#"
//- /app.php
<?php
namespace App;

class File
{
    public static function load(int $id): ?File
    {
        return NULL;
    }
}

class Sales
{
    public function retail(): ?array
    {
        if ($this->rate() < 0.0) {
            return NULL;
        }
        return [
            'date' => 'today',
            'rate' => (float) $this->rate(),
            'count' => 12,
            'file' => File::load(1),
        ];
    }

    public function rate(): float
    {
        return 1.0;
    }
}
"#,
        expect![[r#"
            load() -> ?\App\File
            retail() -> ?array{date: non-falsy-string, rate: float, count: int, file: ?\App\File}
            rate() -> float
        "#]],
    );
}

/// The nine shapes that give the declaration back its own word, each for a different reason.
#[test]
fn everything_the_literal_does_not_settle_keeps_the_bare_word() {
    check(
        r#"
//- /app.php
<?php
namespace App;

class Refusals
{
    // A second return the reader cannot see into: all or nothing.
    public function mixed_returns(): array
    {
        if ($this->flag()) {
            return ['a' => 1];
        }
        return $this->rows();
    }

    // Two literals naming two shapes describe two values, not one.
    public function disagreeing(): array
    {
        if ($this->flag()) {
            return ['a' => 1];
        }
        return ['b' => 2];
    }

    // A bare element means the array is a list, and a list is not a record.
    public function a_list(): array
    {
        return [1, 2, 3];
    }

    // A key nothing can identify names no slot.
    public function computed_key(string $slug): array
    {
        return [$slug => 1];
    }

    // What a spread contributes was written somewhere else.
    public function spread(array $rest): array
    {
        return ['a' => 1, ...$rest];
    }

    // Nothing to say about an array with nothing in it.
    public function empty_literal(): array
    {
        return [];
    }

    // A declaration that already names its contents made a claim of its own.
    /** @return array<string, int> */
    public function already_said(): array
    {
        return ['a' => 1];
    }

    // A `return` inside a generator hands its value to `getReturn()`, not to the caller.
    public function generating(): array
    {
        yield 1;
        return ['a' => 1];
    }

    public function flag(): bool
    {
        return TRUE;
    }

    public function rows(): array
    {
        return [];
    }
}
"#,
        expect![[r#"
            mixed_returns() -> array
            disagreeing() -> array
            a_list() -> array
            computed_key() -> array
            spread() -> array
            empty_literal() -> array
            already_said() -> array<string, int>
            generating() -> array
            flag() -> bool
            rows() -> array
        "#]],
    );
}

/// The keys PHP writes that PHPDoc cannot: an unquoted `#type` reads back as a bare `array`, so
/// the whole shape is lost with nothing failing anywhere.
#[test]
fn a_key_that_is_not_an_identifier_is_quoted() {
    check(
        r#"
//- /app.php
<?php
namespace App;

function element(): array
{
    return [
        '#type' => 'textfield',
        '#title' => 'Name',
        'plain' => 1,
    ];
}
"#,
        expect![[r#"
            element() -> array{'#type': non-falsy-string, '#title': non-falsy-string, plain: int}
        "#]],
    );
}

/// A free function is the other half, and a Drupal module is made of them -- the gap §8 records
/// for `documentable_function`, which had this whole feature silent on exactly the files it was
/// written for.
#[test]
fn a_free_function_is_read_the_same_way() {
    check(
        r#"
//- /app.php
<?php
namespace App;

function summary(int $id): array
{
    return [
        'id' => $id,
        'label' => 'x',
    ];
}
"#,
        expect![[r#"
            summary() -> array{id: int, label: non-falsy-string}
        "#]],
    );
}
