//! Statement bodies, as lowered.
//!
//! Every node carries its range, and the ranges are in the snapshot on purpose: a body is read to
//! answer "what is at this offset", so a tree with the right shape and wrong positions is worse
//! than no tree at all — it answers confidently about the wrong thing.

use expect_test::expect;

use super::check_bodies;

#[test]
fn an_assignment_from_a_constructor() {
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run(): void
    {
        $repo = new Repo();
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-9:1
                ?? declaration (arrival) @ 2:1-8:2
            Repo::run
              body @ 5:5-7:6
                assign @ 6:9-6:27
                  $repo @ 6:9-6:14
                  =
                    new Repo @ 6:17-6:27
        "#]],
    );
}

#[test]
fn a_chain_of_calls_and_property_accesses() {
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run(): void
    {
        $this->repo->find(1)->name;
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-9:1
                ?? declaration (arrival) @ 2:1-8:2
            Repo::run
              body @ 5:5-7:6
                property ->name @ 6:9-6:35
                  receiver
                    call ->find() @ 6:9-6:29
                      receiver
                        property ->repo @ 6:9-6:20
                          receiver
                            $this @ 6:9-6:14
                      args
                        int @ 6:27-6:28
        "#]],
    );
}

#[test]
fn a_nullsafe_call_is_marked_as_one() {
    // The distinction is not cosmetic: `?->` on null is null rather than an error, so a diagnostic
    // has to tell the two apart. It is also where a strip-order bug once killed every nullsafe
    // receiver.
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run(): void
    {
        $this->maybe?->find();
        $this->always->find();
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-10:1
                ?? declaration (arrival) @ 2:1-9:2
            Repo::run
              body @ 5:5-8:6
                call ?->find() @ 6:9-6:30
                  receiver
                    property ->maybe @ 6:9-6:21
                      receiver
                        $this @ 6:9-6:14
                call ->find() @ 7:9-7:30
                  receiver
                    property ->always @ 7:9-7:22
                      receiver
                        $this @ 7:9-7:14
        "#]],
    );
}

