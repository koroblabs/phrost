//! Names written in a framework's configuration files, and what each one refers to.
//!
//! Neutral throughout, as everywhere below the framework seam: the test states which key of which
//! file names which table, exactly as a provider does, and nothing here knows whose convention that
//! is. What is being tested is the scanner and the lookup, not Drupal.
//!
//! The scanner is the risky half. It reads one line rather than parsing the file, so what it has to
//! get right is which key governs a value, where a scalar begins and ends, and which of several
//! names in one scalar the cursor is on -- and each of those is a case below.

use crate::{ConfigCandidateKind, ConfigReferent, IndexView};
use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_index::{ConfigTarget, Place};
use phrost_testkit::FixtureSpec;

/// Renders what is referred to at every `|` in `yaml`, one line each.
///
/// Every position, not one: a line scanner is as likely to be wrong about where a scalar *ends* as
/// about where it begins, and a single probe in the middle of a value cannot see either edge.
fn check(php: &str, file_name: &str, yaml: &str, expect: Expect) {
    let spec = FixtureSpec::parse(php);
    let world = World::build_with(&spec, install);

    // The cursor markers are stripped and their offsets kept, the way a fixture's are.
    let mut text = String::with_capacity(yaml.len());
    let mut offsets = Vec::new();
    for ch in yaml.chars() {
        match ch {
            '|' => offsets.push(text.len()),
            _ => text.push(ch),
        }
    }
    assert!(!offsets.is_empty(), "the fixture marks no position");

    let rendered = with_open_scopes(&world, world.any_file(), |open| {
        let view = IndexView::new(world.skeleton(), open);
        let mut out = String::new();
        for offset in &offsets {
            let at = text[..*offset].chars().count();
            out.push_str(&format!("{at} "));
            match view.config_reference_at(file_name, &text, *offset) {
                None => out.push_str("(nothing)\n"),
                Some(found) => {
                    let what = match &found.referent {
                        ConfigReferent::Declaration(location) => match &location.container {
                            Some(class) => format!("-> {class}::{}", location.fqn),
                            None => format!("-> {}", location.fqn),
                        },
                        ConfigReferent::CatalogClass { class, .. } => format!("-> class {class}"),
                        ConfigReferent::CatalogEntry { place, detail } => match place {
                            Some(place) => {
                                format!("-> {}:{} {detail}", place.path.display(), place.line + 1)
                            }
                            None => format!("-> (no file) {detail}"),
                        },
                        ConfigReferent::Unregistered { catalog } => {
                            format!("-> nothing registers this {catalog}")
                        }
                    };
                    out.push_str(&format!("`{}` {what}\n", found.name));
                }
            }
        }
        out
    });

    expect.assert_eq(&rendered);
}

/// The rules and the catalogs a provider would have contributed, stated once for both harnesses.
fn install(world: &mut World) {
    let contracts = world.contracts_mut();
    contracts.reference_in_config(
        "services.yml",
        "",
        "@",
        "?",
        ConfigTarget::CatalogKey("service".into()),
    );
    contracts.reference_in_config("services.yml", "class", "", "", ConfigTarget::Class);
    contracts.reference_in_config("routing.yml", "_controller", "", "", ConfigTarget::Callable);
    contracts.reference_in_config(
        "routing.yml",
        "_permission",
        "",
        "",
        ConfigTarget::CatalogKeys {
            catalog: "permission".into(),
            separators: "+,".into(),
        },
    );
    contracts.catalog_entry("service", "app.repo", Some("App\\Repo"));
    // Closed, as a container read from a file that can be read in full is: a key missing from it
    // does not exist. The permission catalog below is left open, which is why a name missing from
    // *that* one answers nothing rather than claiming anything.
    contracts.close_catalog("service");
    contracts.catalog_place(
        "permission",
        "administer things",
        Place {
            path: std::path::Path::new("/app/app.permissions.yml").into(),
            line: 3,
            character: 0,
            detail: "Administer things".into(),
        },
    );
}

const CLASSES: &str = r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}
"#;

