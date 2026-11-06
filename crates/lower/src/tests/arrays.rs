//! Array literals, and what their elements are.
//!
//! The elements used to arrive as one flat list from the generic unmodelled walk, which yields every
//! expression written inside a node and says nothing about where each sat. So `['a' => new Pen()]`
//! and `[new Pen(), 'a']` lowered to the same two ids, and the one thing a reader wants from a
//! literal -- which half of a pair is the value -- was not in the tree to read.
//!
//! The node count is the other half of the story, because array literals are where the body IR of a
//! framework project mostly is. A literal key carries its text on the element instead of a node of
//! its own, so the commonest element shape ends up costing exactly what it did before.
//!
//! These snapshots are of the whole body, on purpose: what a shape change to a node inside a
//! generic walk risks is not a wrong line but a duplicated or a missing subtree, and only the whole
//! output shows either.

use expect_test::expect;

use super::check_bodies;

#[test]
fn a_bare_element_is_its_own_expression() {
    // No wrapper node: a positional element says nothing its value does not, and one node per
    // element in every list literal in a project is memory spent for no fact gained.
    check_bodies(
        r#"
//- /src/Case.php
<?php
class Case_
{
    public function run(): void
    {
        $tools = [new Pen(), new Pencil()];
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-9:1
                ?? declaration (arrival) @ 2:1-8:2
            Case_::run
              body @ 5:5-7:6
                assign @ 6:9-6:43
                  $tools @ 6:9-6:15
                  =
                    array @ 6:18-6:43
                      new Pen @ 6:19-6:28
                      new Pencil @ 6:30-6:42
        "#]],
    );
}

#[test]
fn a_literal_key_travels_on_the_element_and_costs_no_node() {
    // The point of the shape, and the reason it is free. Which half of the pair is the value is now
    // in the tree instead of being guessed from the order they were collected in -- and the key that
    // says so is text on the element rather than a node, so the element takes the node the key used
    // to and a project full of render arrays pays nothing.
    check_bodies(
        r#"
//- /src/Case.php
<?php
class Case_
{
    public function run(): void
    {
        $tools = ['pen' => new Pen(), 3 => new Pencil()];
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-9:1
                ?? declaration (arrival) @ 2:1-8:2
            Case_::run
              body @ 5:5-7:6
                assign @ 6:9-6:57
                  $tools @ 6:9-6:15
                  =
                    array @ 6:18-6:57
                      element pen @ 6:19-6:37 (key @ 6:19-6:24)
                        new Pen @ 6:28-6:37
                      element 3 @ 6:39-6:56 (key @ 6:39-6:40)
                        new Pencil @ 6:44-6:56
        "#]],
    );
}

#[test]
fn a_spread_element_is_marked_as_one() {
    // Apart from a keyed element because what it holds is one level further in: reading the value as
    // an element would say this array holds arrays.
    check_bodies(
        r#"
//- /src/Case.php
<?php
class Case_
{
    public function run(): void
    {
        $tools = [...$other, new Pen()];
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-9:1
                ?? declaration (arrival) @ 2:1-8:2
            Case_::run
              body @ 5:5-7:6
                assign @ 6:9-6:40
                  $tools @ 6:9-6:15
                  =
                    array @ 6:18-6:40
                      element ... @ 6:19-6:28
                        $other @ 6:22-6:28
                      new Pen @ 6:30-6:39
        "#]],
    );
}

#[test]
fn a_computed_key_keeps_its_expression() {
    // The one key that does need a node: it holds calls as readily as a value does, there is no text
    // to put on the element instead, and a variable nothing can reach is one a rename would not
    // change.
    check_bodies(
        r#"
//- /src/Case.php
<?php
class Case_
{
    public function run(): void
    {
        $tools = [$this->slug() => new Pen()];
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-9:1
                ?? declaration (arrival) @ 2:1-8:2
            Case_::run
              body @ 5:5-7:6
                assign @ 6:9-6:46
                  $tools @ 6:9-6:15
                  =
                    array @ 6:18-6:46
                      element @ 6:19-6:45
                        key
                          call ->slug() @ 6:19-6:32
                            receiver
                              $this @ 6:19-6:24
                        value
                          new Pen @ 6:36-6:45
        "#]],
    );
}

#[test]
fn a_missing_element_lowers_to_nothing() {
    // `[1, , 3]` is legal, and the gap is a comma with no expression behind it. Dropped rather than
    // given a node: there is nothing to point a cursor at and nothing to type.
    check_bodies(
        r#"
//- /src/Case.php
<?php
class Case_
{
    public function run(): void
    {
        $rows = [1, , 3];
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-9:1
                ?? declaration (arrival) @ 2:1-8:2
            Case_::run
              body @ 5:5-7:6
                assign @ 6:9-6:25
                  $rows @ 6:9-6:14
                  =
                    array @ 6:17-6:25
                      int @ 6:18-6:19
                      int @ 6:23-6:24
        "#]],
    );
}

#[test]
fn the_legacy_spelling_lowers_the_same() {
    // `array(...)` is the same node with different punctuation, and generated code still writes it.
    check_bodies(
        r#"
//- /src/Case.php
<?php
class Case_
{
    public function run(): void
    {
        $tools = array('pen' => new Pen());
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-9:1
                ?? declaration (arrival) @ 2:1-8:2
            Case_::run
              body @ 5:5-7:6
                assign @ 6:9-6:43
                  $tools @ 6:9-6:15
                  =
                    array @ 6:18-6:43
                      element pen @ 6:24-6:42 (key @ 6:24-6:29)
                        new Pen @ 6:33-6:42
        "#]],
    );
}

#[test]
fn a_nested_literal_keeps_its_own_elements() {
    // Each level lowers its own, so a member access three levels down is still one node with one
    // parent -- which is the property a double-lowering bug breaks without failing anything.
    check_bodies(
        r#"
//- /src/Case.php
<?php
class Case_
{
    public function run(): void
    {
        $form = ['#rows' => [new Pen()]];
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-9:1
                ?? declaration (arrival) @ 2:1-8:2
            Case_::run
              body @ 5:5-7:6
                assign @ 6:9-6:41
                  $form @ 6:9-6:14
                  =
                    array @ 6:17-6:41
                      element #rows @ 6:18-6:40 (key @ 6:18-6:25)
                        array @ 6:29-6:40
                          new Pen @ 6:30-6:39
        "#]],
    );
}

#[test]
fn a_destructuring_target_is_still_an_array_literal() {
    // `['a' => $x] = $row` is a literal in a write position, and the variables inside it are bound
    // by it. The shape is the same one, so the layer that reads bindings reads one node kind.
    check_bodies(
        r#"
//- /src/Case.php
<?php
class Case_
{
    public function run(array $row): void
    {
        ['pen' => $pen, $rest] = $row;
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-9:1
                ?? declaration (arrival) @ 2:1-8:2
            Case_::run
              body @ 5:5-7:6
                assign @ 6:9-6:38
                  array @ 6:9-6:31
                    element pen @ 6:10-6:23 (key @ 6:10-6:15)
                      $pen @ 6:19-6:23
                    element 1 @ 6:25-6:30 (key @ 6:25-6:30)
                      $rest @ 6:25-6:30
                  =
                    $row @ 6:34-6:38
        "#]],
    );
}

#[test]
fn a_literal_in_a_declaration_stays_out_of_every_method_body() {
    // The gotcha this shape change is closest to. A class declaration is a *statement*, so the
    // generic walk that lowers what surrounds a body once descended into every method body and
    // lowered it a second time -- and nothing failed, because identical duplicates are invisible to
    // a position lookup and only show up as memory.
    //
    // So: a literal written in a property default belongs to the file's own body, the one written in
    // the method belongs to the method, and neither appears twice or in the other.
    check_bodies(
        r#"
//- /src/Case.php
<?php
class Case_
{
    private array $defaults = ['pen' => 1];

    public function run(): void
    {
        $tools = ['pen' => new Pen()];
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-11:1
                ?? declaration (arrival) @ 2:1-10:2
                  array @ 4:31-4:43
                    element pen @ 4:32-4:42 (key @ 4:32-4:37)
                      int @ 4:41-4:42
            Case_::run
              body @ 7:5-9:6
                assign @ 8:9-8:38
                  $tools @ 8:9-8:15
                  =
                    array @ 8:18-8:38
                      element pen @ 8:19-8:37 (key @ 8:19-8:24)
                        new Pen @ 8:28-8:37
        "#]],
    );
}
