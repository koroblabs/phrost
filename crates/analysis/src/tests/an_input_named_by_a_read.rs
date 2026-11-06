//! The name a command's read of an input is given: `$this->option('queue')`.
//!
//! The `$signature` is the only place an input is declared, so it is where goto on the name lands,
//! what a hover describes and what completion offers inside the quotes -- the inputs the method can
//! be given, an argument's to `argument()` and an option's to `option()`. A search lists the
//! declaration and every read; a rename is refused, since whoever runs the command names the input
//! too.

use expect_test::expect;
use phrost_harness::{DeclaredInputsRule, World};
use phrost_model::{BuiltinTy, DeclaredInput, TextRange, ValueShape};

use crate::tests::{Query, check_with};

/// `{name}` a string argument, `{--name}` a flag and `{--name=}` a value or nothing, each with an
/// optional ` : description` -- enough of the grammar to see the view read what a parser hands back.
fn inputs(signature: &str) -> Vec<DeclaredInput> {
    let mut found = Vec::new();
    let mut rest = signature;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let token = &rest[open + 1..open + close];
        rest = &rest[open + close + 1..];
        let (token, description) = match token.split_once(" : ") {
            Some((token, said)) => (token, Some(said.to_owned())),
            None => (token, None),
        };
        let shape = match (token.starts_with("--"), token.ends_with('=')) {
            (false, _) => ValueShape {
                builtin: BuiltinTy::String,
                nullable: false,
                list: false,
            },
            (true, false) => ValueShape {
                builtin: BuiltinTy::Bool,
                nullable: false,
                list: false,
            },
            (true, true) => ValueShape {
                builtin: BuiltinTy::String,
                nullable: true,
                list: false,
            },
        };
        let name = token.trim_end_matches('=');
        let at = signature
            .substr_range(name.trim_start_matches('-'))
            .expect("a slice of the signature");
        found.push(DeclaredInput {
            name: name.to_owned(),
            shape,
            range: TextRange::new((at.start as u32).into(), (at.end as u32).into()),
            description,
        });
    }
    found
}

fn rule(world: &mut World) {
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
}

const FIXTURE: &str = r#"
//- /src/Console/Command.php
<?php
namespace Console;

class Command
{
    public function argument($key = null) {}
    public function option($key = null) {}
}

//- /src/SendMail.php
<?php
namespace App;

class SendMail extends \Console\Command
{
    protected $signature = 'mail:send {user : The user to mail} {--dry-run} {--queue=}';

    public function handle(SendMail $other): void
    {
        $this->argument('us@argument@er');
        $this->option('que@option@ue');
        $this->option('dry-r@dashed@un');
        $this->option('verb@always@ose');
        $this->option('us@crossed@er');
        $other->option('que@other@ue');
        $this->option('queue');
    }
}

class Escaped extends \Console\Command
{
    protected $signature = "escaped\t{--queue=}";

    public function handle(): void
    {
        $this->option('qu@escaped@eue');
        $later = new class {
            public function option($key) {}
            public function run(): void
            {
                $this->option('qu@anonymous@eue');
            }
        };
    }
}
"#;

