//! Template names written in a Twig template, what each refers to, and which misses are mistakes.
//!
//! Neutral, as everywhere below the framework seam: the test files the catalogs a provider would,
//! under names it chose, and nothing here knows whose namespaces these are. What is tested is the
//! reader and the judgement -- when a name a catalog does not hold proves a template missing.

use crate::{ConfigReferent, IndexView};
use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_index::Place;
use phrost_testkit::FixtureSpec;

/// Renders what is referred to at every `|` in `twig`, then every name a reader would report.
fn check(twig: &str, expect: Expect) {
    let world = World::build_with(&FixtureSpec::parse(PHP), install);

    let mut text = String::with_capacity(twig.len());
    let mut offsets = Vec::new();
    for ch in twig.chars() {
        match ch {
            '|' => offsets.push(text.len()),
            _ => text.push(ch),
        }
    }

    let rendered = with_open_scopes(&world, world.any_file(), |open| {
        let view = IndexView::new(world.skeleton(), open);
        let mut out = String::new();
        for offset in &offsets {
            out.push_str(&format!("{offset} "));
            match view.config_reference_at("page.html.twig", &text, *offset) {
                None => out.push_str("(nothing)\n"),
                Some(found) => {
                    let what = match &found.referent {
                        ConfigReferent::CatalogEntry { place, detail } => match place {
                            Some(place) => format!("-> {} ({detail})", place.path.display()),
                            None => format!("-> (no file) {detail}"),
                        },
                        ConfigReferent::Unregistered { catalog } => {
                            format!("-> nothing registers this {catalog}")
                        }
                        other => format!("-> {other:?}"),
                    };
                    out.push_str(&format!("`{}` {what}\n", found.name));
                }
            }
        }
        for problem in view.config_problems("page.html.twig", &text) {
            let used = problem.used;
            out.push_str(&format!(
                "problem `{}` at {:?}: closed {}, nearest {:?}\n",
                used.key, problem.range, used.closed, used.nearest
            ));
        }
        out
    });

    expect.assert_eq(&rendered);
}

const PHP: &str = r#"
//- /src/Nothing.php
<?php
"#;

/// The loader and the catalogs a provider would have contributed.
fn install(world: &mut World) {
    let contracts = world.contracts_mut();
    contracts.add_twig_loader(phrost_index::TwigLoader {
        templates: "template".into(),
        aliases: "template path".into(),
        namespaces: "template namespace".into(),
        unwalked: ["src".into()].into(),
        globals: Box::default(),
    });
    contracts.catalog_place(
        "template path",
        "@mine/templates/parts/thumb.html.twig",
        Place {
            path: std::path::Path::new("/mine/templates/parts/thumb.html.twig").into(),
            line: 0,
            character: 0,
            detail: "".into(),
        },
    );
    for (key, path) in [
        (
            "@mine/parts/thumb.html.twig",
            "/mine/templates/parts/thumb.html.twig",
        ),
        ("thumb.html.twig", "/mine/templates/parts/thumb.html.twig"),
        (
            "@mine/icons/logo.svg.twig",
            "/mine/templates/icons/logo.svg.twig",
        ),
        (
            "navigation:toolbar-button",
            "/navigation/components/toolbar-button.twig",
        ),
    ] {
        contracts.catalog_place(
            "template",
            key,
            Place {
                path: std::path::Path::new(path).into(),
                line: 0,
                character: 0,
                detail: path.trim_start_matches('/').into(),
            },
        );
    }
    contracts.catalog_place(
        "template namespace",
        "mine",
        Place {
            path: std::path::Path::new("/mine/mine.info.yml").into(),
            line: 0,
            character: 0,
            detail: "mine/templates".into(),
        },
    );
}

