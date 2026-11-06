//! Parameter names drawn at call sites.

use std::fmt::Write as _;

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_model::{TextRange, TextSize};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::inlay_hints;

/// Renders every hint in the fixture's last file, as the line it lands on with the label inserted,
/// and what a client would show on demand under it.
fn check(fixture: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let path = spec
        .files()
        .last()
        .expect("the fixture declares a file")
        .path()
        .to_owned();
    let file = world.file_id(&path);
    let text = world.text_of(file).to_owned();
    let whole = TextRange::new(
        TextSize::from(0),
        TextSize::try_from(text.len()).expect("a fixture fits"),
    );

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let mut out = String::new();
        for hint in inlay_hints(&view, whole) {
            let at = world.render_range(file, TextRange::empty(hint.offset));
            let _ = writeln!(out, "{at} {}", hint.label);
            if let Some(tooltip) = &hint.tooltip {
                for line in tooltip.lines() {
                    let _ = writeln!(out, "    | {line}");
                }
            }
        }
        out
    });

    expect.assert_eq(&rendered);
}

const CALLEES: &str = r#"
//- /src/callees.php
<?php

class Mailer
{
    public function __construct(private string $host, private int $port) {}

    public function send(string $body, bool $urgent, int $retries): void {}

    public static function make(string $host): self {}
}

function format_amount(int $cents, string $currency): string {}

function join_all(string $glue, string ...$parts): string {}
"#;