#[test]
fn static_access_records_how_the_class_was_written() {
    check_bodies(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(): void
    {
        Repo::make();
        self::$count;
        static::CONST_ONE;
        parent::make();
        $class::make();
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-15:1
                block @ 2:1-14:2
                  ?? declaration (arrival) @ 4:1-14:2
            Repo::run
              body @ 7:5-13:6
                call Repo::make() @ 8:9-8:21
                static property self::$count @ 9:9-9:21
                class const static::CONST_ONE @ 10:9-10:26
                call parent::make() @ 11:9-11:23
                call <value>::make() @ 12:9-12:23
                  class
                    $class @ 12:9-12:15
        "#]],
    );
}

#[test]
fn a_function_call_keeps_the_written_name_and_an_indirect_one_does_not() {
    check_bodies(
        r#"
//- /src/helpers.php
<?php
namespace App;

function run(): void
{
    strlen('a');
    \App\make();
    $callable();
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-10:1
                block @ 2:1-9:2
                  ?? declaration (arrival) @ 4:1-9:2
            function run
              body @ 5:1-9:2
                call strlen() @ 6:5-6:16
                  args
                    string @ 6:12-6:15
                call App\make() @ 7:5-7:16
                call <value>() @ 8:5-8:16
                  callee
                    $callable @ 8:5-8:14
        "#]],
    );
}

#[test]
fn an_elseif_chain_becomes_nested_ifs() {
    // One shape to narrow against. A chain would mean every consumer walking clauses to work out
    // which conditions held on the way in.
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run($a, $b): void
    {
        if ($a) {
            return;
        } elseif ($b) {
            $a;
        } else {
            $b;
        }
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-15:1
                ?? declaration (arrival) @ 2:1-14:2
            Repo::run
              body @ 5:5-13:6
                if @ 6:9-12:10
                  $a @ 6:13-6:15
                  then
                    return @ 7:13-7:20
                  else
                    if @ 8:11-12:10
                      $b @ 8:19-8:21
                      then
                        $a @ 9:13-9:15
                      else
                        $b @ 11:13-11:15
        "#]],
    );
}

#[test]
fn a_catch_clause_records_the_type_that_gives_its_variable_one() {
    check_bodies(
        r#"
//- /src/Repo.php
<?php
namespace App;

class Repo
{
    public function run(): void
    {
        try {
            $this->go();
        } catch (\RuntimeException | LogicException $error) {
            $error;
        } catch (\Throwable) {
        } finally {
            $this->done();
        }
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-18:1
                block @ 2:1-17:2
                  ?? declaration (arrival) @ 4:1-17:2
            Repo::run
              body @ 7:5-16:6
                try @ 8:9-15:10
                  call ->go() @ 9:13-9:24
                    receiver
                      $this @ 9:13-9:18
                  catch \RuntimeException|LogicException $error @ 10:11-12:10
                    $error @ 11:13-11:19
                  catch \Throwable @ 12:11-13:10
                  finally
                    call ->done() @ 14:13-14:26
                      receiver
                        $this @ 14:13-14:18
        "#]],
    );
}

#[test]
fn a_foreach_records_its_key_and_value_targets() {
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run($items): void
    {
        foreach ($items as $name => $item) {
            $item;
        }
        foreach ($items as $only) {
        }
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-13:1
                ?? declaration (arrival) @ 2:1-12:2
            Repo::run
              body @ 5:5-11:6
                foreach @ 6:9-8:10
                  $items @ 6:18-6:24
                  key
                    $name @ 6:28-6:33
                  value
                    $item @ 6:37-6:42
                  $item @ 7:13-7:18
                foreach @ 9:9-10:10
                  $items @ 9:18-9:24
                  value
                    $only @ 9:28-9:33
        "#]],
    );
}

#[test]
fn a_closure_and_an_arrow_function_have_the_same_shape() {
    // The arrow function's expression becomes a body holding one `return`, so a caller asking what
    // a callable produces has one path rather than two.
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run($items): void
    {
        $closure = function (Repo $item) use ($items) {
            return $item;
        };
        $arrow = fn (Repo $item) => $item->name;
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-12:1
                ?? declaration (arrival) @ 2:1-11:2
            Repo::run
              body @ 5:5-10:6
                assign @ 6:9-8:10
                  $closure @ 6:9-6:17
                  =
                    closure(Repo $item) @ 6:20-8:10
                      use $items @ 6:47-6:53
                      body @ 6:55-8:10
                        return @ 7:13-7:26
                          $item @ 7:20-7:25
                assign @ 9:9-9:48
                  $arrow @ 9:9-9:15
                  =
                    fn(Repo $item) @ 9:18-9:48
                      body @ 9:37-9:48
                        return @ 9:37-9:48
                          property ->name @ 9:37-9:48
                            receiver
                              $item @ 9:37-9:42
        "#]],
    );
}

#[test]
fn an_operator_is_modelled_by_what_it_produces() {
    // `instanceof` is what narrowing reads, and it keeps its own shape because the class it names
    // is a class reference like any other. Everything else is kept for the one thing a reader wants
    // from it -- what it produces -- with its operands still lowered underneath.
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run($value): void
    {
        if ($value instanceof Repo) {
            $value;
        }
        $value . 'x';
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-12:1
                ?? declaration (arrival) @ 2:1-11:2
            Repo::run
              body @ 5:5-10:6
                if @ 6:9-8:10
                  instanceof Repo @ 6:13-6:35
                    $value @ 6:13-6:19
                  then
                    $value @ 7:13-7:19
                operator -> non-falsy-string @ 9:9-9:21
                  $value @ 9:9-9:15
                  string @ 9:18-9:21
        "#]],
    );
}

#[test]
fn an_unmodelled_statement_keeps_its_range_rather_than_vanishing() {
    // `echo` and `unset` are not modelled. Showing them as a placeholder is what keeps the tree
    // complete and makes the gap something a test can see rather than something a feature
    // discovers -- and the variables written inside one stay reachable, which is the half that
    // matters. An `unset` is told apart from the rest, because a reader of an array's keys needs
    // to know which slots it took out, and that is all that is known about it.
    //
    // `global` used to be the first example here and is modelled now, which is why the fixture
    // says `unset` instead: an example of a gap has to be an actual gap.
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run($value): void
    {
        unset($value);
        echo $value;
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-10:1
                ?? declaration (arrival) @ 2:1-9:2
            Repo::run
              body @ 5:5-8:6
                ?? unset (onward) @ 6:9-6:23
                  $value @ 6:15-6:21
                ?? statement (onward) @ 7:9-7:21
                  $value @ 7:14-7:20
        "#]],
    );
}

#[test]
fn an_abstract_method_has_no_body_and_that_is_not_the_same_as_unread() {
    check_bodies(
        r#"
//- /src/Repo.php
<?php
abstract class Repo
{
    abstract public function find(): void;

    public function run(): void
    {
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-10:1
                ?? declaration (arrival) @ 2:1-9:2
            Repo::find
              <no body>
            Repo::run
              body @ 7:5-8:6
        "#]],
    );
}

#[test]
fn a_broken_body_still_lowers_what_parsed() {
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run(): void
    {
        $a = new Repo();
        $b = ;
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-10:1
                ?? declaration (arrival) @ 2:1-9:2
            Repo::run
              body @ 5:5-8:6
                assign @ 6:9-6:24
                  $a @ 6:9-6:11
                  =
                    new Repo @ 6:14-6:24
                assign @ 7:9-7:15
                  $b @ 7:9-7:11
                  =
                    ?? expression @ 7:14-7:15
            !! Unexpected token `Semicolon`
            !! Expected one of `Semicolon`, `CloseTag`, found `RightBrace`
        "#]],
    );
}

#[test]
fn an_argument_is_wrapped_only_when_it_says_more_than_its_value() {
    // A plain positional argument *is* its expression. A name or a spread is not, and something has
    // to be able to tell them apart: a parameter-name hint over `id: 1` would repeat what is
    // written, and a spread lines up with no single parameter.
    check_bodies(
        r#"
//- /src/C.php
<?php
function go(): void
{
    send($body, urgent: true, ...$rest);
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-6:1
                ?? declaration (arrival) @ 2:1-5:2
            function go
              body @ 3:1-5:2
                call send() @ 4:5-4:40
                  args
                    $body @ 4:10-4:15
                    arg urgent: @ 4:17-4:29
                      true @ 4:25-4:29
                    arg ... @ 4:31-4:39
                      $rest @ 4:34-4:39
        "#]],
    );
}

#[test]
fn a_match_arm_can_list_several_conditions_and_a_default_has_none() {
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run($value): string
    {
        return match ($value) {
            1, 2 => 'small',
            3 => 'three',
            default => 'other',
        };
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-13:1
                ?? declaration (arrival) @ 2:1-12:2
            Repo::run
              body @ 5:5-11:6
                return @ 6:9-10:11
                  match @ 6:16-10:10
                    $value @ 6:23-6:29
                    arm @ 7:13-7:28
                      values
                        int @ 7:13-7:14
                        int @ 7:16-7:17
                      =>
                        string @ 7:21-7:28
                    arm @ 8:13-8:25
                      values
                        int @ 8:13-8:14
                      =>
                        string @ 8:18-8:25
                    default @ 9:13-9:31
                      =>
                        string @ 9:24-9:31
        "#]],
    );
}

#[test]
fn a_switch_folds_fallthrough_labels_into_one_arm() {
    // `case 1: case 2:` shares one body -- see `SwitchCaseData`. Modelling them as two arms
    // would make "does every arm leave" wrong: the empty-bodied one never leaves on its own.
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run($value): void
    {
        switch ($value) {
            case 1:
            case 2:
                echo 'small';
                break;
            default:
                echo 'other';
        }
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-16:1
                ?? declaration (arrival) @ 2:1-15:2
            Repo::run
              body @ 5:5-14:6
                switch @ 6:9-13:10
                  $value @ 6:17-6:23
                  case @ 7:13-10:23
                    values
                      int @ 7:18-7:19
                      int @ 8:18-8:19
                    ?? statement (onward) @ 9:17-9:30
                      string @ 9:22-9:29
                    break @ 10:17-10:23
                  default @ 11:13-12:30
                    ?? statement (onward) @ 12:17-12:30
                      string @ 12:22-12:29
        "#]],
    );
}

#[test]
fn a_switch_leaves_only_when_every_arm_does_including_the_default() {
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run($value): string
    {
        switch ($value) {
            case 1:
                return 'one';
            case 2:
                return 'two';
            default:
                return 'other';
        }
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-16:1
                ?? declaration (arrival) @ 2:1-15:2
            Repo::run
              body @ 5:5-14:6
                switch @ 6:9-13:10
                  $value @ 6:17-6:23
                  case @ 7:13-8:30
                    values
                      int @ 7:18-7:19
                    return @ 8:17-8:30
                      string @ 8:24-8:29
                  case @ 9:13-10:30
                    values
                      int @ 9:18-9:19
                    return @ 10:17-10:30
                      string @ 10:24-10:29
                  default @ 11:13-12:32
                    return @ 12:17-12:32
                      string @ 12:24-12:31
        "#]],
    );
}

#[test]
fn a_static_or_global_declaration_is_a_binding_of_its_own() {
    // The variable a `static` or `global` names is not an expression, so the generic walk that
    // keeps an unmodelled statement's subtree never reached it and the statement bound nothing at
    // all -- not even at the declaration itself.
    //
    // The initialised form is spelled as an assignment on purpose: everything that already reads
    // one gives `$seeded` the initialiser's type without a single new arm.
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run(): void
    {
        static $bare;
        static $seeded = 1;
        global $config, $other;
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-11:1
                ?? declaration (arrival) @ 2:1-10:2
            Repo::run
              body @ 5:5-9:6
                static @ 6:9-6:22
                  $bare @ 6:16-6:21
                static @ 7:9-7:28
                  assign @ 7:16-7:27
                    $seeded @ 7:16-7:23
                    =
                      int @ 7:26-7:27
                global @ 8:9-8:32
                  $config @ 8:16-8:23
                  $other @ 8:25-8:31
        "#]],
    );
}

/// An attribute is a constructor call written where no expression walk finds one.
///
/// Its name is recorded by the declaration pass, so the body pass reuses that reference rather than
/// adding a second one at the same position — see `Lowerer::attribute_ref_at`.
#[test]
fn an_attribute_is_lowered_as_the_constructor_call_it_is() {
    check_bodies(
        r##"
//- /src/Repo.php
<?php
#[Attribute]
class FooAttr
{
    public function __construct(string $bar, int $baz = 1) {}
}

#[FooAttr("x", baz: 2)]
class Bar {}
"##,
        expect![[r#"
            <file scope>
              body @ 1:1-10:1
                ?? declaration (arrival) @ 2:1-6:2
                  new Attribute @ 2:3-2:12
                  int @ 5:57-5:58
                ?? declaration (arrival) @ 8:1-9:13
                  new FooAttr @ 8:3-8:23
                    args
                      string @ 8:11-8:14
                      arg baz: @ 8:16-8:22
                        int @ 8:21-8:22
            FooAttr::__construct
              body @ 5:60-5:62
        "#]],
    );
}
