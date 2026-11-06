//! A catalog key a declaration spells out of two of its own arguments.
//!
//! Nothing writes `entity.node.canonical` down. Drupal generates one route per entry of an entity
//! type's `links`, naming it `entity.<id>.<template>` with hyphens turned into underscores, so the
//! route catalog read out of YAML holds 1 052 keys on the reference project and misses the 298 its
//! own code writes most -- 42 call sites for `entity.node.canonical` alone.
//!
//! Two things separate this from the [`super::annotation_keys`] kind of claim. The key is *computed*
//! from two literals of the same declaration rather than being one of them; and it names **nothing**,
//! because a route is not a class -- recording the declaring class as its value would make a call
//! bound to that catalog return one, which is the trap the entity catalog's own documentation warns
//! about. What travels instead is the entry's value, which for a link template is the path, and
//! which is the same line a route read from YAML shows beside its name.
//!
//! Neutral names throughout, as everywhere in this crate: what a catalog means is a framework's
//! business and nothing at this layer may know.

use std::path::PathBuf;

use expect_test::expect;
use phrost_model::{FileId, SourceOrigin};

use super::{Step, check_incremental};
use crate::{KeyedAttributes, ProjectSkeleton};

/// A skeleton with the one rule these tests are about, plus the plain key rule beside it.
fn skeleton() -> ProjectSkeleton {
    let mut skeleton = ProjectSkeleton::new();
    skeleton.set_root("/fixture");
    skeleton
        .returns_mut()
        .name_keys_with("Plugin", "id", "kind");
    skeleton.returns_mut().derive_keys_with(
        "Plugin",
        "links",
        "id",
        "page.{name}.{key}",
        &[("x:", ""), ("-", "_")],
        "page",
    );
    skeleton.set_keyed_attributes(KeyedAttributes::of(["Plugin"]).entering(["links"]));
    skeleton
}

/// Every catalog key the skeleton holds a claim for, with what each names.
fn claimed(skeleton: &ProjectSkeleton) -> String {
    skeleton
        .returns()
        .declared_keys()
        .into_iter()
        .map(|(catalog, key, classes)| match classes.is_empty() {
            true => format!("{catalog} {key} -> (names nothing)\n"),
            false => format!("{catalog} {key} -> {}\n", classes.join(", ")),
        })
        .collect()
}

#[test]
fn each_entry_spells_one_key_and_the_rewrites_are_applied_in_order() {
    let mut skeleton = skeleton();
    skeleton.ingest(
        FileId::from_raw(0),
        &PathBuf::from("/fixture/src/Report.php"),
        SourceOrigin::User,
        r#"<?php
namespace App\Reports;

#[Plugin(
    id: 'report',
    links: [
        'canonical' => '/report/{report}',
        'edit-form' => '/report/{report}/edit',
        'x:translation-overview' => '/report/{report}/translations',
    ],
    handlers: [
        'storage' => 'App\Reports\ReportStorage',
    ],
)]
class Report {}
"#,
    );

    expect![[r#"
        kind report -> App\Reports\Report
        page page.report.canonical -> (names nothing)
        page page.report.edit_form -> (names nothing)
        page page.report.translation_overview -> (names nothing)
    "#]]
    .assert_eq(&claimed(&skeleton));
}

#[test]
fn a_derived_key_describes_itself_and_resolves_to_no_class() {
    let mut skeleton = skeleton();
    skeleton.ingest(
        FileId::from_raw(0),
        &PathBuf::from("/fixture/src/Report.php"),
        SourceOrigin::User,
        r#"<?php
namespace App\Reports;

#[Plugin(id: 'report', links: ['canonical' => '/report/{report}'])]
class Report {}
"#,
    );

    let contracts = skeleton.returns();
    assert!(contracts.catalog_holds("page", "page.report.canonical"));
    // Where the class would be for a service id. A key generated at runtime has no file to send a
    // reader to, so what its own entry wrote is the whole of the answer.
    assert_eq!(
        contracts.catalog_detail("page", "page.report.canonical"),
        Some("/report/{report}")
    );
    // The one that matters: a call bound to this catalog must not start returning a `Report`.
    assert_eq!(
        contracts.catalog_class("page", "page.report.canonical"),
        None
    );
    // And the plain key beside it still names the class it always did, from the same declaration.
    assert_eq!(
        contracts.catalog_class("kind", "report"),
        Some("App\\Reports\\Report")
    );
}

#[test]
fn editing_the_entries_withdraws_the_keys_the_old_ones_spelled() {
    // The invariant every per-file claim has to hold, and the one a derived key could most easily
    // break: what is withdrawn is re-derived from the rows still held for the file, so a key spelled
    // from an entry no longer written has to leave with it. Two derivations of this is how a
    // re-ingested file leaves a key behind.
    check_incremental(
        r#"
//- /src/Report.php
<?php
#[Plugin(id: 'report', links: ['canonical' => '/report/{report}', 'edit-form' => '/report/{report}/edit'])]
class Report {}
"#,
        &[Step::write(
            "src/Report.php",
            r#"<?php
#[Plugin(id: 'report', links: ['canonical' => '/report/{report}'])]
class Report {}
"#,
        )],
        expect![[r#"
            state ready
            file src/Report.php [User]
              class Report @ 3:7-3:13
              Report is class
            catalog
              [class] report -> src/Report.php (Report)
            namespaces
            declared keys
              [page] page.report.canonical -> (names nothing)
              [plugin] report -> Report
            dispatched keys
        "#]],
    );
}
