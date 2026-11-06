//! A Twig template's filters, functions and tests: the PHP an extension's construction says runs
//! each, and what that PHP returns.
//!
//! Neutral, as below the seam everywhere: the test states which methods construct which classes
//! into which catalogs, as the Twig provider does, and nothing here knows they are Twig's.

use expect_test::{Expect, expect};
use phrost_harness::{OpenScope, World};
use phrost_testkit::FixtureSpec;

use crate::{
    IndexView,
    twig::{TwigCallableKind, TwigSymbolKind},
};

const PHP: &str = r#"
//- /vendor/acme/twig/Core.php
<?php
namespace Acme;

use Acme\Filter;

class Core
{
    public function getFilters(): array
    {
        return [
            new Filter('upper', [self::class, 'upper']),
            new Filter('t', 't'),
            new Filter('raw', null),
        ];
    }

    public function getFunctions(): array
    {
        return [new Func('path', [$this, 'path'])];
    }

    /** Makes it loud. */
    public static function upper(string $value): string { return $value; }

    public function path(string $route): \Url { return new \Url(); }
}

//- /vendor/acme/twig/functions.php
<?php
function t(string $text): Markup { return new Markup(); }

//- /src/Markup.php
<?php
class Markup {}
class Url { public function getHost(): string { return ''; } }
"#;

fn world() -> World {
    World::build_with(&FixtureSpec::parse(PHP), |world| {
        world.set_constructed_entries(&[
            ("getFilters", "Acme\\Filter", "filter"),
            ("getFunctions", "Acme\\Func", "function"),
        ]);
        world
            .contracts_mut()
            .set_twig_callables("filter", "function", "test");
    })
}

/// Renders what is at every `¦` in `twig`, and what is offered at the first.
fn check(twig: &str, expect: Expect) {
    let world = world();
    let mut text = String::new();
    let mut offsets = Vec::new();
    for ch in twig.chars() {
        match ch {
            '¦' => offsets.push(text.len()),
            _ => text.push(ch),
        }
    }
    let template = world.root().join("templates/page.html.twig");
    let open = world.open(OpenScope::EveryFile, None);
    let view = IndexView::new(world.skeleton(), &open);
    let mut out = String::new();
    for offset in &offsets {
        let Some(found) = view.twig_symbol_at(&template, &text, *offset) else {
            out.push_str(&format!("{offset} (nothing)\n"));
            continue;
        };
        let what = match &found.kind {
            TwigSymbolKind::Called(called) => format!(
                "{} {} runs {} (declared by {})",
                match called.kind {
                    TwigCallableKind::Filter => "filter",
                    TwigCallableKind::Function => "function",
                    TwigCallableKind::Test => "test",
                },
                called.name,
                called
                    .runs
                    .as_ref()
                    .map_or("(nothing named)".to_owned(), |runs| {
                        match &runs.container {
                            Some(class) => format!("{class}::{}", runs.fqn),
                            None => runs.fqn.to_string(),
                        }
                    }),
                called.declarer
            ),
            TwigSymbolKind::Variable { name, .. } => format!("variable {name}"),
            TwigSymbolKind::Member { location, .. } => format!(
                "member {}::{}",
                location
                    .container
                    .as_ref()
                    .map_or("", |class| class.as_str()),
                location.fqn
            ),
            other => format!("{other:?}"),
        };
        out.push_str(&format!(
            "{offset} {what}: {}\n",
            found.ty.as_deref().unwrap_or("?")
        ));
    }
    expect.assert_eq(&out);
}

#[test]
fn a_filter_and_a_function_are_the_php_their_construction_names() {
    check(
        "{{ title|up¦per }}{{ 'Hello'|¦t }}{{ body|ra¦w }}{{ pa¦th('home').host }}{{ x|nope¦ }}",
        expect![[r#"
            11 filter upper runs Acme\Core::upper (declared by Acme\Core): string
            28 filter t runs t (declared by Acme\Core): Markup
            42 filter raw runs (nothing named) (declared by Acme\Core): ?
            51 function path runs Acme\Core::path (declared by Acme\Core): Url
            78 (nothing)
        "#]],
    );
}

/// A filter's result is what its PHP returns, and a chain goes on from it.
#[test]
fn a_value_through_a_filter_is_what_the_filter_returns() {
    check(
        "{% set label = 'x'|t %}{{ lab¦el }}{% set url = path('a') %}{{ url.ho¦st }}",
        expect![[r#"
            29 variable label: Markup
            68 member Url::getHost: string
        "#]],
    );
}

#[test]
fn a_filter_being_written_is_offered_every_filter() {
    let world = world();
    let template = world.root().join("templates/page.html.twig");
    let open = world.open(OpenScope::EveryFile, None);
    let view = IndexView::new(world.skeleton(), &open);
    let text = "{{ title|u }}";
    let (replaced, items) = view
        .twig_completions(&template, text, 10, 50)
        .expect("a filter is being written");
    let mut out = format!("replaces {replaced:?}\n");
    for item in items {
        out.push_str(&format!(
            "{:?} {} {}\n",
            item.kind,
            item.name,
            item.detail.as_deref().unwrap_or("")
        ));
    }
    expect![[r#"
        replaces 9..10
        Filter raw 
        Filter t t()
        Filter upper Core::upper()
    "#]]
    .assert_eq(&out);
}
