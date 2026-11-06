//! A command reading an input its `$signature` does not declare.
//!
//! Symfony throws on the read -- `The "queu" option does not exist.` -- so it is an error, and a
//! typo one edit from a declared name says which. What every command has beside its signature is
//! declared too; a class declaring `configure()` or calling `addOption()`, which can add any input,
//! is not judged, and nor is one extending the command through a class of its own, which may add
//! some in its constructor. Only `$this`: the declaration is about the instance the method runs on.

use expect_test::expect;
use phrost_harness::DeclaredInputsRule;
use phrost_model::{BuiltinTy, DeclaredInput, TextRange, ValueShape};

use super::diagnostics::check_framework;

/// `{name}` an argument, `{--name}` an option -- enough grammar to see what is declared.
fn inputs(signature: &str) -> Vec<DeclaredInput> {
    let shape = ValueShape {
        builtin: BuiltinTy::String,
        nullable: false,
        list: false,
    };
    signature
        .split_whitespace()
        .filter_map(|token| token.strip_prefix('{')?.strip_suffix('}'))
        .map(|token| {
            let name = token.trim_end_matches('=');
            let at = signature
                .substr_range(name.trim_start_matches('-'))
                .expect("a slice of the signature");
            DeclaredInput {
                name: name.to_owned(),
                shape,
                range: TextRange::new((at.start as u32).into(), (at.end as u32).into()),
                description: None,
            }
        })
        .collect()
}

#[test]
fn a_read_of_an_input_the_signature_does_not_declare_is_reported() {
    check_framework(
        r#"
//- /src/Console/Command.php
<?php
namespace Console;

class Command
{
    public function argument($key = null) {}
    public function option($key = null) {}
    public function addOption(string $name) {}
}

//- /src/SendMail.php
<?php
namespace App;

class SendMail extends \Console\Command
{
    protected $signature = 'mail:send {user} {--queue=}';

    public function handle(SendMail $other): void
    {
        $this->argument('user');
        $this->option('queue');
        $this->option('verbose');
        $this->option('queu');
        $this->argument('usr');
        $this->argument('queue');
        $this->option('nothing-near');
        $other->option('queu');
    }
}

class Configured extends \Console\Command
{
    protected $signature = 'configured {user}';

    protected function configure(): void {}

    public function handle(): void
    {
        $this->option('anything');
    }
}

class Added extends \Console\Command
{
    protected $signature = 'added {user}';

    public function __construct()
    {
        $this->addOption('extra');
    }

    public function handle(): void
    {
        $this->option('anything');
    }
}

class Generated extends Generator
{
    protected $signature = 'make:thing {name}';

    public function handle(): void
    {
        $this->option('test');
    }
}

abstract class Generator extends \Console\Command {}
"#,
        |world| {
            world.set_kept_defaults(&["signature"], &[]);
            world
                .contracts_mut()
                .bind_declared_inputs(DeclaredInputsRule {
                    class: "Console\\Command".into(),
                    property: "signature".into(),
                    readers: vec![
                        ("argument".into(), "".into()),
                        ("option".into(), "--".into()),
                    ],
                    parse: inputs,
                    always: vec!["--verbose".into()],
                    unless: vec!["configure".into(), "addOption".into()],
                });
        },
        expect![[r#"
            13:23-13:29 error [undefined-input] The signature declares no option `queu`. Did you mean `queue`?
            14:25-14:30 error [undefined-input] The signature declares no argument `usr`. Did you mean `user`?
            15:25-15:32 error [undefined-input] The signature declares no argument `queue`
            16:23-16:37 error [undefined-input] The signature declares no option `nothing-near`
        "#]],
    );
}
