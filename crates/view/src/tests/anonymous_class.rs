//! `new class { ... }`: reachable from inside, unwritable from outside.
//!
//! An anonymous class is a real declaration -- `$this`, `self`, `static`, `parent` and `new self()`
//! inside its body all name it, and every member reached through them needs the catalog to hold it.
//! But its name is synthesised from where the `class` keyword sits, so nobody can type it and no
//! list of names to offer may contain it. Both halves are one behavior, because the temptation is
//! to keep the second by dropping the first -- which is what left every member access inside an
//! anonymous class body unresolved.

use expect_test::expect;
use phrost_harness::{OpenScope, World};
use phrost_model::TextSize;

use crate::{Candidate, IndexView, NameKinds};

use super::{Query, check_view};

/// One anonymous class extending a named one, exercising every way its own body can name it.
const FIXTURE: &str = r#"
//- /src/Product.php
<?php
namespace App;

class Product
{
    public function base(): string
    {
        return '';
    }
}

//- /src/Factory.php
<?php
namespace App;

class Factory
{
    public function make(): Product
    {
        return new cl@keyword@ass extends Product {
            private array $settings = [];

            public function grid(): mixed
            {
                return $this->set@this@tings;
            }

            public function viaSelf(): mixed
            {
                return self::gr@self@id();
            }

            public function viaStatic(): mixed
            {
                return static::gr@static@id();
            }

            public function viaParent(): string
            {
                return parent::ba@parent@se();
            }

            public function name(): string
            {
                return $this::cl@classconst@ass;
            }

            public function copy(): self
            {
                return new se@newself@lf();
            }
        };
    }
}
"#;

/// Every implicit receiver inside the body lands on the anonymous class itself -- except `parent`,
/// which lands on what it extends. The synthesised name is in the snapshot on purpose: it is the
/// identity the catalog holds, and a change to how it is derived should be visible here.
#[test]
fn every_implicit_receiver_inside_an_anonymous_class_resolves() {
    check_view(
        FIXTURE,
        &[
            Query::goto("$this->settings", "this"),
            Query::goto("self::grid()", "self"),
            Query::goto("static::grid()", "static"),
            Query::goto("parent::base()", "parent"),
            Query::goto("$this::class", "classconst"),
            Query::goto("new self()", "newself"),
            Query::goto("the class keyword", "keyword"),
        ],
        expect![[r#"
            $this->settings
            - property App\class@anonymous#99@77d19c00::settings @ src/Factory.php 9:27-9:36
            self::grid()
            - method App\class@anonymous#99@77d19c00::grid @ src/Factory.php 11:29-11:33
            static::grid()
            - method App\class@anonymous#99@77d19c00::grid @ src/Factory.php 11:29-11:33
            parent::base()
            - method App\Product::base @ src/Product.php 6:21-6:25
            $this::class
            - class App\class@anonymous#99@77d19c00 @ src/Factory.php 8:20-8:25
            new self()
            - class App\class@anonymous#99@77d19c00 @ src/Factory.php 8:20-8:25
            the class keyword
            - class App\class@anonymous#99@77d19c00 @ src/Factory.php 8:20-8:25
        "#]],
    );
}

/// The other half: the name the catalog now holds must never be offered as something to type.
///
/// Both prefix shapes, because they walk different structures. An unqualified prefix ranges over
/// the short-name index, which a synthesised name never enters; a qualified one scans the
/// class-like table itself, which it does, so that walk has to exclude it.
#[test]
fn completion_never_offers_a_synthesised_anonymous_class_name() {
    let world = World::parse(FIXTURE);
    let file = world.file_id("src/Factory.php");
    // Open, because a prefix carrying a separator is resolved against the current file's imports
    // and namespace before the catalog is searched at all.
    let open = world.open(OpenScope::CursorFileOnly, Some(file));
    let view = IndexView::in_file(world.skeleton(), &open, file);

    let unqualified = view.candidates_with_prefix(TextSize::new(0), "", NameKinds::ClassOnly, 200);
    let qualified =
        view.candidates_with_prefix(TextSize::new(0), "App\\", NameKinds::ClassOnly, 200);

    let names = |candidates: &[Candidate]| {
        candidates
            .iter()
            .map(|candidate| candidate.fqn.as_str().to_owned())
            .collect::<Vec<_>>()
    };

    assert_eq!(names(&unqualified), vec!["App\\Factory", "App\\Product"]);
    assert_eq!(names(&qualified), vec!["App\\Factory", "App\\Product"]);
}

/// The same for search, which is the other surface a name is offered from -- workspace symbols and
/// the missing-import code action both read it. Excluded structurally rather than by a filter: an
/// unnameable declaration never enters the short-name index the search ranges over.
#[test]
fn search_never_finds_a_synthesised_anonymous_class_name() {
    let world = World::parse(FIXTURE);
    let open = world.open(OpenScope::EveryFile, None);
    let view = IndexView::new(world.skeleton(), &open);

    let found = |query: &str| {
        view.search_symbols(query, 50)
            .iter()
            .map(|location| location.fqn.as_str().to_owned())
            .collect::<Vec<_>>()
    };

    assert_eq!(found("class"), Vec::<String>::new());
    assert_eq!(found("anonymous"), Vec::<String>::new());
    assert_eq!(found("Fact"), vec!["App\\Factory"]);
}

/// Two files whose `class` keyword lands on the same byte, which a generator makes the normal case.
///
/// Laravel writes every migration from one stub, so the header is identical in all of them and the
/// keyword is on byte 146 of each: a name synthesised from the offset alone gave every migration in
/// an application one identity, a catalog ranked the claimants, and the loser's members were
/// invisible -- `$this->` inside one answered about another's. The two prologues here are
/// byte-identical on purpose; only the bodies differ.
#[test]
fn two_files_whose_anonymous_classes_sit_at_the_same_offset_stay_apart() {
    check_view(
        r#"
//- /src/One.php
<?php

namespace App;

return new class {
    public function alpha(): string
    {
        return $this->al@one@pha();
    }
};

//- /src/Two.php
<?php

namespace App;

return new class {
    public function bravo(): string
    {
        return $this->br@two@avo();
    }
};
"#,
        &[
            Query::goto("$this->alpha() in One.php", "one"),
            Query::goto("$this->bravo() in Two.php", "two"),
        ],
        expect![[r#"
            $this->alpha() in One.php
            - method class@anonymous#34@25e4e984::alpha @ src/One.php 6:21-6:26
            $this->bravo() in Two.php
            - method class@anonymous#34@f2221654::bravo @ src/Two.php 6:21-6:26
        "#]],
    );
}
