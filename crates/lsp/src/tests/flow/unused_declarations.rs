//! What reaches a declaration when nothing in the project's PHP names it.
//!
//! `unused-declaration` is the editor's half of `phrost unused`, and the two were assembled
//! separately -- the report gathers the reachability rules itself and the diagnostic asks the
//! view. Two constructors of one rule, and the diagnostic's copy was missing two of them: a class
//! a framework builds because of what it *inherits*, and a member the project's *configuration*
//! names. On one real Drupal checkout that was 8 of 76 findings, every one of them a live entry
//! point reported as dead.
//!
//! This is the layer that can say so. A framework provider is installed here and configuration is
//! read here, so neither rule exists in a view unit test at all -- which is why they could go
//! missing without a test failing.

use expect_test::expect;

use crate::tests::Fixture;

/// What was reported, as a set.
///
/// Sorted and deduplicated because neither order nor repetition is what these tests are about:
/// the pass publishes from four worker threads, so the order files arrive in is the scheduler's,
/// and a project is swept twice when the check is on -- once to gather what it reaches and once
/// to judge against the whole of it. Comparing the raw sequence pins both and tests neither.
fn reported(fixture: &Fixture) -> Vec<String> {
    let mut lines = fixture.diagnostics_published();
    lines.sort();
    lines.dedup();
    lines
}

/// A Drush command class, a service built by a factory method, and one method nothing reaches.
const REACHED_BY_THE_FRAMEWORK: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "src/"}}}
//- /phrost.toml
framework = ["drupal"]
[lint]
on = ["unused-declaration"]
//- /app.services.yml
services:
  app.client_factory:
    class: App\ClientFactory
  app.mailer:
    class: App\Mailer
    factory: ['@app.client_factory', 'mailerClient']
//- /vendor/drush/DrushCommands.php
<?php
namespace Drush\Commands;

class DrushCommands {}
//- /src/Commands/BaseCommands.php
<?php
namespace App\Commands;

use Drush\Commands\DrushCommands;

abstract class BaseCommands extends DrushCommands {}
//- /src/Commands/ImportCommands.php
<?php
namespace App\Commands;

class ImportCommands extends BaseCommands
{
    public function importRural(): void {}
}
//- /src/ClientFactory.php
<?php
namespace App;

class ClientFactory
{
    public function mailerClient(): int { return 1; }

    public function unreachedClient(): int { return 2; }
}
"#;

