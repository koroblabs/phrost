//! Where the *type* of the thing under the cursor is declared.
//!
//! Its own file because it is its own question, and the interesting part is where the two answers
//! differ: on a variable, goto definition lands on the assignment a line above and this lands on the
//! class. A test that only covered the cases where they agree would pass with the feature wired to
//! the wrong function.

use expect_test::{Expect, expect};
use phrost_harness::{World, with_open_scopes};
use phrost_testkit::FixtureSpec;
use phrost_view::IndexView;

use crate::{goto_definition, goto_type_definition};

/// Both answers at the marker, so the snapshot shows the difference rather than one of them.
fn check(fixture: &str, marker: &str, expect: Expect) {
    let spec = FixtureSpec::parse(fixture);
    let world = World::build(&spec);
    let position = spec.markers().position(marker);
    let file = world.file_id(&position.path);

    let rendered = with_open_scopes(&world, Some(file), |open| {
        let view = IndexView::in_file(world.skeleton(), open, file);
        let render = |label: &str, targets: Vec<crate::NavigationTarget>| {
            if targets.is_empty() {
                return format!("{label}: (nothing)\n");
            }
            targets
                .into_iter()
                .map(|target| {
                    format!(
                        "{label}: {} {} @ {} {}\n",
                        target.kind.as_str(),
                        target.name,
                        world.path_of(target.file),
                        world.render_range(target.file, target.selection_range),
                    )
                })
                .collect()
        };
        let mut out = render("definition", goto_definition(&view, position.offset));
        out.push_str(&render(
            "type",
            goto_type_definition(&view, position.offset),
        ));
        out
    });

    spec.markers().assert_all_used();
    expect.assert_eq(&rendered);
}

