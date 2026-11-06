//! A constructor's template bound through a parameter that is a generic class, not a container.
//!
//! `@param Builder<TRelated> $query` says `TRelated` is what the builder was given at that position.
//! It used to be read as a container's element -- the last argument -- which is right only while the
//! template happens to be last, and asked a class PHP does not traverse what iterating it yields.
//! And a `Closure(): T` parameter binds `T` to what the closure handed over returns.

use expect_test::expect;

use super::{Query, check_view};

const FIXTURE: &str = r#"
//- /src/Orm.php
<?php
namespace App;

class User {}
class Team {}

/** @template TModel */
class Builder {}

/** @extends Builder<User> */
class UserBuilder extends Builder {}

/**
 * @template TFirst
 * @template TSecond
 */
class Pair {}

/**
 * @template TRelated
 * @template TOther
 */
class Relation
{
    /**
     * @param Builder<TRelated> $query
     * @param Pair<TOther, TRelated> $pair
     */
    public function __construct(Builder $query, Pair $pair) {}

    /** @return TRelated */
    public function related() {}

    /** @return TOther */
    public function other() {}
}

class Connection {}

/** @template TReturn */
class Transaction
{
    /** @param (\Closure(Connection): TReturn) $callback */
    public function __construct(\Closure $callback) {}

    /** @return TReturn */
    public function run(): mixed {}
}

/**
 * @param Builder<User> $users
 * @param Pair<Team, User> $pair
 */
function run(Builder $users, UserBuilder $built, Pair $pair): void
{
    $w@written@ritten = (new Relation($users, $pair))->related();
    $s@subclass@ubclass = (new Relation($built, $pair))->related();
    $f@first@irst = (new Relation($users, $pair))->other();
    $t@transacted@ransacted = (new Transaction(fn (Connection $c) => new Team()))->run();
}
"#;

#[test]
fn a_template_a_generic_class_parameter_names_is_read_by_position() {
    check_view(
        FIXTURE,
        &[
            Query::variable_type("the argument the value was written with", "written"),
            Query::variable_type("the argument a subclass gives the class", "subclass"),
            Query::variable_type("a template that is not the last argument", "first"),
            Query::variable_type("what the closure it is handed returns", "transacted"),
        ],
        expect![[r#"
            the argument the value was written with
            - User
            the argument a subclass gives the class
            - User
            a template that is not the last argument
            - Team
            what the closure it is handed returns
            - Team
        "#]],
    );
}