#[test]
fn every_kind_of_call_gets_its_parameter_names() {
    check(
        &[
            CALLEES,
            r#"
//- /src/app.php
<?php

class Service
{
    public function run(Mailer $mailer): void
    {
        $mailer->send('hello', true, 3);
        Mailer::make('smtp.example');
        new Mailer('smtp.example', 25);
        format_amount(1999, 'USD');
    }
}
"#,
        ]
        .concat(),
        expect![[r#"
            7:23-7:23 body:
            7:32-7:32 urgent:
            7:38-7:38 retries:
            8:22-8:22 host:
            9:20-9:20 host:
            9:36-9:36 port:
            10:23-10:23 cents:
            10:29-10:29 currency:
        "#]],
    );
}

#[test]
fn nothing_is_drawn_where_it_would_repeat_what_is_written() {
    check(
        &[
            CALLEES,
            r#"
//- /src/app.php
<?php

class Service
{
    public function run(Mailer $mailer, string $body, bool $urgent): void
    {
        // `$body` and `$urgent` already say it; `$n` does not.
        $n = 3;
        $mailer->send($body, $urgent, $n);
    }
}
"#,
        ]
        .concat(),
        expect![[r#"
            9:39-9:39 retries:
        "#]],
    );
}

#[test]
fn a_named_argument_ends_the_correspondence() {
    check(
        &[
            CALLEES,
            r#"
//- /src/app.php
<?php

class Service
{
    public function run(Mailer $mailer): void
    {
        $mailer->send('hello', urgent: true, retries: 3);
    }
}
"#,
        ]
        .concat(),
        expect![[r#"
            7:23-7:23 body:
        "#]],
    );
}

#[test]
fn a_spread_and_a_variadic_are_left_alone() {
    check(
        &[
            CALLEES,
            r#"
//- /src/app.php
<?php

class Service
{
    public function run(array $parts): void
    {
        // The glue is one parameter and gets a hint; the rest go into a variadic under one name.
        join_all(', ', 'a', 'b');
        format_amount(...$parts);
    }
}
"#,
        ]
        .concat(),
        expect![[r#"
            8:18-8:18 glue:
        "#]],
    );
}

#[test]
fn a_call_that_does_not_resolve_draws_nothing() {
    check(
        r#"
//- /src/app.php
<?php

class Service
{
    public function run($untyped): void
    {
        $untyped->whatever('a', 'b');
        missing_function('a', 'b');
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn a_variable_gets_its_type_where_the_value_does_not_say_it() {
    // The rule that decides whether this is useful or unbearable. Most assignments in real code
    // state their own type, and a hint over one of those costs a reader the same screen space to
    // learn nothing -- which teaches them to stop reading hints at all.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo
{
    public static function load(int $id): Repo
    {
    }

    public function find(): ?Repo
    {
    }
}

class Factory
{
    public function make(): Repo
    {
    }
}

class Service
{
    public function run(Factory $factory): void
    {
        $made = $factory->make();
        $found = $made->find();
        $built = new Repo();
        $loaded = Repo::load(1);
        $count = 5;
        $negative = -1;
        $label = 'x';
        $rows = [];
        $legacy = array();
        $flag = true;
        $nothing = null;
        $cast = (int) $count;
        $copy = $made;
        $unknown = $undeclared;
    }
}
"#,
        expect![[r#"
            26:14-26:14 : Repo
            27:15-27:15 : ?Repo
            29:30-29:30 id:
            38:14-38:14 : Repo
        "#]],
    );
}

/// The type from the report this rule exists for: twelve keys of a item product's limits, 250
/// characters, drawn into a line of code somebody was reading.
///
/// Folded to the brace rather than to a bare `array`, which would say the value has no keys worth
/// knowing -- and the whole of it stays reachable, in what the client shows on demand.
#[test]
fn a_shape_too_wide_for_the_line_folds_to_its_brace() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Rules
{
    /**
     * @return ?array{package_type: string, weight_limit: string, distance_limit: int, gift_wrap: bool, gift_value: int, gift_amount: int, max_days_since_order: int, discount_reduction: float, rebate_rate: float, rebate_rate_floor: float, payout: bool, payout_max: int}
     */
    public function limits(): ?array
    {
    }

    /** @return array{app: string, count: int} */
    public function narrow(): array
    {
    }
}

class Service
{
    public function run(Rules $rules): void
    {
        $limits = $rules->limits();
        $row = $rules->narrow();
    }
}
"#,
        expect![[r#"
            23:16-23:16 : ?array{…}
                | ?array{
                |     package_type: string,
                |     weight_limit: string,
                |     distance_limit: int,
                |     gift_wrap: bool,
                |     gift_value: int,
                |     gift_amount: int,
                |     max_days_since_order: int,
                |     discount_reduction: float,
                |     rebate_rate: float,
                |     rebate_rate_floor: float,
                |     payout: bool,
                |     payout_max: int
                | }
            24:13-24:13 : array{app: string, count: int}
        "#]],
    );
}

/// A `match` whose arms return different classes, which is how a factory is written.
///
/// Eight arms of a real project's coordinator factory: the type is right and the hint drawn from it
/// is longer than the code it is drawn into. The fold is what the tooltip exists for.
#[test]
fn a_union_too_wide_for_the_line_keeps_what_fits() {
    check(
        r#"
//- /app.php
<?php

class DigitalGoodsCoordinator {}
class PerishableCoordinator {}
class BulkOrderCoordinator {}
class ExpressOvernightCoordinator {}
class RefurbishedKitCoordinator {}

class Factory
{
    public static function make(string $type): object
    {
        return match ($type) {
            'digital' => self::digital(),
            'fresh' => self::fresh(),
            'bulk' => self::bulk(),
            'express' => self::express(),
            'refurb' => self::refurb(),
        };
    }

    private static function digital(): DigitalGoodsCoordinator { return new DigitalGoodsCoordinator(); }

    private static function fresh(): PerishableCoordinator { return new PerishableCoordinator(); }

    private static function bulk(): BulkOrderCoordinator { return new BulkOrderCoordinator(); }

    private static function express(): ExpressOvernightCoordinator { return new ExpressOvernightCoordinator(); }

    private static function refurb(): RefurbishedKitCoordinator { return new RefurbishedKitCoordinator(); }

    public function run(string $type): void
    {
        $coordinator = match ($type) {
            'digital' => self::digital(),
            'fresh' => self::fresh(),
            'bulk' => self::bulk(),
            'express' => self::express(),
            'refurb' => self::refurb(),
        };
        $coordinator->run();
    }
}
"#,
        expect![[r#"
            34:21-34:21 : DigitalGoodsCoordinator|PerishableCoordinator|…
                | DigitalGoodsCoordinator
                | |PerishableCoordinator
                | |BulkOrderCoordinator
                | |ExpressOvernightCoordinator
                | |RefurbishedKitCoordinator
        "#]],
    );
}
