//! `in_array($x, $list, true)` that held: the needle is identical to one of the list's elements, so
//! it is what they are.
//!
//! The meet of two types, and each case below is one way of meeting: the elements narrower than the
//! needle (they are the answer), the needle a union some of whose alternatives no element can be
//! (those go), and the null, which goes exactly when no element can be one. A container that says
//! nothing about its elements says nothing about the needle.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_needle_found_in_a_container_is_one_of_its_elements() {
    check_view(
        r#"
//- /src/app.php
<?php
namespace App;

final class User {}
final class Admin {}

/** @param list<User> $users */
function nullable(?User $user, array $users): void
{
    if (in_array($user, $users, true)) {
        $n@nullable@ullable = $user;
    }
}

/** @param list<User> $users */
function unknown(mixed $user, array $users): void
{
    if (in_array($user, $users, true)) {
        $m@mixed@ixed = $user;
    }
}

/** @param list<User> $users */
function union(User|Admin|null $user, array $users): void
{
    if (in_array($user, $users, true)) {
        $u@union@nion = $user;
    }
}

/** @param list<?User> $users */
function nullElements(?User $user, array $users): void
{
    if (in_array($user, $users, true)) {
        $e@nullable_elements@lements = $user;
    }
}

/** @param array<string, int> $ids */
function scalars(int|string|null $id, array $ids): void
{
    if (in_array($id, $ids, true)) {
        $s@scalars@calars = $id;
    }
}

function nothingSaid(?User $user, array $users): void
{
    if (in_array($user, $users, true)) {
        $b@bare@are = $user;
    }
}

/** @param list<User> $users */
function loose(?User $user, array $users): void
{
    if (in_array($user, $users)) {
        $l@loose@oose = $user;
    }
}
"#,
        &[
            Query::variable_type("a nullable needle in a list of users", "nullable"),
            Query::variable_type("a needle nothing typed", "mixed"),
            Query::variable_type("a union of unrelated classes", "union"),
            Query::variable_type(
                "elements that may be null keep the null",
                "nullable_elements",
            ),
            Query::variable_type("a scalar union in a list of ints", "scalars"),
            Query::variable_type("a container that names no element", "bare"),
            Query::variable_type("a loose search", "loose"),
        ],
        expect![[r#"
            a nullable needle in a list of users
            - User
            a needle nothing typed
            - User
            a union of unrelated classes
            - User
            elements that may be null keep the null
            - ?User
            a scalar union in a list of ints
            - int
            a container that names no element
            - ?User
            a loose search
            - ?User
        "#]],
    );
}
