//! A `NULL` the author wrote that can come back out of a function declared not to return one, where
//! nothing resolves the rest of the value.
//!
//! The reference project's `StockReport::arf(): string` was `$arf = $this->data['arf'] ?? NULL;
//! ... return $arf;`, fixed by hand after it returned null in production. It was silent here because
//! `$this->data` is a bare `array`: its element is not known, so the union with it is not built and
//! the return had no type to judge. The `NULL` is not a guess about that element, so the path says
//! what the type cannot.
//!
//! The negative half is most of it. A guard that throws on the null removes it; `$a ?? $b` replaces
//! the fallback of `$a` with `$b`; and a fallback written in a condition may be the very thing the
//! condition tested, which a walk of the path cannot see -- so it is not read.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_written_null_that_reaches_the_return_is_reported_and_one_that_is_guarded_is_not() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Missing {}

class Report
{
    public function __construct(protected array $data, protected array $cache) {}

    public function fellBack(): string
    {
        $arf = $this->data['arf'] ?? NULL;
        return $arf;
    }

    public function directly(): string
    {
        return $this->data['arf'] ?? NULL;
    }

    public function neverFilled(array $rows): string
    {
        $best = NULL;
        foreach ($rows as $row) {
            $best = $row;
        }
        return $best;
    }

    public function guarded(): string
    {
        $map = $this->data['map'] ?? NULL;
        if (!$map) {
            throw new Missing('none');
        }
        return $map;
    }

    public function replaced(string $other): string
    {
        $state = $this->data['state'] ?? NULL;
        return $state ?? $other;
    }

    public function testedInTheCondition(string $host): string
    {
        if ($host === '') {
            $ip = 'x';
        } elseif (null === $ip = $this->cache[$host] ?? null) {
            throw new Missing('unresolved');
        } else {
            $host = 'cached';
        }
        return $ip;
    }

    public function admitted(): ?string
    {
        return $this->data['arf'] ?? NULL;
    }
}
"#,
        expect![[r#"
            13:16-13:20 warning [possibly-null-return] a value that falls back to NULL returned where string is declared
            18:16-18:42 warning [possibly-null-return] a value that falls back to NULL returned where string is declared
            27:16-27:21 warning [possibly-null-return] a value that falls back to NULL returned where string is declared
        "#]],
    );
}