#[test]
fn a_container_id_is_recognised_by_the_way_it_is_written() {
    // Under no key of its own -- a bare list item -- which is why this rule is recognised by the
    // `@` and not by a key. The prefix is not part of the name, so the range and the answer are
    // both about what follows it.
    check(
        CLASSES,
        "app.services.yml",
        "services:\n  app.thing:\n    class: App\\Repo\n    arguments: ['@|app.re|po|', '@|app.nope|']\n",
        expect![[r#"
            61 `app.repo` -> class App\Repo
            67 `app.repo` -> class App\Repo
            69 `app.repo` -> class App\Repo
            74 `app.nope` -> nothing registers this service
            82 `app.nope` -> nothing registers this service
        "#]],
    );
}

#[test]
fn an_optional_container_id_is_the_name_after_its_mark() {
    // `'@?id'` is the container's spelling for a service that may not be there. The `?` belongs
    // to the spelling and not to the name, so a position on it is on nothing -- the same as the `@`.
    check(
        CLASSES,
        "app.services.yml",
        "services:\n  app.thing:\n    arguments: ['@|?|app.re|po|', '@?|app.nope|']\n",
        expect![[r#"
            41 (nothing)
            42 `app.repo` -> class App\Repo
            48 `app.repo` -> class App\Repo
            50 `app.repo` -> class App\Repo
            56 `app.nope` -> nothing registers this service
            64 `app.nope` -> nothing registers this service
        "#]],
    );
}

#[test]
fn a_class_is_found_with_or_without_its_leading_separator() {
    // Both spellings are written in these files -- a route's controller carries the separator and a
    // service's class does not -- and they name one class.
    check(
        CLASSES,
        "app.services.yml",
        "services:\n  one:\n    class: |App\\Repo|\n  two:\n    class: \\App|\\Repo\n  three:\n    class: App\\Miss|ing\n",
        expect![[r#"
            28 `App\Repo` -> App\Repo
            36 `App\Repo` -> App\Repo
            59 `\App\Repo` -> App\Repo
            93 (nothing)
        "#]],
    );
}

#[test]
fn the_key_is_not_a_reference_and_neither_is_a_file_no_rule_covers() {
    // Two silences that matter. A cursor on `class` is on the key, which names nothing -- and the
    // same value in a file no rule covers is not a reference either, because which file it is in is
    // half of what the rule says.
    check(
        CLASSES,
        "app.services.yml",
        "services:\n  app.thing:\n    cl|ass: App\\Repo\n",
        expect![[r#"
            29 (nothing)
        "#]],
    );
    check(
        CLASSES,
        "app.links.menu.yml",
        "services:\n  app.thing:\n    class: A|pp\\Repo\n",
        expect![[r#"
            35 (nothing)
        "#]],
    );
}

#[test]
fn a_callable_lands_on_the_method_when_one_is_named() {
    // And on the class when none is, which is the difference between this and a plain class: a
    // controller is written both ways in one file.
    check(
        CLASSES,
        "app.routing.yml",
        "app.one:\n  defaults:\n    _controller: '\\App\\R|epo::find'\napp.two:\n  defaults:\n    _controller: '\\App\\Re|po'\napp.three:\n  defaults:\n    _controller: '\\App\\Repo::mis|sing'\n",
        expect![[r#"
            45 `\App\Repo::find` -> App\Repo::find
            102 `\App\Repo` -> App\Repo
            161 `\App\Repo::missing` -> App\Repo
        "#]],
    );
}

#[test]
fn one_of_several_names_in_a_scalar_is_the_one_the_cursor_is_on() {
    // `+` and `,` are two different meanings to the framework and the same thing to a reader: each
    // side is a name of its own, and the one being asked about is the one under the cursor.
    check(
        CLASSES,
        "app.routing.yml",
        "app.one:\n  requirements:\n    _permission: 'admin|ister things+|nothing at all'\n",
        expect![[r#"
            48 `administer things` -> /app/app.permissions.yml:4 Administer things
            61 (nothing)
        "#]],
    );
}

#[test]
fn a_position_past_the_end_of_a_value_is_not_on_it() {
    // The edge a single probe in the middle of a scalar cannot see. A trailing comment is not part
    // of the value either, and a `#` with no space before it is -- which is how a render array's
    // keys are written.
    check(
        CLASSES,
        "app.services.yml",
        "services:\n  app.thing:\n    class: App\\Repo  |# a comment|\n",
        expect![[r#"
            44 (nothing)
            55 (nothing)
        "#]],
    );
}

/// Renders what could be written at every `|` in `yaml`, and what accepting one would replace.
///
/// The replaced range is shown because it is half of what makes a completion correct: a client that
/// is handed the wrong one leaves `app_core.app_core.accounts` in the buffer, and a list of labels
/// alone cannot see that.
fn offers(php: &str, file_name: &str, yaml: &str, expect: Expect) {
    let spec = FixtureSpec::parse(php);
    let world = World::build_with(&spec, install);

    let mut text = String::with_capacity(yaml.len());
    let mut offsets = Vec::new();
    for ch in yaml.chars() {
        match ch {
            '|' => offsets.push(text.len()),
            _ => text.push(ch),
        }
    }

    let rendered = with_open_scopes(&world, world.any_file(), |open| {
        let view = IndexView::new(world.skeleton(), open);
        let mut out = String::new();
        for offset in &offsets {
            match view.config_completions(file_name, &text, *offset, 8) {
                None => out.push_str("(no place for a name)\n"),
                Some((replaced, items)) => {
                    let names = items
                        .iter()
                        .map(|item| match item.kind {
                            ConfigCandidateKind::CatalogKey => item.name.to_string(),
                            ConfigCandidateKind::Class => format!("class {}", item.name),
                            ConfigCandidateKind::Namespace => format!("namespace {}", item.name),
                            ConfigCandidateKind::Method => format!("method {}", item.name),
                        })
                        .collect::<Vec<_>>();
                    out.push_str(&format!(
                        "replaces `{}`: {}\n",
                        &text[replaced.clone()],
                        match names.is_empty() {
                            true => "(nothing)".to_owned(),
                            false => names.join(", "),
                        }
                    ));
                }
            }
        }
        out
    });

    expect.assert_eq(&rendered);
}

#[test]
fn an_id_is_offered_wherever_one_is_being_written() {
    // The three shapes a container reference is written in, and the empty one -- which is where a
    // person actually asks for the list, and which a reader keyed on an existing scalar cannot see.
    offers(
        CLASSES,
        "app.services.yml",
        "services:\n  a:\n    arguments: ['@app.r|', '@|']\n  b:\n    arguments:\n      - '@ap|p.repo'\n",
        expect![[r#"
            replaces `app.r`: app.repo
            replaces ``: app.repo
            replaces `app.repo`: app.repo
        "#]],
    );
}

#[test]
fn an_id_is_offered_after_the_optional_mark() {
    // The replaced range starts past the `?`, or accepting an item would write `@app.repo` over the
    // mark and quietly make an optional service a required one.
    offers(
        CLASSES,
        "app.services.yml",
        "services:\n  a:\n    arguments: ['@?app.r|', '@?|']\n",
        expect![[r#"
            replaces `app.r`: app.repo
            replaces ``: app.repo
        "#]],
    );
}

#[test]
fn a_name_with_spaces_in_it_keeps_the_words_already_typed() {
    // A permission is `administer things`, so a prefix that stopped at a space would offer every
    // permission there is to somebody who has already typed half of one. Inside quotes the whole
    // quoted run is the value.
    offers(
        CLASSES,
        "app.routing.yml",
        "app.one:\n  requirements:\n    _permission: 'admin|'\n    _permission: 'administer t|'\n",
        expect![[r#"
            replaces `admin`: administer things
            replaces `administer t`: administer things
        "#]],
    );
}

#[test]
fn one_of_several_names_is_offered_for_the_piece_being_written() {
    offers(
        CLASSES,
        "app.routing.yml",
        "app.one:\n  requirements:\n    _permission: 'administer things+admin|'\n",
        expect![[r#"
            replaces `admin`: administer things
        "#]],
    );
}

/// A class where configuration names one: the namespaces a path goes on into and the classes under
/// it, in the spelling the value was written in -- a service's `class:` without a leading
/// separator, a route's `_controller` with one -- and after `::`, the methods a controller can be.
///
/// Answered with an empty list until this, on the grounds that a class is copied rather than typed.
/// On the reference project a quarter of the YAML references are classes, and a path is typed a
/// segment at a time in every other place the editor offers one.
#[test]
fn a_class_and_a_controller_method_are_offered_where_configuration_names_one() {
    offers(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
    public function findAll(): void {}
    protected function hidden(): void {}
}

//- /src/Storage/Disk.php
<?php
namespace App\Storage;

class Disk {}
"#,
        "app.services.yml",
        "services:\n  a:\n    class: App\\Re|\n  b:\n    class: App\\St|\n  c:\n    class: Ap|\n",
        expect![[r#"
            replaces `App\Re`: class App\Repo
            replaces `App\St`: namespace App\Storage\
            replaces `Ap`: namespace App\
        "#]],
    );
    offers(
        CLASSES,
        "app.routing.yml",
        "app.one:\n  defaults:\n    _controller: '\\App\\R|'\napp.two:\n  defaults:\n    _controller: '\\App\\Repo::fi|'\n",
        expect![[r#"
            replaces `\App\R`: class \App\Repo
            replaces `\App\Repo::fi`: method \App\Repo::find
        "#]],
    );
}

#[test]
fn a_key_and_a_file_no_rule_covers_offer_nothing() {
    // The same two silences the reference reader keeps, asked of the other feature: a position that
    // is not where a name goes must not be answered with a list, or a client stops showing its own.
    offers(
        CLASSES,
        "app.services.yml",
        "services:\n  a:\n    cla|ss: App\\Repo\n",
        expect![[r#"
            (no place for a name)
        "#]],
    );
    offers(
        CLASSES,
        "app.links.menu.yml",
        "services:\n  a:\n    arguments: ['@app.r|']\n",
        expect![[r#"
            (no place for a name)
        "#]],
    );
}

/// Renders every name in `yaml` that its catalog does not hold, with the verdict's three inputs.
///
/// The verdict itself is the caller's -- `phrost_analysis::diagnostics::catalog::reported` -- so
/// what is pinned here is what it is handed: which catalog, whether that catalog claims to hold
/// everything, and what it is near. Rendering the message instead would test the other layer twice
/// and this one not at all.
fn problems(php: &str, file_name: &str, yaml: &str, expect: Expect) {
    let spec = FixtureSpec::parse(php);
    let world = World::build_with(&spec, install);

    let rendered = with_open_scopes(&world, world.any_file(), |open| {
        let view = IndexView::new(world.skeleton(), open);
        let mut out = String::new();
        for problem in view.config_problems(file_name, yaml) {
            out.push_str(&format!(
                "`{}` in {} ({}), near {}\n",
                &yaml[problem.range.clone()],
                problem.used.catalog,
                match problem.used.closed {
                    true => "closed",
                    false => "open",
                },
                problem.used.nearest.as_deref().unwrap_or("nothing"),
            ));
        }
        match out.is_empty() {
            true => "(no problem)\n".to_owned(),
            false => out,
        }
    });

    expect.assert_eq(&rendered);
}

#[test]
fn a_key_no_catalog_holds_is_handed_to_the_verdict_with_what_it_needs() {
    // The three inputs, and each one changes what is said: a closed catalog proves the line cannot
    // run, an open one can only say a name looks like a misspelling, and a near miss is what
    // separates a typo from a name that is merely absent.
    problems(
        CLASSES,
        "app.services.yml",
        "services:\n  a:\n    arguments: ['@app.repo', '@app.repp', '@nothing.like.it']\n",
        expect![[r#"
            `app.repp` in service (closed), near app.repo
            `nothing.like.it` in service (closed), near nothing
        "#]],
    );
}

#[test]
fn an_optional_ids_miss_is_handed_over_as_an_open_catalogs() {
    // The container passes null where nothing registers an optional id, so a miss behind the mark
    // does not prove the line cannot run -- the catalog is no less complete, but its completeness
    // no longer decides anything. The last one is the same name without the mark, as the control.
    problems(
        CLASSES,
        "app.services.yml",
        "services:\n  a:\n    arguments: ['@?app.repo', '@?app.repp', '@?nothing.like.it', '@nothing.like.it']\n",
        expect![[r#"
            `app.repp` in service (open), near app.repo
            `nothing.like.it` in service (open), near nothing
            `nothing.like.it` in service (closed), near nothing
        "#]],
    );
}

#[test]
fn an_open_catalogs_misses_come_back_with_what_they_are_near() {
    // Both of them, including the one nothing is near -- because the silence is the *caller's*, and
    // this layer's job is to hand over what the decision needs. What that decision does with an open
    // catalog and no near miss is pinned where it is made, in `tests::config_diagnostics`.
    //
    // It is the half that keeps this liveable: a route can be generated at runtime and a permission
    // can come from a callback, so a name missing from either catalog may still be real -- 21 of one
    // real project's own route references are exactly that.
    problems(
        CLASSES,
        "app.routing.yml",
        "app.one:\n  requirements:\n    _permission: 'administer things'\napp.two:\n  requirements:\n    _permission: 'administer thingz'\napp.three:\n  requirements:\n    _permission: 'unrelated'\n",
        expect![[r#"
            `administer thingz` in permission (open), near administer things
            `unrelated` in permission (open), near nothing
        "#]],
    );
}

#[test]
fn a_class_is_not_judged_yet_and_neither_is_a_file_no_rule_covers() {
    problems(
        CLASSES,
        "app.services.yml",
        "services:\n  a:\n    class: App\\NotThere\n",
        expect![[r#"
            (no problem)
        "#]],
    );
    problems(
        CLASSES,
        "app.links.menu.yml",
        "one:\n  arguments: ['@nothing.like.it']\n",
        expect![[r#"
            (no problem)
        "#]],
    );
}
