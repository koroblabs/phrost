//! `@deprecated` and `#[\Deprecated]`, which say the same thing in two spellings.
//!
//! PHP 8.4 gave the fact an attribute and 8.5.10 reports reading one at runtime -- `Constant X is
//! deprecated`. The docblock is what every framework actually writes: 797 of them in WordPress
//! against seven attributes in Symfony. So one bit records whichever said it, read in one place,
//! and the list a person picks from is where it has to show -- a warning that arrives when they
//! highlight the row has arrived after they chose it.

use expect_test::expect;

use super::{Query, check};

const FIXTURE: &str = r#"
//- /src/app.php
<?php

/**
 * @deprecated 2.0 use Fresh instead
 */
class Old
{
    /** @deprecated use open() */
    public function unlock(): bool
    {
        return true;
    }

    #[\Deprecated(message: 'use open()', since: '2.0')]
    public function lock(): bool
    {
        return true;
    }

    /** @deprecated */
    public const LIMIT = 10;

    /** @deprecated */
    public int $count = 0;

    public function fine(): void {}
}

/** @deprecated */
enum Stage: string
{
    case Open = 'open';
}

class Promoted
{
    public function __construct(
        /** @deprecated */
        public readonly int $id = 0,
    ) {}
}

class Fresh {}

/** @deprecated */
function legacy(): void {}

function current(): void {}

class Caller
{
    public function write(): void
    {
        $x = O@name@ld::class;
    }
}

//- /src/members.php
<?php

class Reader
{
    public function run(Old $old): void
    {
        // Last in its own file: the marker leaves a dangling `->`, and mago drops what follows it.
        $old->@member@
    }
}

//- /src/statics.php
<?php

class Statics
{
    public function run(): void
    {
        Old::@static@
    }
}

//- /src/promoted.php
<?php

class Reads
{
    public function run(Promoted $it): void
    {
        $it->@promoted@
    }
}
"#;

#[test]
fn both_spellings_mark_the_row_a_person_picks_from() {
    check(
        FIXTURE,
        &[
            Query::completion("members of a deprecated class", "member"),
            Query::completion("names in scope", "name"),
            Query::completion("static members and constants", "static"),
            Query::completion("a promoted property", "promoted"),
        ],
        expect![[r#"
            members of a deprecated class
            - fine [method]  : void  writes `();`
            - lock [method]  : bool  deprecated  writes `();`
            - unlock [method]  : bool  deprecated  writes `();`
            - count [property]  int  deprecated
            - accepting the first leaves `$old->fine`
            names in scope
            - Old [class]  Old  deprecated
            - ... and the client must ask again
            - accepting the first leaves `$x = Oldld::class;`
            static members and constants
            - LIMIT [const]  int  deprecated
            - class [const]  Old
            - accepting the first leaves `Old::LIMIT`
            a promoted property
            - __construct [method]  shown last  writes `(|);`
            - id [property]  int  deprecated
            - accepting the first leaves `$it->id`
        "#]],
    );
}