#[tokio::test]
async fn a_command_class_a_framework_runs_by_inheritance_is_not_reported() {
    // Drush runs every `DrushCommands`, and a project writes its own abstract base between the
    // two -- so the rule has to climb rather than read the immediate parent. Nothing in the PHP
    // names either class or the command method, which is exactly what the report is about, and
    // exactly why saying "dead" here would be wrong.
    let mut fixture = Fixture::new(REACHED_BY_THE_FRAMEWORK);
    fixture.initialize(false, false).await;
    fixture.initialized().await;

    fixture.open("src/Commands/ImportCommands.php").await;

    expect![[r#"
        [
            "ClientFactory.php:8: unused-declaration method ClientFactory::unreachedClient is reached from nowhere in this project",
            "ImportCommands.php: none",
        ]
    "#]]
    .assert_debug_eq(&reported(&fixture));
}

#[tokio::test]
async fn a_method_a_container_factory_names_is_reached_and_its_neighbour_is_not() {
    // `factory: ['@id', 'method']` is the caller, and it is written in YAML: no PHP file holds
    // `->mailerClient` anywhere. Read as a class name -- which is what `factory` meant before this
    // -- the method was reached by nothing and a live service read as dead code. The neighbour is
    // the control: same class, same shape, named by nobody.
    let mut fixture = Fixture::new(REACHED_BY_THE_FRAMEWORK);
    fixture.initialize(false, false).await;
    fixture.initialized().await;

    fixture.open("src/ClientFactory.php").await;

    expect![[r#"
        [
            "ClientFactory.php:8: unused-declaration method ClientFactory::unreachedClient is reached from nowhere in this project",
        ]
    "#]]
    .assert_debug_eq(&reported(&fixture));
}

/// Two classes declaring one method name, and only one of them is called.
const ONE_NAME_TWO_CLASSES: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "src/"}}}
//- /phrost.toml
[lint]
on = ["unused-declaration"]
//- /src/Manager.php
<?php
namespace App;

class Manager
{
    public function importStockData(string $file): void {}
}
//- /src/Commands.php
<?php
namespace App;

class Commands
{
    public function __construct(private Manager $manager) {}

    public function importStockData(string $file): void
    {
        $this->manager->importStockData($file);
    }
}
//- /src/Kernel.php
<?php
namespace App;

class Kernel
{
    public function boot(Manager $manager): Commands
    {
        return new Commands($manager);
    }
}
"#;

#[tokio::test]
async fn a_method_is_judged_by_its_own_class_and_not_by_its_name() {
    // The case the word set cannot decide. `Manager::importStockData` is called; the identically
    // named method one line above the call is called by nobody, and matching on the bare name
    // keeps it alive off its neighbour's use. Worse, the call is in the *same file* as the dead
    // declaration, so the file's own text holds the word too -- both halves of the coarse rule
    // say "reached" and both are answering a different question than the one asked.
    //
    // The exact answer comes from the record the project pass fills: an access is attributed to
    // the class that *declares* what it resolved to, so this call is a claim on `Manager` and on
    // nothing else.
    let mut fixture = Fixture::new(ONE_NAME_TWO_CLASSES);
    fixture.initialize(false, false).await;
    fixture.initialized().await;

    // Both classes are named by a third file, so what is left to judge is the members -- a class
    // nothing reaches is reported instead of its members, and would pin the wrong rule here.
    // `Kernel` itself is that third file and nothing names it, so it is reported, correctly: in a
    // fixture with no framework and no configuration it is a class the project does not use.
    //
    // Each closed file is published twice because the project is swept twice: once to gather what
    // it reaches and once to judge against the whole of it. The two agree here, and where they do
    // not the second is the sharper -- it never withdraws a finding, only adds.
    fixture.open("src/Commands.php").await;
    fixture.open("src/Manager.php").await;

    expect![[r#"
        [
            "Commands.php:8: unused-declaration method Commands::importStockData is reached from nowhere in this project, though the name is written elsewhere and nothing resolves to this one",
            "Kernel.php:4: unused-declaration class Kernel is reached from nowhere in this project",
            "Manager.php: none",
        ]
    "#]]
    .assert_debug_eq(&reported(&fixture));
}

/// One short name, two namespaces, and only one of them is used.
const ONE_NAME_TWO_NAMESPACES: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "src/"}}}
//- /phrost.toml
[lint]
on = ["unused-declaration"]
//- /src/Billing/Repo.php
<?php
namespace App\Billing;

class Repo
{
    public function find(): int { return 1; }
}
//- /src/Legacy/Repo.php
<?php
namespace App\Legacy;

/**
 * The one nothing uses, declared lower down so the two can be told apart.
 */
class Repo
{
    public function find(): int { return 2; }
}
//- /src/Caller.php
<?php
namespace App;

use App\Billing\Repo;

class Caller
{
    public function run(Repo $repo): int { return $repo->find(); }
}
"#;

#[tokio::test]
async fn a_class_is_judged_by_its_namespace_and_not_by_its_short_name() {
    // `App\Legacy\Repo` is used by nobody and shares a word with a class that is. A word test
    // spares both, which is the same mistake one level up from judging a method by its name --
    // and the same fix: a reference is recorded against what it *resolved to*.
    //
    // The methods say it twice over: `Billing\Repo::find` is called and `Legacy\Repo::find` is
    // not, and only the class is reported for the legacy one because a class nothing reaches
    // says everything its members would.
    //
    // Both files are `Repo.php` and both classes are `Repo`, as PSR-4 requires -- and what is
    // published carries only the base name and the short name, so the *line* is what says which
    // one was reported. The dead one is declared lower down for exactly that reason.
    let mut fixture = Fixture::new(ONE_NAME_TWO_NAMESPACES);
    fixture.initialize(false, false).await;
    fixture.initialized().await;

    expect![[r#"
        [
            "Caller.php:6: unused-declaration class Caller is reached from nowhere in this project",
            "Repo.php:7: unused-declaration class Repo is reached from nowhere in this project, though the name is written elsewhere and nothing resolves to this one",
        ]
    "#]]
    .assert_debug_eq(&reported(&fixture));
}

/// Two constants, one read.
const CONSTANTS: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "src/"}}}
//- /phrost.toml
[lint]
on = ["unused-declaration"]
//- /src/Codes.php
<?php
namespace App;

