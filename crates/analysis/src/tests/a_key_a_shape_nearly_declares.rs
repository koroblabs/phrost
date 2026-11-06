//! `$row['titel']` where the shape declares `title`.
//!
//! Not "a key the shape does not hold", which is a different and unsafe claim: a docblock shape may
//! be written open (`array{id: int, ...}`), and a key this layer cannot read is skipped rather than
//! failing the shape -- either leaves a type that states *some* of what is there. What separates a
//! mistake from an absence is the same thing it separates for a catalog key, and it is the whole of
//! the rule here: a typo nearly hits.
//!
//! Prospective by design, like the catalog's near miss. It fires while somebody is typing the
//! mistake, which for a key read out of an array is the only moment it can be caught at all --
//! PHP's own answer at runtime is a warning and a null.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_key_one_edit_from_a_declared_one_is_reported() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Rows
{
    /** @return array{id: int, title: string} */
    public function row(): array
    {
        return ['id' => 1, 'title' => 't'];
    }

    public function read(): string
    {
        $row = $this->row();

        return $row['titel'];
    }
}
"#,
        expect![[r#"
            16:16-16:29 warning [misspelt-key] 'titel' is not declared; 'title' is one edit away
        "#]],
    );
}

/// A key nothing is close to says nothing: an unlisted key may be one the shape did not write.
#[test]
fn a_key_nothing_is_close_to_is_left_alone() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Rows
{
    /** @return array{id: int, title: string} */
    public function row(): array
    {
        return ['id' => 1, 'title' => 't'];
    }

    public function read(): mixed
    {
        $row = $this->row();

        return $row['created_at'];
    }
}
"#,
        expect![""],
    );
}
