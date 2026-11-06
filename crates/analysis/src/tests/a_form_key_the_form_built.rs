//! `$form_state->getValue('email')`: a key the same class's builder wrote into the array it builds.
//!
//! Drupal reads a submitted value by the element's own key, at whatever depth `buildForm()` wrote it
//! -- `$form['email']`, or `'email' => [...]` inside another element -- so the builder's body is the
//! only statement of which keys there are, and where each one is declared.
//!
//! Fixture names stand in for Drupal's: the rule is stated, not detected.

use expect_test::expect;

use crate::tests::{Query, check_with};

const FIXTURE: &str = r#"
//- /src/Form.php
<?php
namespace App;

interface State
{
    public function getValue(string $key): mixed;
}

class Other
{
    public function getValue(string $key): mixed
    {
        return null;
    }
}

class Form
{
    public function buildForm(array $form, State $state): array
    {
        $form['name'] = ['#type' => 'textfield'];
        $form['account'] = [
            '#type' => 'details',
            'email' => ['#type' => 'email'],
        ];
        $form['actions']['submit'] = ['#type' => 'submit'];
        $form['name']['#required'] = true;
        $form['kind']['#type'] = 'select';
        $form['kind']['#options'] = ['plain' => 'Plain'];
        $form['rows'] = [
            '#type' => 'container',
            'first' => ['#type' => 'textfield', '#parents' => ['filed', 0]],
        ];
        return $this->more($form);
    }

    private function more(array $form): array
    {
        $elements['district'] = $this->district();
        $elements['zip'] = ['#type' => 'textfield'];
        $settings['plain'] = 1;
        return $form + $elements + ['history' => ['#markup' => '']];
    }

    private function district(): array
    {
        return ['#type' => 'select'];
    }

    public function submitForm(array &$form, State $state, Other $other): void
    {
        $state->getValue('em@nested@ail');
        $state->getValue('na@top@me');
        $state->getValue('mis@missing@sing');
        $other->getValue('na@other@me');
        $state->getValue('dis@helper@trict');
        $state->getValue('fil@filed@ed');
    }
}
"#;

fn rule(world: &mut phrost_harness::World) {
    world.set_built_key("App\\State", "getValue");
}

/// Each key goes to where the class first wrote it -- nested or not, in whichever method -- and a key
/// it never wrote, and the same call on a class the rule does not name, go nowhere.
#[test]
fn the_key_reaches_the_element_the_class_wrote() {
    check_with(
        FIXTURE,
        &[
            Query::goto("a nested element", "nested"),
            Query::goto("a top-level element, written to twice", "top"),
            Query::goto("a key the class never wrote", "missing"),
            Query::goto("the same call on another class", "other"),
            Query::goto("a key a helper method writes, of a call's result", "helper"),
            Query::goto("a key `#parents` files a value under", "filed"),
            Query::hover("hover", "nested"),
        ],
        rule,
        expect![[r#"
            a nested element
            - `email` @ src/Form.php 24:14
            a top-level element, written to twice
            - `name` @ src/Form.php 21:16
            a key the class never wrote
            - (nothing to go to)
            the same call on another class
            - (nothing to go to)
            a key a helper method writes, of a call's result
            - `district` @ src/Form.php 39:20
            a key `#parents` files a value under
            - `filed` @ src/Form.php 32:65
            hover
            - code: 'email'
            - note: email
            - note: declared at Form.php:24
        "#]],
    );
}

/// What the string can name: every element key, in the order written -- not the properties, not
/// the options one of them lists, not the keys of an array that is not a render array, and not the
/// key of an element whose `#parents` files its value under another.
#[test]
fn completion_offers_the_keys_the_class_wrote() {
    check_with(
        &FIXTURE
            .replace("@nested@", "")
            .replace("@top@", "")
            .replace("@other@", "")
            .replace("@helper@", "")
            .replace("@filed@", "")
            .replace("'mis@missing@sing'", "'@typed@'"),
        &[Query::completion("in an empty string", "typed")],
        rule,
        expect![[r#"
            in an empty string
            - name [catalog key]  textfield
            - account [catalog key]  details
            - email [catalog key]  email
            - actions [catalog key]
            - submit [catalog key]  submit
            - kind [catalog key]  select
            - rows [catalog key]  container
            - filed [catalog key]
            - district [catalog key]
            - zip [catalog key]  textfield
            - history [catalog key]
            - accepting the first leaves `$state->getValue('name');`
        "#]],
    );
}

/// From a read and from the element alike: every write of the key and every read of it in the class,
/// and nothing from the other class's same-named call. A rename is refused rather than half done.
#[test]
fn references_list_the_element_and_its_reads() {
    use std::fmt::Write as _;

    let fixture = FIXTURE
        .replace("@nested@", "")
        .replace("@other@", "")
        .replace("@helper@", "")
        .replace("@filed@", "")
        .replace("@missing@", "")
        .replace("$form['name'] = [", "$form['na@element@me'] = [");
    let spec = phrost_testkit::FixtureSpec::parse(&fixture);
    let world = phrost_harness::World::build_with(&spec, rule);
    let mut out = String::new();
    for marker in ["top", "element"] {
        let position = spec.markers().position(marker);
        let file = world.file_id(&position.path);
        let open = world.open(phrost_harness::OpenScope::EveryFile, Some(file));
        let view = phrost_view::IndexView::in_file(world.skeleton(), &open, file);
        let target = crate::reference_target(&view, position.offset).expect("a key to search for");
        let _ = writeln!(out, "from {marker}:");
        for range in crate::references_in(&view, &target) {
            let _ = writeln!(out, "  {}", world.render_range(file, range));
        }
        let _ = writeln!(
            out,
            "  rename: {}",
            crate::rename_refusal(&view, &target, None).unwrap_or_default()
        );
    }
    spec.markers().assert_all_used();
    expect![[r#"
        from top:
          21:16-21:20
          27:16-27:20
          53:27-53:31
          rename: `name` is also named where this cannot see -- `#states` selectors, form alters, tests that submit the form -- so a rename here would leave those reading the old key
        from element:
          21:16-21:20
          27:16-27:20
          53:27-53:31
          rename: `name` is also named where this cannot see -- `#states` selectors, form alters, tests that submit the form -- so a rename here would leave those reading the old key
    "#]]
    .assert_eq(&out);
}
