//! `find(idd: 5)` and `find(1, id: 2)`: labels PHP stops the call on.
//!
//! Measured on 8.5.10: `Error: Unknown named parameter $idd` and `Error: Named parameter $id
//! overwrites previous argument`, in those words. Before this the only report was the consequence
//! -- `find() needs 1 argument, 1 given -- nothing for $id` -- which names the parameter that is
//! fine and not the label that is wrong.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_label_naming_no_parameter_or_one_already_filled_is_reported() {
    check(
        r#"
//- /src/A.php
<?php
namespace App;

/**
 * @method void magic(int $one)
 */
class Repo
{
    public function find(int $id, bool $fresh = false): ?string
    {
        return null;
    }

    public function any(string ...$rest): void {}

    public function __call(string $name, array $arguments): mixed
    {
        return null;
    }
}

function load(string $key): string
{
    return $key;
}

function go(Repo $repo): void
{
    $repo->find(idd: 5);
    $repo->find(1, id: 2);
    $repo->find(id: 1, fresh: true, fresh: false);
    $repo->find(1, fresh: true);
    $repo->any(whatever: 'x');
    $repo->magic(two: 2);
    load(kye: 'a');
    load(...['key' => 'a'], key: 'b');
}
"#,
        expect![[r#"
            29:17-29:20 error [unknown-named-parameter] Unknown named parameter $idd; $id is one edit away
            30:20-30:22 error [named-parameter-twice] Named parameter $id overwrites previous argument
            31:37-31:42 error [named-parameter-twice] Named parameter $fresh overwrites previous argument
            34:5-34:25 error [missing-argument] magic() needs 1 argument, 1 given -- nothing for $one
            35:10-35:13 error [unknown-named-parameter] Unknown named parameter $kye; $key is one edit away
        "#]],
    );
}

/// The fix: the label changed to the parameter it is one edit from.
#[test]
fn a_misspelt_label_is_corrected_to_the_parameter_it_meant() {
    let spec = phrost_testkit::FixtureSpec::parse(
        r#"
//- /src/A.php
<?php
namespace App;

function load(string $key, int $limit = 0): string
{
    return $key;
}

function go(): void
{
    load(k@at@ye: 'a');
}
"#,
    );
    let world = phrost_harness::World::build(&spec);
    let position = spec.markers().position("at");
    let file = world.file_id(&position.path);
    let rendered = phrost_harness::with_open_scopes(&world, Some(file), |open| {
        let view = phrost_view::IndexView::in_file(world.skeleton(), open, file);
        let request = crate::ActionRequest {
            view: &view,
            at: phrost_model::TextRange::empty(position.offset),
            only: &[],
            asked: Default::default(),
        };
        crate::code_actions(&request)
            .into_iter()
            .filter(|action| action.fixes.is_some())
            .map(|action| {
                let edits = action
                    .edits
                    .iter()
                    .map(|edit| {
                        format!("{} -> {}", world.render_range(file, edit.range), edit.text)
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}: {edits}\n", action.title)
            })
            .collect::<String>()
    });
    spec.markers().assert_all_used();
    expect![[r#"
        Change $kye to $key: 11:10-11:13 -> key
    "#]]
    .assert_eq(&rendered);
}
