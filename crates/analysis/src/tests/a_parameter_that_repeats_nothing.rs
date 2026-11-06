//! `{@inheritdoc}` over a *parameter* that states no type, which is the other half of how Drupal is
//! written.
//!
//! The return half has worked for as long as `a_signature_that_repeats_nothing` has: an override
//! saying nothing about its return is saying it has not changed. The input half said nothing at
//! all, so `FileSystem::deleteRecursive($path)` -- `{@inheritdoc}` over an untyped parameter whose
//! `@param string $path` is on `FileSystemInterface` -- took anything at all, and
//! `$fs->deleteRecursive($fs->realpath($uri))` passed `realpath()`'s `false` into it unremarked.
//! Two constructors of one rule, with the two halves of one signature as the two constructors.
//!
//! Three things stop the carry, and each is a test below. A parameter that wrote its own `@param`
//! keeps it -- *written*, which is not the same as resolved: a tag naming a template interns to
//! nothing, and reading the empty slot as silence carried an interface's `bool` over a trait's
//! `TWhenParameter` in Laravel's own type tests. A parameter that wrote a native type keeps that
//! too, because on the input side a narrower claim from above contradicts a declaration rather than
//! refining it -- the asymmetry with the return half, and the direction §8 records four times over.
//! And a renamed parameter carries nothing, because position alone would be a claim about the wrong
//! value.

use expect_test::expect;

use super::diagnostics::check;

/// An interface documenting an untyped parameter, and something that hands back a `false`.
///
/// Declared here rather than reached for: the harness indexes no standard library, so a fixture
/// written with `realpath()` tests `undefined-function` and nothing else.
const SINK: &str = r#"
//- /src/Sink.php
<?php

interface Sink
{
    /**
     * @param string $path
     */
    public function put($path);
}

class Paths
{
    public function resolve(string $path): string|false
    {
        return $path;
    }
}
"#;

#[test]
fn an_untyped_parameter_takes_the_type_the_declaration_it_overrides_documents() {
    check(
        &format!(
            r#"{SINK}
//- /src/app.php
<?php

class Writes implements Sink
{{
    /**
     * {{@inheritdoc}}
     */
    public function put($path)
    {{
    }}
}}

function write(Writes $sink, Paths $paths, string $where): void
{{
    $sink->put($paths->resolve($where));
}}
"#
        ),
        expect![[r#"
            15:16-15:39 warning [possibly-false-argument] $path takes string, and this may be false
        "#]],
    );
}

#[test]
fn a_parameter_that_wrote_its_own_tag_keeps_it_even_where_nothing_could_be_made_of_it() {
    // Laravel's `Collection::when()` is this exactly: `Enumerable` documents `@param bool $value`
    // and the `Conditionable` trait that implements it documents
    // `@param (\Closure($this): TWhenParameter)|TWhenParameter|null $value`. The trait's tag names
    // a template, so it resolves to nothing -- and reading that as "said nothing" carried the
    // interface's `bool` down over it, for six false `incompatible-argument` on Laravel's own type
    // assertions. A tag that was *written* is an answer, whatever came of it.
    check(
        &format!(
            r#"{SINK}
//- /src/app.php
<?php

class Anything implements Sink
{{
    /**
     * @template TPath
     *
     * @param TPath $path
     */
    public function put($path)
    {{
    }}
}}

function write(Anything $sink, Paths $paths, string $where): void
{{
    $sink->put($paths->resolve($where));
}}
"#
        ),
        expect![""],
    );
}

#[test]
fn a_parameter_that_wrote_a_native_type_is_not_narrowed_from_above() {
    // `mixed` is the declaration saying *anything*, and on the input side that is a widening the
    // override chose. Symfony's `BooleanToStringTransformer::transform(mixed $value)` implements a
    // `@param TValue` on a `@template`d interface; carrying over the `mixed` put a class nothing
    // declares on a parameter its author had opened to everything.
    check(
        &format!(
            r#"{SINK}
//- /src/app.php
<?php

class Everything implements Sink
{{
    /**
     * {{@inheritdoc}}
     */
    public function put(mixed $path)
    {{
    }}
}}

function write(Everything $sink, Paths $paths, string $where): void
{{
    $sink->put($paths->resolve($where));
}}
"#
        ),
        expect![""],
    );
}

#[test]
fn a_renamed_parameter_carries_nothing_because_position_alone_would_name_the_wrong_value() {
    check(
        &format!(
            r#"{SINK}
//- /src/app.php
<?php

class Renames implements Sink
{{
    /**
     * {{@inheritdoc}}
     */
    public function put($destination)
    {{
    }}
}}

function write(Renames $sink, Paths $paths, string $where): void
{{
    $sink->put($paths->resolve($where));
}}
"#
        ),
        expect![""],
    );
}
