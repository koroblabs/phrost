//! A method that declares no return type, carries no `@return`, and hands back a chain.
//!
//! `SqlBase::select()` in Drupal core is the shape: nothing written about it says anything, and its
//! whole body is `return $this->getDatabase()->select($table, $alias, $options);`. A reader
//! following `$this->select('t')->fields(...)` in a migrate source got the first hop and nothing
//! after it, because the wrapper's type is written only in the expression it forwards.
//!
//! Core declares **4 181** methods with neither a return type nor a `@return`. 505 of them return a
//! `new`, which was already read; 1 235 are a chain like this, which was not.
//!
//! **`$this` means the class the caller reached it through.** A subclass calling an inherited
//! wrapper gets its own override, which is what late static binding is for -- so the stored root
//! for a receiver is no class at all, and the caller supplies one.

use expect_test::expect;

use super::{Query, check_view};

const PROJECT: &str = r#"
//- /src/Db.php
<?php
namespace App;

class Select
{
    public function fields(string $table): self {}
}

class Connection
{
    /**
     * @return \App\Select
     */
    public function select(string $table) {}
}

class Base
{
    public function connection(): Connection {}

    /** Wrapper for database select. */
    public function select($table)
    {
        return $this->connection()->select($table);
    }

    /** A wrapper whose own body forwards to another wrapper. */
    public function shortcut()
    {
        return $this->select('t');
    }

    /** A chain rooted at a written class rather than at `$this`. */
    public function viaClass()
    {
        return Registry::current()->select('t');
    }

    /** What the subclass overrides, so late static binding is visible. */
    public function build(): Select {}

    /** Returns through an override the subclass supplies. */
    public function built()
    {
        return $this->build();
    }
}

class Registry
{
    public static function current(): Connection {}
}

class Sub extends Base
{
    public function build(): Widget {}
}

class Widget
{
    public function render(): string {}
}

class Use1
{
    public function run(Base $base, Sub $sub): void
    {
        $wrapped = $base->select('t');
        $twoDeep = $base->shortcut();
        $rooted = $base->viaClass();
        $chained = $base->select('t')->fields('u');
        $late = $sub->built();

        $w@wrapped@rapped;
        $t@twoDeep@woDeep;
        $r@rooted@ooted;
        $c@chained@hained;
        $l@late@ate;
    }
}
"#;

#[test]
fn a_wrapper_hands_back_what_its_body_forwards() {
    check_view(
        PROJECT,
        &[
            Query::variable_type("a one-hop wrapper", "wrapped"),
            Query::variable_type("a wrapper forwarding to a wrapper", "twoDeep"),
            Query::variable_type("a chain rooted at a written class", "rooted"),
            Query::variable_type("and the call after it", "chained"),
            Query::variable_type("`$this` is the class the caller used", "late"),
        ],
        expect![[r#"
            a one-hop wrapper
            - Select
            a wrapper forwarding to a wrapper
            - Select
            a chain rooted at a written class
            - Select
            and the call after it
            - Select
            `$this` is the class the caller used
            - Widget
        "#]],
    );
}
