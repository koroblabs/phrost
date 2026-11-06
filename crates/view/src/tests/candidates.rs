//! Name completion offers project code before vendor and stubs -- see [`crate::candidates`].
//!
//! Unlike the rest of this module, these tests call [`IndexView::candidates_with_prefix`] directly
//! instead of going through [`super::Query`]: the property under test is about `limit`, a parameter
//! [`Query::Completion`] has no way to vary, and the whole point of the second test below is a
//! `limit` too small for every match to fit.

use phrost_harness::World;
use phrost_index::OpenSet;
use phrost_model::TextSize;

use crate::{Candidate, IndexView, NameKinds};

/// A vendor class sorting alphabetically before the project's own, both matching an empty prefix --
/// the shape of the real regression, where nothing typed yet means every class in the catalog
/// matches and a plain alphabetical walk exhausts the standard library before reaching user code.
const PROJECT: &str = r#"
//- /vendor/acme/src/AaaWidget.php
<?php
namespace Acme;

class AaaWidget {}

//- /src/Repo.php
<?php
namespace App;

class Repo {}
"#;

fn short_names(candidates: &[Candidate]) -> Vec<&str> {
    candidates.iter().map(Candidate::short).collect()
}

/// With room for every match, the project's class still comes first even though `Aaa` sorts before
/// `Repo` -- origin outranks the alphabet, not just breaks ties within it.
#[test]
fn a_project_class_outranks_a_vendor_class_that_sorts_first() {
    let world = World::parse(PROJECT);
    let open = OpenSet::default();
    let view = IndexView::new(world.skeleton(), &open);

    let candidates = view.candidates_with_prefix(TextSize::new(0), "", NameKinds::ClassOnly, 200);

    assert_eq!(short_names(&candidates), vec!["Repo", "AaaWidget"]);
}

/// The cap has to count what survives ranking, not what a name-ordered walk happened to reach
/// first: a `limit` of one must still come back with the project's class, not the vendor one that
/// the alphabet would have handed a naive cap first.
#[test]
fn a_cap_too_small_for_both_keeps_the_project_class() {
    let world = World::parse(PROJECT);
    let open = OpenSet::default();
    let view = IndexView::new(world.skeleton(), &open);

    let candidates = view.candidates_with_prefix(TextSize::new(0), "", NameKinds::ClassOnly, 1);

    assert_eq!(short_names(&candidates), vec!["Repo"]);
}

/// A vendor attribute, a project attribute and a project class that sorts before both.
const ATTRIBUTES: &str = r#"
//- /vendor/acme/src/Route.php
<?php
namespace Acme;

#[\Attribute(\Attribute::TARGET_METHOD)]
final class Route {}

//- /src/Alpha.php
<?php
namespace App;

class Alpha {}

//- /src/Marker.php
<?php
namespace App;

#[Attribute]
final readonly class Marker {}
"#;

/// After a `#[`, the classes that carry `#[Attribute]` come first -- including a vendor one, which
/// every other position ranks behind the project's own code.
///
/// The ranking is what the position is for: a real checkout holds a few dozen attribute classes
/// among thirty thousand class-likes, so an alphabetical page of two hundred reaches none of them.
/// Not a filter -- `Alpha` is still offered, because PHP refuses a class that is not an attribute
/// only when something reflects on it.
#[test]
fn an_attribute_position_offers_the_attributes_first() {
    let world = World::parse(ATTRIBUTES);
    let open = OpenSet::default();
    let view = IndexView::new(world.skeleton(), &open);

    let candidates = view.candidates_with_prefix(TextSize::new(0), "", NameKinds::Attribute, 200);

    assert_eq!(
        short_names(&candidates),
        vec!["Marker", "Route", "Alpha"],
        "the two attributes first, then the project's own class, then nothing from the alphabet"
    );
}

/// The same names at a position that is not an attribute's: origin decides and the alphabet breaks
/// the tie, exactly as before. The tier belongs to the one place that asks for it.
#[test]
fn every_other_position_ranks_them_the_way_it_always_did() {
    let world = World::parse(ATTRIBUTES);
    let open = OpenSet::default();
    let view = IndexView::new(world.skeleton(), &open);

    let candidates = view.candidates_with_prefix(TextSize::new(0), "", NameKinds::ClassOnly, 200);

    assert_eq!(short_names(&candidates), vec!["Alpha", "Marker", "Route"]);
}

/// A prefix narrows the tier the same way it narrows the walk, and a cap too small for everything
/// still spends itself on the attributes.
#[test]
fn a_prefix_narrows_the_attribute_tier_and_the_cap_keeps_it() {
    let world = World::parse(ATTRIBUTES);
    let open = OpenSet::default();
    let view = IndexView::new(world.skeleton(), &open);

    let narrowed = view.candidates_with_prefix(TextSize::new(0), "Ro", NameKinds::Attribute, 200);
    assert_eq!(short_names(&narrowed), vec!["Route"]);

    let capped = view.candidates_with_prefix(TextSize::new(0), "", NameKinds::Attribute, 1);
    assert_eq!(short_names(&capped), vec!["Marker"]);
}