#[test]
fn on_a_variable_the_two_answers_differ() {
    check(
        r#"
//- /src/Thing.php
<?php
namespace App;

class Thing {}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(Thing $given): void
    {
        $held = $given;
        $he@here@ld;
    }
}
"#,
        "here",
        expect![[r#"
            definition: variable held @ src/Caller.php 8:9-8:14
            type: class App\Thing @ src/Thing.php 4:7-4:12
        "#]],
    );
}

#[test]
fn on_a_method_call_the_type_is_what_it_returns() {
    check(
        r#"
//- /src/Thing.php
<?php
namespace App;

class Thing {}

//- /src/Caller.php
<?php
namespace App;

class Repo
{
    public function find(): Thing
    {
        return new Thing();
    }
}

class Caller
{
    public function run(Repo $repo): void
    {
        $repo->fi@here@nd();
    }
}
"#,
        "here",
        expect![[r#"
            definition: method find @ src/Caller.php 6:21-6:25
            type: class App\Thing @ src/Thing.php 4:7-4:12
        "#]],
    );
}

#[test]
fn on_a_class_name_the_two_answers_agree() {
    // Not a redundancy: a name written where a class goes *is* its own type, and answering nothing
    // here would make the key look broken in the commonest place a person presses it.
    check(
        r#"
//- /src/Thing.php
<?php
namespace App;

class Thing {}

//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run(): void
    {
        $held = new Th@here@ing();
    }
}
"#,
        "here",
        expect![[r#"
            definition: class App\Thing @ src/Thing.php 4:7-4:12
            type: class App\Thing @ src/Thing.php 4:7-4:12
        "#]],
    );
}

#[test]
fn a_variable_with_no_declared_type_has_no_type_to_go_to() {
    check(
        r#"
//- /src/Caller.php
<?php
namespace App;

class Caller
{
    public function run($whatever): void
    {
        $what@here@ever;
    }
}
"#,
        "here",
        expect![[r#"
            definition: parameter whatever @ src/Caller.php 6:25-6:34
            type: (nothing)
        "#]],
    );
}

/// A list of a class, in the three places a type is written.
///
/// There is no `Rule[]` to jump to, so a type-definition that answers nothing is answering a
/// question nobody asked. `Rule` is the only declaration a reader on `@var Rule[]` could want, and
/// it is what every other server lands on. Found by `capability-audit`, which scored
/// `typeDefinition` 19/20 on one real project and named the site.
const RULES: &str = r#"
//- /src/Rule.php
<?php
namespace App;

interface Rule {}

//- /src/Collection.php
<?php
namespace App;

/**
 * @template T
 */
class Collection {}
"#;

#[test]
fn a_list_of_a_class_answers_with_the_class() {
    check(
        &format!(
            r#"{RULES}
//- /src/Holder.php
<?php
namespace App;

class Holder
{{
    /**
     * @var \App\Rule[]
     */
    private array $ru@here@les;
}}
"#
        ),
        "here",
        expect![[r#"
            definition: property rules @ src/Holder.php 9:19-9:25
            type: interface App\Rule @ src/Rule.php 4:11-4:15
        "#]],
    );
}

#[test]
fn a_parameter_declared_as_a_list_answers_the_same_way() {
    check(
        &format!(
            r#"{RULES}
//- /src/Holder.php
<?php
namespace App;

class Holder
{{
    /**
     * @param \App\Rule[] $given
     */
    public function run(array $gi@here@ven): void
    {{
    }}
}}
"#
        ),
        "here",
        expect![[r#"
            definition: parameter given @ src/Holder.php 9:31-9:37
            type: interface App\Rule @ src/Rule.php 4:11-4:15
        "#]],
    );
}

#[test]
fn a_local_holding_what_a_list_returning_call_gave_it_answers_the_same_way() {
    check(
        &format!(
            r#"{RULES}
//- /src/Holder.php
<?php
namespace App;

class Holder
{{
    /**
     * @return \App\Rule[]
     */
    public function all(): array
    {{
        return [];
    }}

    public function run(): void
    {{
        $he@here@ld = $this->all();
    }}
}}
"#
        ),
        "here",
        expect![[r#"
            definition: variable held @ src/Holder.php 16:9-16:14
            type: interface App\Rule @ src/Rule.php 4:11-4:15
        "#]],
    );
}

#[test]
fn a_bare_array_still_answers_nothing() {
    // The position kept: a member declared `array` says nothing about what it holds, so there is no
    // type to go to. The element rule reads a declaration, not a guess.
    check(
        &format!(
            r#"{RULES}
//- /src/Holder.php
<?php
namespace App;

class Holder
{{
    private array $ru@here@les;
}}
"#
        ),
        "here",
        expect![[r#"
            definition: property rules @ src/Holder.php 6:19-6:25
            type: (nothing)
        "#]],
    );
}

#[test]
fn a_class_that_holds_elements_is_still_its_own_answer() {
    // The reason the element is read only where the type names no class: `Collection<Rule>` *is* a
    // class, and a reader on it is asking where `Collection` is declared. Answering `Rule` would be
    // a jump to the thing it contains.
    check(
        &format!(
            r#"{RULES}
//- /src/Holder.php
<?php
namespace App;

class Holder
{{
    /**
     * @var \App\Collection<\App\Rule>
     */
    private Collection $ru@here@les;
}}
"#
        ),
        "here",
        expect![[r#"
            definition: property rules @ src/Holder.php 9:24-9:30
            type: class App\Collection @ src/Collection.php 7:7-7:17
        "#]],
    );
}

#[test]
fn a_union_of_classes_is_declared_in_each_of_them() {
    // `if (!$app->isOrder() && !$app->isLine()) { return; }` over methods that return
    // `$this instanceof Order` leaves a `Order|Line`, and the card says so. Their common ancestor is
    // where the members are looked up, not where this type is written -- the reference project's
    // `ReceiptsSentAction` is the shape, and `a-type-definition-lands-on-what-hover-names` found
    // it.
    check(
        r#"
//- /src/App.php
<?php
namespace App;

abstract class Application {}
class Order extends Application {}
class Line extends Application {}

class Action
{
    public function run(Order|Line $app): void
    {
        $a@here@pp;
    }
}
"#,
        "here",
        expect![[r#"
            definition: parameter app @ src/App.php 10:36-10:40
            type: class App\Order @ src/App.php 5:7-5:12
            type: class App\Line @ src/App.php 6:7-6:11
        "#]],
    );
}
