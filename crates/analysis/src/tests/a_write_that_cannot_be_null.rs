//! A slot written something the declaration already allowed, and what the write still says.
//!
//! An assignment to a property *refines* what the declaration says and can never widen it -- PHP
//! checks a property write at runtime -- so a write that says nothing more specific loses to the
//! declaration. That is right about the type and drops one thing the write does say:
//!
//! ```php
//! /** @var array<int, string>|null */
//! protected ?array $permissions = NULL;
//!
//! if ($this->permissions === NULL) {
//!     $this->permissions = JsonUtility::decode($this->_value('field_permissions')) ?? [];
//! }
//! return $this->permissions;     // "?array<int, string> returned where array is declared"
//! ```
//!
//! `decode` is declared `mixed`, so the write cannot replace a `?array<int, string>` -- and the
//! `??` says the one thing about it that matters here. The declaration stands and loses its null,
//! because what got through the write is both what the declaration allows and what was written.
//!
//! `mixed` is exactly why this needs the *path* rather than the type: `union_of` folds a `mixed`
//! operand into the whole answer, deliberately, so the coalesce resolves to `mixed` and the
//! operator's guarantee is gone by the time a type is in hand. And `mixed` is the one type that
//! answers "cannot be null" without meaning it, which is what keeps it silent everywhere -- so a
//! bare `mixed` write states nothing and the null stays. The pair of tests below is that
//! difference.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_coalesced_write_takes_the_declarations_null_with_it() {
    check(
        r#"
//- /app.php
<?php

class JsonInvalidFormatException {}

class JsonUtility
{
    /**
     * @return mixed
     *   Whatever the JSON held.
     */
    public static function decode(string $text): mixed { return NULL; }
}

trait Wrapper
{
    protected function _value(string $field): ?string { return NULL; }
}

class CoreUser {}

class Own
{
    /**
     * @var array<int, string>|null
     */
    private ?array $mine = NULL;

    private function value(string $field): ?string { return NULL; }

    public function mine(): array
    {
        if ($this->mine === NULL) {
            try {
                $this->mine = JsonUtility::decode($this->value('f') ?? '[]') ?? [];
            }
            catch (JsonInvalidFormatException) {
                $this->mine = [];
            }
        }
        return $this->mine;
    }
}

class User extends CoreUser
{
    use Wrapper;

    /**
     * User custom permissions.
     *
     * @var array<int, string>|null
     */
    protected ?array $permissions = NULL;

    /**
     * Retrieves the permissions associated with the current entity.
     *
     * @return array
     *   An array of permissions.
     */
    public function getPermissions(): array
    {
        if ($this->permissions === NULL) {
            try {
                $this->permissions = JsonUtility::decode($this->_value('field_permissions') ?? '[]') ?? [];
            }
            catch (JsonInvalidFormatException) {
                $this->permissions = [];
            }
        }
        return $this->permissions;
    }
}
"#,
        expect![[r#""#]],
    );
}

#[test]
fn a_write_that_states_nothing_leaves_the_null_where_it_was() {
    check(
        r#"
//- /app.php
<?php

/**
 * @param string $hook
 * @param mixed $value
 * @return mixed
 *   The filtered value.
 */
function apply_filters(string $hook, $value) { return $value; }

function make(): mixed { return NULL; }

class Screen
{
    /** @var ?bool */
    private $shown;

    /** @var ?bool */
    private $direct;

    /** @var ?bool */
    private $coalesced;

    public function filtered(): bool
    {
        $show = TRUE;
        $this->shown = apply_filters('x', $show);
        return $this->shown;
    }

    public function direct(): bool
    {
        $this->direct = make();
        return $this->direct;
    }

    public function coalesced(): bool
    {
        $this->coalesced = make() ?? FALSE;
        return $this->coalesced;
    }
}
"#,
        expect![[r#"
            28:16-28:28 warning [possibly-null-return] ?bool returned where bool is declared
            34:16-34:29 warning [possibly-null-return] ?bool returned where bool is declared
        "#]],
    );
}