/// Each name goes to where the signature writes it, a dash in it included; a name every command has
/// is described and goes nowhere, and an argument's name given to `option()` -- or a read through
/// anything but `$this` -- names nothing.
#[test]
fn a_read_reaches_the_input_its_signature_writes() {
    check_with(
        FIXTURE,
        &[
            Query::goto("an argument", "argument"),
            Query::goto("an option", "option"),
            Query::goto("a name with a dash", "dashed"),
            Query::goto("what every command has", "always"),
            Query::goto("an argument's name read as an option", "crossed"),
            Query::goto("another command's read", "other"),
            Query::goto("a signature whose text is not its value", "escaped"),
            Query::goto("a class inside the command", "anonymous"),
            Query::hover("hover on an argument", "argument"),
            Query::hover("hover on an option", "option"),
            Query::hover("hover on what every command has", "always"),
            Query::hover("hover where the text is not the value", "escaped"),
            Query::hover("hover inside a class inside the command", "anonymous"),
        ],
        rule,
        expect![[r#"
            an argument
            - `user` @ src/SendMail.php 6:40
            an option
            - `queue` @ src/SendMail.php 6:80
            a name with a dash
            - `dry-run` @ src/SendMail.php 6:68
            what every command has
            - (nothing to go to)
            an argument's name read as an option
            - (nothing to go to)
            another command's read
            - (nothing to go to)
            a signature whose text is not its value
            - (nothing to go to)
            a class inside the command
            - (nothing to go to)
            hover on an argument
            - code: 'user'
            - note: argument user: string -- The user to mail
            - note: declared at SendMail.php:6
            hover on an option
            - code: 'queue'
            - note: option --queue: ?string
            - note: declared at SendMail.php:6
            hover on what every command has
            - code: 'verbose'
            - note: option --verbose, which every command has
            hover where the text is not the value
            - code: 'queue'
            - note: option --queue: ?string
            hover inside a class inside the command
            - (nothing to describe)
        "#]],
    );
}

/// What the quotes can hold: the inputs the method reads, with what each holds, whatever was typed
/// before the cursor narrowing them -- dash included -- and the whole literal replaced.
#[test]
fn completion_offers_what_the_method_reads() {
    let fixture = FIXTURE
        .replace("@argument@", "")
        .replace("@dashed@", "")
        .replace("@always@", "")
        .replace("@crossed@", "")
        .replace("@other@", "")
        .replace("@escaped@", "")
        .replace("@anonymous@", "")
        .replace("'que@option@ue'", "'@option@'")
        .replacen(
            "$this->option('queue');",
            "$this->argument('@argument@');",
            1,
        );
    check_with(
        &fixture,
        &[
            Query::completion("an option", "option"),
            Query::completion("an argument", "argument"),
        ],
        rule,
        expect![[r#"
            an option
            - dry-run [catalog key]  bool
            - queue [catalog key]  ?string
            - accepting the first leaves `$this->option('dry-run');`
            an argument
            - user [catalog key]  string
            - accepting the first leaves `$this->argument('user');`
        "#]],
    );
    check_with(
        &FIXTURE
            .replace("@argument@", "")
            .replace("@option@", "")
            .replace("@always@", "")
            .replace("@crossed@", "")
            .replace("@other@", "")
            .replace("@escaped@", "")
            .replace("@anonymous@", "")
            .replace("@escaped@", "")
            .replace("@anonymous@", "")
            .replace("'dry-r@dashed@un'", "'dry-@dashed@'"),
        &[Query::completion("past a dash", "dashed")],
        rule,
        expect![[r#"
            past a dash
            - dry-run [catalog key]  bool
            - ... and the client must ask again
            - accepting the first leaves `$this->option('dry-run');`
        "#]],
    );
}

/// From a read and from the declaration alike: the name in the signature and every read of it on
/// `$this`, and nothing from another command's read. A rename is refused rather than half done.
#[test]
fn references_list_the_declaration_and_every_read() {
    use std::fmt::Write as _;

    let fixture = FIXTURE
        .replace("@argument@", "")
        .replace("@dashed@", "")
        .replace("@always@", "")
        .replace("@crossed@", "")
        .replace("@other@", "")
        .replace("@escaped@", "")
        .replace("@anonymous@", "")
        .replacen("{--queue=}", "{--qu@declared@eue=}", 1);
    let spec = phrost_testkit::FixtureSpec::parse(&fixture);
    let world = World::build_with(&spec, rule);
    let mut out = String::new();
    for marker in ["option", "declared"] {
        let position = spec.markers().position(marker);
        let file = world.file_id(&position.path);
        let open = world.open(phrost_harness::OpenScope::EveryFile, Some(file));
        let view = phrost_view::IndexView::in_file(world.skeleton(), &open, file);
        let target =
            crate::reference_target(&view, position.offset).expect("an input to search for");
        let _ = writeln!(out, "from {marker}:");
        for range in crate::references_in(&view, &target) {
            let _ = writeln!(out, "  {}", world.render_range(file, range));
        }
        let _ = writeln!(
            out,
            "  rename: {}",
            crate::rename_refusal(&view, &target, None).unwrap_or_default()
        );
    }
    spec.markers().assert_all_used();
    expect![[r#"
        from option:
          6:80-6:85
          11:24-11:29
          16:24-16:29
          rename: `--queue` is also named by whatever runs the command -- the scheduler, `Artisan::call()`, a test, a crontab -- so a rename here would leave those passing the old name
        from declared:
          6:80-6:85
          11:24-11:29
          16:24-16:29
          rename: `--queue` is also named by whatever runs the command -- the scheduler, `Artisan::call()`, a test, a crontab -- so a rename here would leave those passing the old name
    "#]]
    .assert_eq(&out);
}
