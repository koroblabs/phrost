//! An input a command declares in its `$signature`, read back by name.
//!
//! `$this->argument('user')` declares every scalar there is, because a signature cannot say which
//! input it is asked for; the class's own declaration can. Read from the file being analysed, where
//! a command's reads sit beside its signature -- so a command declared elsewhere keeps the method's
//! own answer, as does a name the declaration does not write and a class that is no command.
//!
//! The grammar is the framework's, and the real one is tested where it lives; this one is just
//! enough of it to see the view read what a parser hands back.

use expect_test::expect;
use phrost_harness::World;
use phrost_index::DeclaredInputsRule;
use phrost_model::{BuiltinTy, DeclaredInput, TextRange, ValueShape};
use phrost_testkit::FixtureSpec;

use super::{Query, check_world};

/// `{name}` a string, `{name?}` a string or nothing, `{--name}` a flag, `{--name=*}` a list.
fn inputs(signature: &str) -> Vec<DeclaredInput> {
    let text = ValueShape {
        builtin: BuiltinTy::String,
        nullable: false,
        list: false,
    };
    signature
        .split_whitespace()
        .filter_map(|token| token.strip_prefix('{')?.strip_suffix('}'))
        .map(|token| match token {
            _ if token.ends_with("=*") => (
                token.trim_end_matches("=*"),
                ValueShape {
                    nullable: true,
                    list: true,
                    ..text
                },
            ),
            _ if token.ends_with('?') => (
                token.trim_end_matches('?'),
                ValueShape {
                    nullable: true,
                    ..text
                },
            ),
            _ if token.starts_with("--") => (
                token,
                ValueShape {
                    builtin: BuiltinTy::Bool,
                    ..text
                },
            ),
            _ => (token, text),
        })
        .map(|(name, shape)| {
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

const FIXTURE: &str = r#"
//- /src/Console/Command.php
<?php
namespace Console;

class Command
{
    /** @return array|string|bool|null */
    public function argument($key = null) {}
    /** @return array|string|bool|null */
    public function option($key = null) {}
}

//- /src/Elsewhere.php
<?php
namespace App;

class Elsewhere extends \Console\Command
{
    protected $signature = 'elsewhere {user}';
}

//- /src/SendMail.php
<?php
namespace App;

class SendMail extends \Console\Command
{
    protected $signature = 'mail:send {user} {cc?} {--force} {--to=*}';

    public function handle(Elsewhere $other): void
    {
        $u@user@ser = $this->argument('user');
        $c@cc@c = $this->argument('cc');
        $f@force@orce = $this->option('force');
        $t@to@o = $this->option('to');
        $m@missing@issing = $this->argument('missing');
        $e@elsewhere@lsewhere = $other->argument('user');
    }
}

class NoCommand
{
    protected $signature = 'not {user}';

    /** @return array|string|bool|null */
    public function argument($key = null) {}

    public function handle(): void
    {
        $n@nocommand@o = $this->argument('user');
    }
}
"#;

#[test]
fn a_command_reads_back_what_its_signature_declares() {
    let spec = FixtureSpec::parse(FIXTURE);
    let world = World::build_with(&spec, |world| {
        // Before the files, because lowering keeps only the defaults it was told to.
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
                unless: vec!["configure".into()],
            });
    });
    check_world(
        &spec,
        &world,
        &[
            Query::variable_type("a required argument", "user"),
            Query::variable_type("an optional one", "cc"),
            Query::variable_type("an option with no value", "force"),
            Query::variable_type("an option taking many", "to"),
            Query::variable_type("a name the signature does not write", "missing"),
            Query::variable_type("a command declared in another file", "elsewhere"),
            Query::variable_type("a class that is no command", "nocommand"),
        ],
        expect![[r#"
            a required argument
            - string
            an optional one
            - ?string
            an option with no value
            - bool
            an option taking many
            - array<?string>
            a name the signature does not write
            - array|string|bool|null
            a command declared in another file
            - array|string|bool|null
            a class that is no command
            - array|string|bool|null
        "#]],
    );
}
