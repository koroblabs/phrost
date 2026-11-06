//! What a variable is, and where it was bound.
//!
//! The one thing hover describes that no file declares. A local exists because a parameter, an
//! assignment, a `foreach`, or a `catch` put it there, and saying which is as useful as saying what
//! it holds.

use expect_test::expect;

use super::{Query, check};

#[test]
fn a_parameter_at_its_declaration_and_at_its_uses() {
    check(
        r#"
//- /src/app.php
<?php

class Repo {}

class Service
{
    public function run(Repo $rep@declared@o, int $count): void
    {
        $re@used@po;
        $cou@scalar@nt;
    }
}
"#,
        &[
            Query::hover("at the declaration", "declared"),
            Query::hover("at a use", "used"),
            Query::goto("goto from a use", "used"),
            Query::hover("a scalar", "scalar"),
        ],
        expect![[r#"
            at the declaration
            - code: Repo $repo
            at a use
            - code: Repo $repo
            - note: parameter
            goto from a use
            - parameter repo @ src/app.php 7:30-7:35 (full 7:30-7:35)
            a scalar
            - code: int $count
            - note: parameter
        "#]],
    );
}

#[test]
fn an_assignment_names_what_it_holds_at_the_target_and_after_it() {
    check(
        r#"
//- /src/app.php
<?php

class Repo {}

class Service
{
    public function run(): void
    {
        $lo@target@cal = new Repo();
        $loc@later@al;
    }
}
"#,
        &[
            Query::hover("on the target", "target"),
            Query::hover("after it", "later"),
            Query::goto("goto from after it", "later"),
        ],
        expect![[r#"
            on the target
            - code: Repo $local
            - note: assigned
            after it
            - code: Repo $local
            - note: assigned
            goto from after it
            - variable local @ src/app.php 9:9-9:15 (full 9:9-9:15)
        "#]],
    );
}

#[test]
fn a_loop_variable_a_caught_one_and_a_narrowed_one() {
    // `$one` gets its type from the docblock over a native `array`: the two are halves of one claim,
    // and `array` alone says nothing about what is inside.
    check(
        r#"
//- /src/app.php
<?php

class Repo {}
class Other {}
class Failure extends \Exception {}

class Service
{
    /** @param list<Repo> $items */
    public function run(array $items, object $thing): void
    {
        foreach ($items as $one) {
            $o@loop@ne;
        }

        try {
            $items[0];
        } catch (Failure $err@site@or) {
            $err@caught@or;
        }

        if ($thing instanceof Other) {
            $thi@narrowed@ng;
        }
    }
}
"#,
        &[
            Query::hover("loop", "loop"),
            Query::goto("goto the loop variable", "loop"),
            Query::hover("caught", "caught"),
            Query::goto("goto the caught one", "caught"),
            // The binding site itself, which answered nothing while every use of it answered. A
            // `catch` writes its name in the clause rather than in the body, so it is no expression
            // the variable walk would meet -- the same shape as a parameter, and the same fix.
            Query::hover("at the catch clause", "site"),
            Query::goto("goto from the catch clause", "site"),
            Query::hover("narrowed", "narrowed"),
            // A guard is not a binding: the value comes from the `instanceof`, the declaration is
            // still the parameter, and that is where a reader wants to land.
            Query::goto("goto the narrowed one", "narrowed"),
        ],
        expect![[r#"
            loop
            - code: Repo $one
            - note: loop variable
            goto the loop variable
            - variable one @ src/app.php 12:28-12:32 (full 12:28-12:32)
            caught
            - code: Failure $error
            - note: caught
            goto the caught one
            - variable error @ src/app.php 18:26-18:32 (full 18:26-18:32)
            at the catch clause
            - code: Failure $error
            - note: caught
            goto from the catch clause
            - variable error @ src/app.php 18:26-18:32 (full 18:26-18:32)
            narrowed
            - code: Other $thing
            - note: narrowed by a guard
            goto the narrowed one
            - parameter thing @ src/app.php 10:46-10:52 (full 10:46-10:52)
        "#]],
    );
}

#[test]
fn a_variable_nothing_declares_says_so() {
    // The absence is the useful part: it is the reason completion offers nothing on this name, and a
    // reader who is told can fix it by writing a type.
    check(
        r#"
//- /src/app.php
<?php

class Service
{
    public function run($untyped): void
    {
        $unty@here@ped;
    }
}
"#,
        &[Query::hover("untyped", "here")],
        expect![[r#"
            untyped
            - code: $untyped
            - note: parameter
            - note: no declared type
        "#]],
    );
}

#[test]
fn this_is_the_enclosing_class() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Service
{
    public function run(): void
    {
        $th@here@is;
    }
}
"#,
        &[Query::hover("this", "here")],
        expect![[r#"
            this
            - code: Service $this
        "#]],
    );
}

#[test]
fn a_cast_is_the_type_the_source_states() {
    // Reported from real code: hover on `$this->id()` said `int` and hover on `$uid` said nothing,
    // because the cast around it was unmodelled and the path to the value stopped there.
    check(
        r#"
//- /src/app.php
<?php

class Account
{
    public function id(): string { return '1'; }
}

class Service
{
    public function run(Account $account): void
    {
        $uid = (int) $account->id();
        $ui@here@d;
    }
}
"#,
        &[Query::hover("after a cast", "here")],
        expect![[r#"
            after a cast
            - code: int $uid
            - note: assigned
        "#]],
    );
}

#[test]
fn a_call_written_inside_a_cast_or_an_array_is_still_reachable() {
    // Both shapes state a type and contain expressions, and modelling the first without descending
    // into the second is how a call inside one stops answering.
    check(
        r#"
//- /src/app.php
<?php

class Repo
{
    public function find(): void {}
}

class Service
{
    public function run(Repo $repo): void
    {
        $one = (int) $repo->fi@cast@nd();
        $two = [$repo->fi@array@nd(), 2];
    }
}
"#,
        &[
            Query::goto("inside a cast", "cast"),
            Query::goto("inside an array", "array"),
        ],
        expect![[r#"
            inside a cast
            - method Repo::find @ src/app.php 5:21-5:25 (full 5:5-5:36)
            inside an array
            - method Repo::find @ src/app.php 5:21-5:25 (full 5:5-5:36)
        "#]],
    );
}

#[test]
fn a_type_written_in_full_is_shown_the_same_way_as_one_that_was_inferred() {
    // The one thing a reader notices in a popup is that two lines describing the same kind of fact
    // do not look the same. A `@var` tag and a parameter hint were rendered verbatim -- so an
    // author who wrote the fully qualified name got one style and everything inferred got another,
    // in the same file.
    //
    // The shape survives. Cutting `?Repo` down to `Repo` would hide nullability from the one place
    // a reader looks for it.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo {}

class Service
{
    public function inferred(): ?Repo
    {
    }

    public function run(\App\Repo $writ@param@ten): void
    {
        /** @var \App\Repo $tagged */
        $tag@tag@ged = null;
        $infe@inferred@rred = $this->inferred();
    }
}
"#,
        &[
            Query::hover("a parameter written in full", "param"),
            Query::hover("a var tag written in full", "tag"),
            Query::hover("a type nobody wrote", "inferred"),
        ],
        expect![[r#"
            a parameter written in full
            - code: Repo $written
            a var tag written in full
            - code: Repo $tagged
            - note: assigned
            a type nobody wrote
            - code: ?Repo $inferred
            - note: assigned
        "#]],
    );
}

#[test]
fn a_variable_is_described_while_the_access_after_it_is_still_being_typed() {
    // The moment an editor asks most: the arrow is there and the member name is not, so the
    // statement does not parse. What the variable holds is not in doubt, and answering nothing
    // reads as the server having lost track of it.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Service
{
    public function run(): void
    {
        $repo = new Repo();
        $re@dangling@po->
    }

    public function complete(): void
    {
        $repo = new Repo();
        $re@settled@po->find();
    }
}
"#,
        &[
            Query::hover("with the arrow dangling", "dangling"),
            Query::hover("with the access finished", "settled"),
        ],
        expect![[r#"
            with the arrow dangling
            - code: Repo $repo
            - note: assigned
            with the access finished
            - code: Repo $repo
            - note: assigned
        "#]],
    );
}

#[test]
fn a_dangling_arrow_mid_method_still_describes_the_variable_before_it() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Service
{
    public function run(): void
    {
        $repo = new Repo();
        $re@before_break@po->
        $other = 1;
    }
}
"#,
        &[Query::hover(
            "before a break with code after it",
            "before_break",
        )],
        expect![[r#"
            before a break with code after it
            - code: Repo $repo
            - note: assigned
        "#]],
    );
}

#[test]
fn the_arrow_itself_describes_nothing() {
    // There is no member there yet. Describing the receiver instead would answer a question nobody
    // asked, and the cursor is past the variable.
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Repo
{
    public function find(): void {}
}

class Service
{
    public function run(): void
    {
        $repo = new Repo();
        $repo->@on_arrow@
    }
}
"#,
        &[Query::hover("on the arrow", "on_arrow")],
        expect![[r#"
            on the arrow
            - (nothing to describe)
        "#]],
    );
}

/// A shape names every key the value holds, so a real one runs past what any popup shows on a
/// line: twelve keys and 250 characters, which arrived as a paragraph of punctuation.
///
/// Down the page instead, one key a line -- the same answer, in the shape a reader can scan. The
/// card has room; the hint drawn into the line does not, which is why that one folds instead.
#[test]
fn a_shape_too_wide_for_a_line_is_written_down_the_page() {
    check(
        r#"
//- /src/app.php
<?php

class Rules
{
    /**
     * @return ?array{package_type: string, weight_limit: string, distance_limit: int, gift_wrap: bool, gift_value: int, gift_amount: int, max_days_since_order: int, discount_reduction: float, rebate_rate: float, rebate_rate_floor: float, payout: bool, payout_max: int}
     */
    public function limits(): ?array
    {
    }

    /** @return array{app: string, count: int} */
    public function narrow(): array
    {
    }
}

class Service
{
    public function run(Rules $rules): void
    {
        $limits = $rules->limits();
        $row = $rules->narrow();
        $lim@wide@its;
        $r@narrow@ow;
    }
}
"#,
        &[
            Query::hover("the shape", "wide"),
            Query::hover("one that fits", "narrow"),
        ],
        expect![[r#"
            the shape
            - code: ?array{
                package_type: string,
                weight_limit: string,
                distance_limit: int,
                gift_wrap: bool,
                gift_value: int,
                gift_amount: int,
                max_days_since_order: int,
                discount_reduction: float,
                rebate_rate: float,
                rebate_rate_floor: float,
                payout: bool,
                payout_max: int
            } $limits
            - note: assigned
            one that fits
            - code: array{app: string, count: int} $row
            - note: assigned
        "#]],
    );
}
