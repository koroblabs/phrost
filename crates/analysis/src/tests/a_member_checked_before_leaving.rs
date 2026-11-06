//! `method_exists` written as a check that leaves when it fails, and on a class named by a value.
//!
//! `if (!method_exists($this->notifier, 'getAdminRecipients')) { return []; }` vouches for every
//! call after it in the block, and so does the `||` a failed check is usually one term of --
//! `if ($e === null || !method_exists($e, 'context'))` -- which is De Morgan on the negative side.
//! Only the enclosing branch and a sibling operand were read, and Symfony and Laravel wrote four of
//! these the other way. And a static call on a class-string, `$this->type::tryFrom()`, is checked
//! by `method_exists($this->type, 'tryFrom')`: the value the class is named by.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_check_that_left_vouches_for_what_follows() {
    check(
        r#"
//- /src/app.php
<?php

interface Notifier
{
}

interface Kind
{
}

function method_exists(object|string $object_or_class, string $method): bool
{
}

class Handler
{
    /** @var class-string<Kind> */
    private string $type = Kind::class;

    public function early(Notifier $notifier): array
    {
        if (!method_exists($notifier, 'recipients')) {
            return [];
        }
        return $notifier->recipients();
    }

    public function either(?Notifier $notifier): array
    {
        if ($notifier === null || !method_exists($notifier, 'recipients')) {
            return [];
        }
        return $notifier->recipients();
    }

    public function static_call(string $value): mixed
    {
        if (!method_exists($this->type, 'tryFrom')) {
            return null;
        }
        return $this->type::tryFrom($value);
    }

    public function unguarded(Notifier $notifier): array
    {
        return $notifier->recipients();
    }
}
"#,
        expect![[r#"
            46:27-46:37 error [undefined-method] Undefined method Notifier::recipients()
        "#]],
    );
}
