//! `resource`, which PHP will not let anybody declare and its own library writes 320 times.
//!
//! A stream, a file handle, a database link. It has no written-PHP spelling, so it arrives only from
//! a docblock -- and until it was in the vocabulary every one of those docblocks was a name
//! resolving to no class and was dropped whole. `fwrite(fopen($path, 'r'), 'x')` passed `fopen`'s
//! `false` into a parameter about which nothing at all was known, and `@param resource|CurlHandle`
//! lost *both* arms, because a union one of whose operands does not lower does not lower either.
//!
//! What it shares with the rest of the vocabulary is nothing. Measured on PHP 8.5.10: a file handle
//! passed to a declared `string`, `int`, `bool`, `array` or `object` parameter is a `TypeError` in
//! weak mode as well as strict -- the coercions that let a `false` reach a `string` do not exist for
//! one. `(string) $fh` still gives `Resource id #5`, which is a cast and a different question.

use expect_test::expect;

use super::diagnostics::check;

/// A parameter typed only by a docblock, and something that hands back a handle or a `false`.
///
/// Declared here rather than reached for: the harness indexes no standard library.
const STREAMS: &str = r#"
//- /src/Streams.php
<?php

class Streams
{
    /**
     * @return resource|false
     */
    public function open(string $path)
    {
        return NULL;
    }

    /**
     * @param resource $stream
     */
    public function write($stream, string $data): void
    {
    }

    public function name(string $label): string
    {
        return $label;
    }
}

/**
 * Declared because the harness indexes no standard library and an undefined callee is a finding of
 * its own. The guard is recognised by the name it is written with, so declaring it changes nothing
 * about what the narrowing reads.
 */
function is_resource($value): bool
{
    return TRUE;
}
"#;

#[test]
fn a_value_of_another_kind_cannot_be_a_resource() {
    check(
        &format!(
            r#"{STREAMS}
//- /src/app.php
<?php

function run(Streams $streams): void
{{
    $streams->write('not a stream', 'x');
}}
"#
        ),
        expect![[r#"
            5:21-5:35 error [incompatible-argument] $stream takes resource, and this is non-falsy-string
        "#]],
    );
}

#[test]
fn a_handle_that_may_be_false_is_reported_where_a_resource_is_wanted() {
    check(
        &format!(
            r#"{STREAMS}
//- /src/app.php
<?php

function run(Streams $streams, string $path): void
{{
    $streams->write($streams->open($path), 'x');
}}
"#
        ),
        expect![[r#"
            5:21-5:42 warning [possibly-false-argument] $stream takes resource, and this may be false
        "#]],
    );
}

#[test]
fn is_resource_takes_the_false_off_it() {
    // Added with the kind itself, because a type nothing can guard is a type that reports every
    // guarded use -- which is how a precision gain turns into noise on the first real project.
    check(
        &format!(
            r#"{STREAMS}
//- /src/app.php
<?php

function run(Streams $streams, string $path): void
{{
    $stream = $streams->open($path);

    if (is_resource($stream)) {{
        $streams->write($stream, 'x');
    }}
}}
"#
        ),
        expect![""],
    );
}

#[test]
fn a_resource_reaching_a_scalar_is_refused_in_weak_mode_too() {
    // The one pair in the table that does not soften without `strict_types`. This fixture declares
    // none, so weak mode is what is being read.
    check(
        &format!(
            r#"{STREAMS}
//- /src/app.php
<?php

function run(Streams $streams, string $path): void
{{
    $stream = $streams->open($path);

    if (is_resource($stream)) {{
        $streams->name($stream);
    }}
}}
"#
        ),
        expect![[r#"
            8:24-8:31 error [incompatible-argument] $label takes string, and this is resource
        "#]],
    );
}