class Codes
{
    public const KEPT = 1;

    public const DEAD = 2;
}
//- /src/Reader.php
<?php
namespace App;

class Reader
{
    public function read(): int { return Codes::KEPT; }
}
"#;

#[tokio::test]
async fn a_class_constant_nothing_reads_is_reported_and_a_read_one_is_not() {
    // The report has counted constants since it existed and the diagnostic skipped them, so the
    // two disagreed about the same project -- the same way they disagreed about a Drush command
    // and a container factory, one gap at a time. A constant is reached exactly as a method is:
    // by an access that resolves to the class that declares it.
    let mut fixture = Fixture::new(CONSTANTS);
    fixture.initialize(false, false).await;
    fixture.initialized().await;

    expect![[r#"
        [
            "Codes.php:8: unused-declaration constant Codes::DEAD is reached from nowhere in this project",
            "Reader.php:4: unused-declaration class Reader is reached from nowhere in this project",
        ]
    "#]]
    .assert_debug_eq(&reported(&fixture));
}

/// One interface method a call keeps alive, one nothing calls, and a namesake that is neither.
const A_CONTRACT_AND_A_NAMESAKE: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "src/"}}}
//- /phrost.toml
[lint]
on = ["unused-declaration"]
//- /src/AttachmentType.php
<?php
namespace App;

interface AttachmentType
{
    public function getFields(): array;

    public function buildJsonSchema(): array;
}
//- /src/PdfAttachmentType.php
<?php
namespace App;

class PdfAttachmentType implements AttachmentType
{
    public function getFields(): array { return []; }

    public function buildJsonSchema(): array { return []; }
}
//- /src/Processor.php
<?php
namespace App;

class Processor
{
    public function run(PdfAttachmentType $type): array
    {
        return $this->buildJsonSchema($type->getFields());
    }

    protected function buildJsonSchema(array $fields): array
    {
        return $fields;
    }
}
"#;

#[tokio::test]
async fn an_interface_method_is_kept_alive_by_a_call_on_any_class_that_implements_it() {
    // Both directions of the contract rule in one fixture, and the reason they need each other.
    //
    // `getFields()` is called on a `PdfAttachmentType` and on nothing else, and the *interface's*
    // declaration is what has to survive it -- an access is a claim on every declaration in the
    // receiver's hierarchy that it satisfies, which is what stops a project whose callers are
    // typed as the implementation from having every interface it writes reported as dead. The
    // receiver being the concrete class is the whole fixture; typed as the interface it would
    // prove the other direction and read as proving this one.
    //
    // `buildJsonSchema()` is the shape that brought this here from a real project. Nothing calls
    // the contract; two declarations fulfil it; and a third class has a `protected` namesake with
    // a different signature that it really does call. So the word is everywhere, the interface is
    // dead, and the finding has to say both or it reads as a rule that forgot about interfaces.
    // The implementation is not reported beside it because it comes from above -- which is exactly
    // why the message has to speak for it.
    let mut fixture = Fixture::new(A_CONTRACT_AND_A_NAMESAKE);
    fixture.initialize(false, false).await;
    fixture.initialized().await;

    fixture.open("src/AttachmentType.php").await;
    fixture.open("src/PdfAttachmentType.php").await;
    fixture.open("src/Processor.php").await;

    expect![[r#"
        [
            "AttachmentType.php:8: unused-declaration method AttachmentType::buildJsonSchema is reached from nowhere in this project, nor is any implementation of it",
            "PdfAttachmentType.php: none",
            "Processor.php:4: unused-declaration class Processor is reached from nowhere in this project",
        ]
    "#]].assert_debug_eq(&reported(&fixture));
}

/// English prose and a real `include`, in two files that are otherwise the same.
///
/// The rule that silences a whole file -- one member named at runtime and none of its neighbours
/// can be judged -- was a plain substring search, so `required dependencies` in a docblock, a
/// `$required` local and a `getRequired()` call each read as an `include`. One of those is in most
/// files somebody wrote prose for: on the reference project it silenced **97 of 196** findings,
/// half the check, in the direction nothing reports.
const PROSE_AND_A_CONSTRUCT: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "src/"}}}
//- /phrost.toml
[lint]
on = ["unused-declaration"]
//- /src/Quiet.php
<?php
namespace App;

