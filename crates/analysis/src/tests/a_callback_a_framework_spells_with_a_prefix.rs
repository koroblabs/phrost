//! `'callback' => '::districtCallback'` -- Drupal's way of naming a method of the form being built.
//!
//! The form builder resolves the leading `::` against `$form_state->getFormObject()`, which is the
//! class the array was written in: the same target `[$this, 'districtCallback']` already reaches.
//! Two spellings of one callable and only one was taught, and this is the commoner of the two in
//! real code -- `'#submit' => ['::save']` and `'#validate' => ['::check']` are written the same way.
//!
//! The prefix comes from the framework rather than from here. Nothing in PHP itself makes a string
//! name a method, and a rule that assumed one would claim ordinary strings in projects that use no
//! framework at all -- so `own_method_prefixes` is where Drupal says it and nowhere else.

use expect_test::expect;

use crate::tests::{Query, check_with};

#[test]
fn a_string_with_the_frameworks_prefix_reaches_the_method() {
    check_with(
        r#"
//- /src/Form.php
<?php
namespace App;

class Form
{
    public function build(): array
    {
        return [
            'district' => [
                '#ajax' => ['callback' => '::district@here@Callback'],
                '#submit' => ['::save'],
            ],
        ];
    }

    public function districtCallback(): string
    {
        return 'district';
    }

    public function save(): void {}
}
"#,
        &[Query::goto("the method the string names", "here")],
        |world| world.set_own_method_prefixes(&["::"]),
        expect![[r#"
            the method the string names
            - method App\Form::districtCallback @ src/Form.php 16:21-16:37 (full 16:5-19:6)
        "#]],
    );
}

/// A string carrying the prefix and naming nothing is a string: answering for it would send goto
/// somewhere invented.
#[test]
fn a_prefix_naming_no_method_answers_nothing() {
    check_with(
        r#"
//- /src/Form.php
<?php
namespace App;

class Form
{
    public function build(): array
    {
        return ['#submit' => ['::no@here@SuchMethod']];
    }
}
"#,
        &[Query::goto("nothing to reach", "here")],
        |world| world.set_own_method_prefixes(&["::"]),
        expect![[r#"
            nothing to reach
            - (nothing to go to)
        "#]],
    );
}

/// A rename edits the string with the declaration, or the form calls a method that no longer
/// exists. Measured on the reference project: renaming `saveNote` edited the declaration and left
/// `'#submit' => ['::saveNote']` three lines up.
///
/// The string names a method of the class it is written in, so a subclass inheriting the method is
/// reached too -- and an unrelated class's `'::save'` is its own `save` and stays.
#[test]
fn a_rename_edits_the_string_that_names_the_method() {
    use std::fmt::Write as _;

    let spec = phrost_testkit::FixtureSpec::parse(
        r#"
//- /src/Form.php
<?php
namespace App;

class Form
{
    public function build(): array
    {
        return ['#submit' => ['::save']];
    }

    public function sa@here@ve(): void {}
}

class Child extends Form
{
    public function rebuild(): array
    {
        return ['#submit' => ['::save']];
    }
}

class Other
{
    public function build(): array
    {
        return ['#submit' => ['::save']];
    }

    public function save(): void {}
}
"#,
    );
    let world = phrost_harness::World::build_with(&spec, |world| {
        world.set_own_method_prefixes(&["::"]);
    });
    let position = spec.markers().position("here");
    let file = world.file_id(&position.path);
    let open = world.open(phrost_harness::OpenScope::EveryFile, Some(file));
    let view = phrost_view::IndexView::in_file(world.skeleton(), &open, file);
    let target = crate::rename_target(&view, position.offset).expect("a method to rename");
    let mut out = String::new();
    for range in crate::references_in(&view, &target) {
        let _ = writeln!(out, "{}", world.render_range(file, range));
    }
    expect![[r#"
        8:34-8:38
        11:21-11:25
        18:34-18:38
    "#]]
    .assert_eq(&out);
}
