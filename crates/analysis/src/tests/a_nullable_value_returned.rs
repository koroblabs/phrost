//! A `return` handing back a value that may be null where the declaration admits none.
//!
//! The return's half of `?T where T expected`. `function get(): string { return $this->maybe(); }`
//! is a `TypeError` on the run where `maybe()` hands back nothing, and it was reported nowhere: the
//! type check sets nullable values aside on purpose, because null has its own vocabulary in this
//! checker, and that vocabulary had an entry for an argument and a receiver and not for a return.
//! Same severity split as those two -- a value that is null on every run is an error, one that may
//! be is the branch nobody tested -- and the same narrowing keeps it quiet: a guard above the
//! `return` has taken the null off the type by the time the value is resolved. The lazily filled
//! property at the end is the shape that decides whether this check is liveable on real code.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_return_that_may_be_null_is_reported_where_the_declaration_admits_none() {
    check(
        r#"
//- /app.php
<?php

class Svc
{
    private function maybe(): ?string { return null; }

    public function direct(): string
    {
        return $this->maybe();
    }

    public function throughALocal(): string
    {
        $x = $this->maybe();
        return $x;
    }

    public function always(): string
    {
        return null;
    }

    public function eitherWay(bool $c): string
    {
        return $c ? $this->maybe() : 'x';
    }

    public function guarded(): string
    {
        $x = $this->maybe();
        if ($x === null) {
            return '';
        }
        return $x;
    }

    public function earlyReturn(): string
    {
        $x = $this->maybe();
        if (!$x) {
            return 'none';
        }
        return $x;
    }

    public function admitsNull(): ?string
    {
        return $this->maybe();
    }

    public function admitsNullAsAUnion(): string|null
    {
        return $this->maybe();
    }

    public function admitsAnything(): mixed
    {
        return $this->maybe();
    }

    public function generator(): iterable
    {
        yield 1;
        return $this->maybe();
    }
}

class Registry
{
    private ?array $definitions = NULL;

    public function lazily(): array
    {
        if ($this->definitions === NULL) {
            $this->definitions = $this->build();
        }
        return $this->definitions;
    }

    public function conditionally(bool $eager): array
    {
        if ($eager) {
            $this->definitions = $this->build();
        }
        return $this->definitions;
    }

    private function build(): array { return []; }
}
"#,
        expect![[r#"
            9:16-9:30 warning [possibly-null-return] ?string returned where string is declared
            15:16-15:18 warning [possibly-null-return] ?string returned where string is declared
            20:16-20:20 error [null-return] Cannot return null where string is declared
            25:16-25:41 warning [possibly-null-return] ?string returned where string is declared
            85:16-85:34 warning [possibly-null-return] ?array returned where array is declared
        "#]],
    );
}