/**
 * Builds a report from the required dependencies.
 */
class Quiet
{
    public function run(array $required): array
    {
        $required = $this->settings();
        return $required;
    }

    private function settings(): array { return []; }

    public function neverReached(): int { return 1; }
}
//- /src/Loud.php
<?php
namespace App;

class Loud
{
    public function render(): void
    {
        include __DIR__ . '/template.php';
    }

    public function calledByTheTemplate(): int { return 2; }
}
//- /src/Entry.php
<?php
namespace App;

class Entry
{
    public function go(Quiet $quiet, Loud $loud): void
    {
        $quiet->run([]);
        $loud->render();
    }
}
"#;

#[tokio::test]
async fn prose_about_a_requirement_does_not_silence_a_file_and_a_real_include_does() {
    // Both halves in one fixture, because the rule is only right if it still stops where PHP
    // really does reach a member the file does not name: a template pulled into a method runs in
    // that method's scope and can call anything on `$this`.
    let mut fixture = Fixture::new(PROSE_AND_A_CONSTRUCT);
    fixture.initialize(false, false).await;
    fixture.initialized().await;

    fixture.open("src/Quiet.php").await;
    fixture.open("src/Loud.php").await;

    expect![[r#"
        [
            "Entry.php:4: unused-declaration class Entry is reached from nowhere in this project",
            "Loud.php: none",
            "Quiet.php:17: unused-declaration method Quiet::neverReached is reached from nowhere in this project",
        ]
    "#]]
    .assert_debug_eq(&reported(&fixture));
}

/// A contract an interface states, and a trait whose code calls it through `$this`.
const CALLED_FROM_A_TRAIT: &str = r#"
//- /composer.json
{"type": "project", "autoload": {"psr-4": {"App\\": "src/"}}}
//- /phrost.toml
[lint]
on = ["unused-declaration"]
//- /src/Provider.php
<?php
namespace App;

interface Provider
{
    public function productTypes(): array;
}
//- /src/ProviderTrait.php
<?php
namespace App;

trait ProviderTrait
{
    abstract public function productTypes(): array;

    public function applies(string $type): bool
    {
        return $this->productTypes() !== [];
    }
}
//- /src/OrderProvider.php
<?php
namespace App;

final class OrderProvider implements Provider
{
    use ProviderTrait;

    public function productTypes(): array { return ['order']; }
}
//- /src/Caller.php
<?php
namespace App;

final class Caller
{
    public function run(OrderProvider $provider): bool { return $provider->applies('order'); }
}
"#;

#[tokio::test]
async fn a_member_a_trait_calls_through_this_is_reached_on_the_class_using_it() {
    // A trait's code runs in the class that uses it, so `$this->productTypes()` inside one --
    // resolved to the trait's own `abstract` declaration -- calls `OrderProvider::productTypes()`
    // and satisfies `Provider`, which that class implements. Recorded on the trait alone, the
    // interface's method read as a contract nothing calls, on the reference project's document
    // requirement providers.
    let mut fixture = Fixture::new(CALLED_FROM_A_TRAIT);
    fixture.initialize(false, false).await;
    fixture.initialized().await;
    fixture.open("src/Provider.php").await;
    fixture.open("src/OrderProvider.php").await;

    expect![[r#"
        [
            "Caller.php:4: unused-declaration class Caller is reached from nowhere in this project",
            "OrderProvider.php: none",
            "Provider.php: none",
        ]
    "#]]
    .assert_debug_eq(&reported(&fixture));
}
