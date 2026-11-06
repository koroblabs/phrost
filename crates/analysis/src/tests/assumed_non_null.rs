//! What a framework contract decided, said out loud.
//!
//! Three things decide whether a call is disclosed, and there is a test for each: whether the
//! contract hides a real sentinel or proves there is none, whether the call is in the state the
//! sentinel comes from, and whether anything would have refused the sentinel had it stayed. Stating
//! them in separate fixtures is the point -- a test with only the disclosable kind would pass with
//! any of the three distinctions deleted.

use expect_test::expect;
use phrost_harness::World;

use super::diagnostics::check_asked_for_with;

/// A framework that guarantees two nulls away, for two different reasons.
fn guarantees(world: &mut World) {
    world.contracts_mut().add_unproduced(
        "App\\Files",
        "path",
        phrost_model::BuiltinTy::Null,
        Some("the file has no path yet, as between create() and save()"),
        // A state of the object, which no argument of a call can rule out.
        None,
    );
    // Proved rather than judged: reporting it would be a false positive, so there is nothing to
    // tell anybody about.
    world.contracts_mut().add_unproduced(
        "App\\Files",
        "handle",
        phrost_model::BuiltinTy::Null,
        None,
        None,
    );
    // A state the call can be read for: the null needs a second argument asking for it.
    world.contracts_mut().add_unproduced(
        "App\\Files",
        "open",
        phrost_model::BuiltinTy::Null,
        Some("the call asks for it with a second argument"),
        Some(1),
    );
}

/// The two contracted methods, and a pair of parameters that differ only in taking a null.
const FILES: &str = r#"
//- /src/Files.php
<?php
namespace App;

class Files
{
    public function path(): ?string { return null; }

    public function handle(): ?string { return null; }

    public function open(string $path, ?int $flags = NULL): ?string
    {
        return NULL;
    }
}
"#;

#[test]
fn a_null_a_contract_hides_is_disclosed_and_a_null_it_disproves_is_not() {
    check_asked_for_with(
        &format!(
            r#"{FILES}
//- /src/Reader.php
<?php
namespace App;

class Reader
{{
    public function run(Files $files): string
    {{
        return $this->keep($files->path()) . $this->keep($files->handle());
    }}

    private function keep(string $value): string
    {{
        return $value;
    }}
}}
"#
        ),
        &["assumed-non-null"],
        guarantees,
        expect![[r#"
            8:36-8:40 note [assumed-non-null] path() is read as never null, though \App\Files::path() declares it: null only when the file has no path yet, as between create() and save()
        "#]],
    );
}

#[test]
fn a_contracted_call_nothing_would_have_refused_is_not_disclosed() {
    // The measurement this rule comes from: 130 disclosures on the reference project, of which
    // PHPStan at level 8 reports a null problem at 23. The other 107 are this shape -- a
    // concatenation, an array value, a `(string)` cast, a `?? ''` the author already wrote. The
    // decision was made there and no answer depended on it, so saying so is noise.
    check_asked_for_with(
        &format!(
            r#"{FILES}
//- /src/Reader.php
<?php
namespace App;

class Reader
{{
    public function run(Files $files): string
    {{
        return $this->tolerant($files->path()) . $files->path();
    }}

    private function tolerant(?string $value): string
    {{
        return $value ?? '';
    }}
}}
"#
        ),
        &["assumed-non-null"],
        guarantees,
        expect![""],
    );
}

#[test]
fn a_contracted_call_refused_through_the_local_it_was_assigned_to_is_disclosed() {
    // One hop, and it is not a rarity to be waved away: `$uri = $file->getFileUri();` on one line
    // and `is_file($uri)` on the next is four of the twenty-three the reference project has. The
    // disclosure stays on the call, because that is where a guard would go.
    check_asked_for_with(
        &format!(
            r#"{FILES}
//- /src/Reader.php
<?php
namespace App;

class Reader
{{
    public function run(Files $files): string
    {{
        $path = $files->path();

        return $this->keep($path);
    }}

    private function keep(string $value): string
    {{
        return $value;
    }}
}}
"#
        ),
        &["assumed-non-null"],
        guarantees,
        expect![[r#"
            8:25-8:29 note [assumed-non-null] path() is read as never null, though \App\Files::path() declares it: null only when the file has no path yet, as between create() and save()
        "#]],
    );
}

#[test]
fn a_state_the_call_is_plainly_not_in_is_not_disclosed() {
    // Why this exists: 233 of 362 disclosures on the reference project were
    // `ContainerInterface::get($id)`, whose null needs a second argument asking for it -- and every
    // one of the 233 passed a single argument. Two thirds of the output was about a state no call
    // in the project was in.
    //
    // Presence and not value, which is the honest limit: a call that does not pass the argument
    // cannot be in the state whatever the argument would have said, and reading the value would
    // mean resolving a class constant per call on a path a project switched on in order to read.
    check_asked_for_with(
        &format!(
            r#"{FILES}
//- /src/Reader.php
<?php
namespace App;

class Reader
{{
    public function plain(Files $files): string
    {{
        return $this->keep($files->open('a'));
    }}

    public function asking(Files $files): string
    {{
        return $this->keep($files->open('a', 1));
    }}

    private function keep(string $value): string
    {{
        return $value;
    }}
}}
"#
        ),
        &["assumed-non-null"],
        guarantees,
        expect![[r#"
            13:36-13:40 note [assumed-non-null] open() is read as never null, though \App\Files::open() declares it: null only when the call asks for it with a second argument
        "#]],
    );
}