#[test]
fn a_name_the_catalog_holds_is_the_file_it_names() {
    check(
        "{% include '|@mine/parts/thu|mb.html.twig|' with {a: 1} %}\n{{ include('|thumb.html.twig') }}\n{% embed 'navigation:toolbar|-button' %}{% endembed %}\n<p>|@mine</p>",
        expect![[r#"
            12 `@mine/parts/thumb.html.twig` -> /mine/templates/parts/thumb.html.twig (mine/templates/parts/thumb.html.twig)
            27 `@mine/parts/thumb.html.twig` -> /mine/templates/parts/thumb.html.twig (mine/templates/parts/thumb.html.twig)
            39 `@mine/parts/thumb.html.twig` -> /mine/templates/parts/thumb.html.twig (mine/templates/parts/thumb.html.twig)
            68 `thumb.html.twig` -> /mine/templates/parts/thumb.html.twig (mine/templates/parts/thumb.html.twig)
            117 `navigation:toolbar-button` -> /navigation/components/toolbar-button.twig (navigation/components/toolbar-button.twig)
            146 (nothing)
        "#]],
    );
}

/// The namespace was walked whole, so a `.twig` name under it the catalog does not hold is a
/// template that is not there -- whether or not anything is near it.
#[test]
fn a_name_missing_from_a_namespace_walked_whole_is_a_mistake() {
    check(
        "{% include '@mine/parts/thum|.html.twig' %}\n{% extends '@mine/la|yout.html.twig' %}",
        expect![[r#"
            28 `@mine/parts/thum.html.twig` -> nothing registers this template
            63 `@mine/layout.html.twig` -> nothing registers this template
            problem `@mine/parts/thum.html.twig` at 12..38: closed true, nearest Some("@mine/parts/thumb.html.twig")
            problem `@mine/layout.html.twig` at 55..77: closed true, nearest None
        "#]],
    );
}

/// Each of these may be missing or may be there by a route the catalogs cannot see, so only a
/// near miss is said of them: a namespace something else registers, a bare name, a file of another
/// kind, a path climbing out of the namespace, and a name the template allows to be missing.
#[test]
fn a_name_nothing_can_rule_out_is_judged_by_how_near_it_is() {
    check(
        "{% include '@atoms/|button.html.twig' %}\n{% include 'thum.html.twig' %}\n{{ source('@mine/icons/|logo.svg') }}\n{{ source('@mine/../images/x.svg.twig') }}\n{% include '@mine/side.html.twig' ignore missing %}\n{% include ['@mine/a.html.twig', '@mine/b.html.twig'] %}",
        expect![[r#"
            19 (nothing)
            94 (nothing)
            problem `@atoms/button.html.twig` at 12..35: closed false, nearest None
            problem `thum.html.twig` at 52..66: closed false, nearest Some("thumb.html.twig")
            problem `@mine/icons/logo.svg` at 82..102: closed false, nearest None
            problem `@mine/../images/x.svg.twig` at 119..145: closed false, nearest None
            problem `@mine/side.html.twig` at 163..183: closed false, nearest None
            problem `@mine/a.html.twig` at 216..233: closed false, nearest None
            problem `@mine/b.html.twig` at 237..254: closed false, nearest None
        "#]],
    );
}

#[test]
fn a_name_being_written_is_offered_every_template() {
    let world = World::build_with(&FixtureSpec::parse(PHP), install);
    let text = "{% include '@mine/pa' %}";
    let rendered = with_open_scopes(&world, world.any_file(), |open| {
        let view = IndexView::new(world.skeleton(), open);
        let (replaced, items) = view
            .config_completions("page.html.twig", text, 20, 50)
            .expect("a name is being written");
        let mut out = format!("replaces {replaced:?}\n");
        for item in items {
            out.push_str(&format!("{} -- {}\n", item.name, item.detail));
        }
        out
    });
    expect![[r#"
        replaces 12..20
        @mine/parts/thumb.html.twig -- mine/templates/parts/thumb.html.twig
    "#]]
    .assert_eq(&rendered);
}

/// A project nothing renders Twig in holds no loader, and a template there is not read at all.
#[test]
fn without_a_loader_a_template_is_not_read() {
    let world = World::build(&FixtureSpec::parse(PHP));
    let text = "{% include '@mine/parts/thumb.html.twig' %}";
    let answered = with_open_scopes(&world, world.any_file(), |open| {
        let view = IndexView::new(world.skeleton(), open);
        format!(
            "covered {}, referent {}, problems {}",
            view.covers_configuration("page.html.twig"),
            view.config_reference_at("page.html.twig", text, 15)
                .is_some(),
            view.config_problems("page.html.twig", text).len(),
        )
    });
    assert_eq!(answered, "covered false, referent false, problems 0");
}

/// The same file through the namespace's other directory is the same file, and a name through a
/// directory the walk skipped is one nothing can rule out.
#[test]
fn another_directory_of_the_namespace_reaches_the_same_template() {
    check(
        "{% include '@mine/templates/parts/th|umb.html.twig' %}\n{% include '@mine/src/Gen|erated/x.html.twig' %}\n{% include '@mine/templates/parts/gone.html.twig' %}",
        expect![[r#"
            36 `@mine/templates/parts/thumb.html.twig` -> /mine/templates/parts/thumb.html.twig ()
            79 (nothing)
            problem `@mine/src/Generated/x.html.twig` at 66..97: closed false, nearest None
            problem `@mine/templates/parts/gone.html.twig` at 114..150: closed true, nearest None
        "#]],
    );
}
