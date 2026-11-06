//! What is reported about a file that is not PHP.
//!
//! A framework declares its services, routes and permissions in configuration files and reads them
//! from PHP, so a key nothing registers is the same mistake in either language -- and the decision
//! about which misses are worth reporting is [one function](crate::diagnostics) shared by both. What
//! this pins is that the shared decision is reached from here at all, and above all the silence:
//! reported on the reference project's 72 real YAML files, this produces **nothing**, and a check
//! that produced nothing because it never ran would look exactly the same.

use expect_test::{Expect, expect};
use phrost_harness::{ConfigTarget, Place, World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::config_diagnostics;

fn check(file_name: &str, yaml: &str, expect: Expect) {
    let spec = FixtureSpec::parse("//- /src/app.php\n<?php\nclass Anything {}\n");
    let world = World::build_with(&spec, |world| {
        let contracts = world.contracts_mut();
        contracts.reference_in_config(
            "services.yml",
            "",
            "@",
            "?",
            ConfigTarget::CatalogKey("service".into()),
        );
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
        contracts.catalog_entry("service", "app.repository", Some("App\\Repo"));
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
    });

    let rendered = with_open_scopes(&world, world.any_file(), |open| {
        let view = IndexView::new(world.skeleton(), open);
        let mut out = String::new();
        for found in config_diagnostics(&view, file_name, yaml) {
            let at = &yaml[usize::from(found.range.start())..usize::from(found.range.end())];
            out.push_str(&format!("{:?} `{at}` {}\n", found.severity, found.message));
        }
        match out.is_empty() {
            true => "(nothing reported)\n".to_owned(),
            false => out,
        }
    });

    expect.assert_eq(&rendered);
}

#[test]
fn a_closed_catalog_proves_the_line_cannot_run() {
    check(
        "app.services.yml",
        "services:\n  a:\n    arguments: ['@app.repository', '@app.repositry', '@nothing.at.all']\n",
        expect![[r#"
            Error `app.repositry` Nothing registers the service `app.repositry`. Did you mean `app.repository`?
            Error `nothing.at.all` Nothing registers the service `nothing.at.all`
        "#]],
    );
}

#[test]
fn an_open_catalog_reports_a_near_miss_and_nothing_else() {
    // The whole of what makes this liveable, and the reason it is pinned here rather than a layer
    // down: a route can be generated at runtime and a permission can come from a callback, so a
    // name merely absent from either catalog may still be real. Only a name that *nearly hits* one
    // it holds is a mistake, and then only as a warning -- a weaker claim than the errors above,
    // and honestly so.
    check(
        "app.routing.yml",
        "one:\n  requirements:\n    _permission: 'administer things'\ntwo:\n  requirements:\n    _permission: 'administer thingz'\nthree:\n  requirements:\n    _permission: 'a thing nothing is near'\n",
        expect![[r#"
            Warning `administer thingz` No permission is registered as `administer thingz`. Did you mean `administer things`?
        "#]],
    );
}

#[test]
fn an_optional_id_is_reported_only_when_it_nearly_hits() {
    // `'@?id'` asks the container for null where nothing registers `id`, so an absence is what the
    // line was written for and only a typo is worth saying anything about -- and then as a warning,
    // since the line still runs. The `?` is not part of any name reported.
    check(
        "app.services.yml",
        "services:\n  a:\n    arguments: ['@?app.repository', '@?app.repositry', '@?nothing.at.all']\n",
        expect![[r#"
            Warning `app.repositry` No service is registered as `app.repositry`. Did you mean `app.repository`?
        "#]],
    );
}

#[test]
fn a_file_no_rule_covers_is_not_checked() {
    // Which file it is in is half of what a rule says, so the same text elsewhere is not a mistake
    // -- it is not a reference.
    check(
        "app.links.menu.yml",
        "one:\n  arguments: ['@nothing.at.all']\n",
        expect!["(nothing reported)\n"],
    );
}
