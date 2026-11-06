//! `service-locator-call`: a static call into the container from a class the container builds.
//!
//! Only where the front door exists -- a class built through `create()` or named by a service
//! definition -- and only from an instance method, since a static method has no instance to have
//! been handed anything and the factory itself reads the container by design. A trait is left out:
//! it is written for every class that uses it, built or not.

use expect_test::expect;
use phrost_harness::World;

use super::diagnostics::check_asked_for_with;

const ASKED: &[&str] = &["service-locator-call"];

/// The framework's half, as the Drupal provider states it.
fn container(world: &mut World) {
    let contracts = world.contracts_mut();
    contracts.add_container_interface("Core\\ContainerInjectionInterface");
    contracts.add_container_catalog("service");
    contracts.add_container_reach("Drupal", false, &[], "the service it asks for");
    contracts.add_container_reach(
        "Core\\EntityInterface",
        true,
        &["load", "loadMultiple"],
        "the entity type manager",
    );
    contracts.catalog_entry("service", "app.mailer", Some("App\\Mailer"));
}

#[test]
fn a_built_class_fetching_from_the_container_is_reported() {
    check_asked_for_with(
        r#"
//- /core/Drupal.php
<?php

class Drupal
{
    public static function service(string $id): object {}
    public static function time(): object {}
}

//- /core/Core.php
<?php
namespace Core;

interface ContainerInjectionInterface {}
interface EntityInterface {}

class File implements EntityInterface
{
    public static function load(int $id): ?static {}
    public static function create(array $values): static {}
}

//- /src/App.php
<?php
namespace App;

use Core\ContainerInjectionInterface;
use Core\File;

class ReviewForm implements ContainerInjectionInterface
{
    public static function create(): static
    {
        \Drupal::service('app.mailer');
        return new static();
    }

    public function submit(): void
    {
        \Drupal::service('app.mailer');
        File::load(1);
        File::create([]);
    }
}

class Mailer
{
    public function send(): void
    {
        \Drupal::time();
    }
}

class Report
{
    public function render(): void
    {
        \Drupal::service('app.mailer');
        File::load(2);
    }
}

trait LazyMailer
{
    protected function mailer(): object
    {
        return \Drupal::service('app.mailer');
    }
}
"#,
        ASKED,
        container,
        expect![[r#"
            17:9-17:39 warning [service-locator-call] `Drupal::service()` fetches from the container inside `ReviewForm`, which the container builds -- inject the service it asks for instead
            18:9-18:22 warning [service-locator-call] `File::load()` fetches from the container inside `ReviewForm`, which the container builds -- inject the entity type manager instead
            27:9-27:24 warning [service-locator-call] `Drupal::time()` fetches from the container inside `Mailer`, which the container builds -- inject the service it asks for instead
        "#]],
    );
}
