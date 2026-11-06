//! Every way PHP writes "is this null", as lowered.
//!
//! Four spellings and two constructs, all reaching one node, because narrowing has one question to
//! ask and reading six shapes to ask it is how one of them ends up forgotten. The negated forms
//! arrive wrapped in a `!`, which is the shape a guard is already read backwards through.

use expect_test::expect;

use super::check_bodies;

#[test]
fn the_four_comparisons_against_null() {
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run(?Repo $repo): void
    {
        $a = $repo === null;
        $b = $repo !== null;
        $c = $repo == null;
        $d = NULL === $repo;
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-12:1
                ?? declaration (arrival) @ 2:1-11:2
            Repo::run
              body @ 5:5-10:6
                assign @ 6:9-6:28
                  $a @ 6:9-6:11
                  =
                    === null @ 6:14-6:28
                      $repo @ 6:14-6:19
                assign @ 7:9-7:28
                  $b @ 7:9-7:11
                  =
                    ! @ 7:14-7:28
                      === null @ 7:14-7:28
                        $repo @ 7:14-7:19
                assign @ 8:9-8:27
                  $c @ 8:9-8:11
                  =
                    == null @ 8:14-8:27
                      $repo @ 8:14-8:19
                assign @ 9:9-9:28
                  $d @ 9:9-9:11
                  =
                    === null @ 9:14-9:28
                      $repo @ 9:23-9:28
        "#]],
    );
}

#[test]
fn isset_and_empty_are_null_tests_too() {
    // `isset($a, $b)` is an `and` of one test per value, built here so narrowing meets the
    // connective it already understands rather than a list it would have to walk itself.
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run(?Repo $one, ?Repo $two): void
    {
        $a = isset($one);
        $b = isset($one, $two);
        $c = empty($one);
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-11:1
                ?? declaration (arrival) @ 2:1-10:2
            Repo::run
              body @ 5:5-9:6
                assign @ 6:9-6:25
                  $a @ 6:9-6:11
                  =
                    ! @ 6:14-6:25
                      isset @ 6:14-6:25
                        $one @ 6:20-6:24
                assign @ 7:9-7:31
                  $b @ 7:9-7:11
                  =
                    && @ 7:14-7:31
                      ! @ 7:14-7:31
                        isset @ 7:14-7:31
                          $one @ 7:20-7:24
                      ! @ 7:14-7:31
                        isset @ 7:14-7:31
                          $two @ 7:26-7:30
                assign @ 8:9-8:25
                  $c @ 8:9-8:11
                  =
                    empty @ 8:14-8:25
                      $one @ 8:20-8:24
        "#]],
    );
}

#[test]
fn a_coalesce_keeps_both_operands() {
    // Modelled for what it does to its left operand -- PHP reads a property chain there without
    // complaining about a null in the middle -- but both halves stay reachable, because a member
    // access written in the default is a member access like any other.
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run(?Repo $repo): void
    {
        $a = $repo->size ?? $this->fallback();
    }
}
"#,
        expect![[r#"
            <file scope>
              body @ 1:1-9:1
                ?? declaration (arrival) @ 2:1-8:2
            Repo::run
              body @ 5:5-7:6
                assign @ 6:9-6:46
                  $a @ 6:9-6:11
                  =
                    ?? @ 6:14-6:46
                      property ->size @ 6:14-6:25
                        receiver
                          $repo @ 6:14-6:19
                      call ->fallback() @ 6:29-6:46
                        receiver
                          $this @ 6:29-6:34
        "#]],
    );
}

#[test]
fn a_for_loop_keeps_its_body_as_statements() {
    // An unmodelled statement keeps its expressions and not its statements, so before this every
    // `foreach`, `catch` and guarding `if` written inside a `for` loop was invisible to every
    // statement walk -- and nothing failed, because the expressions inside were still reachable.
    check_bodies(
        r#"
//- /src/Repo.php
<?php
class Repo
{
    public function run(int $count): void
    {
        for ($i = 0; $i < $count; $i++) {
            foreach ($this->all() as $one) {
                $one->save();
            }
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
                for @ 6:9-10:10
                  assign @ 6:14-6:20
                    $i @ 6:14-6:16
                    =
                      int @ 6:19-6:20
                  compare < @ 6:22-6:33
                    $i @ 6:22-6:24
                    $count @ 6:27-6:33
                  compound assign @ 6:35-6:39
                    $i @ 6:35-6:37
                  foreach @ 7:13-9:14
                    call ->all() @ 7:22-7:34
                      receiver
                        $this @ 7:22-7:27
                    value
                      $one @ 7:38-7:42
                    call ->save() @ 8:17-8:29
                      receiver
                        $one @ 8:17-8:21
        "#]],
    );
}
