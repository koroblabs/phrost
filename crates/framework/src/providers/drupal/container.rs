//! What Drupal's container builds, and the static calls that fetch from it.
//!
//! A class the container constructs is handed its services through `create()` or a service
//! definition, so a `\Drupal::service()` or a `File::load()` inside it is a dependency taken from
//! the side door: invisible in the constructor, and impossible to replace in a test. Reported only
//! where the front door exists -- everywhere else the static call is how Drupal code reaches the
//! container at all.
//!
//! An entity's static loaders are on the list because they are the same call wearing a class's
//! name: `File::load()` is `\Drupal::entityTypeManager()->getStorage('file')->load()`. `create()`
//! is not: a new entity is a value, and building one is not fetching a dependency.

use crate::rule::{ContainerReach, ContainerRule};

pub(super) static CONTAINER: ContainerRule = ContainerRule {
    built_through: &[
        // Controllers and forms: `create(ContainerInterface $container)`.
        "Drupal\\Core\\DependencyInjection\\ContainerInjectionInterface",
        // Blocks, field widgets and formatters, and every other plugin with a container factory.
        "Drupal\\Core\\Plugin\\ContainerFactoryPluginInterface",
        // List builders, access handlers, view builders: `createInstance($container, $type)`.
        "Drupal\\Core\\Entity\\EntityHandlerInterface",
    ],
    built_from: &[super::services::CATALOG],
    reaches: &[
        ContainerReach {
            class: "Drupal",
            descendants: false,
            methods: &[],
            instead: "the service it asks for",
        },
        ContainerReach {
            class: "Drupal\\Core\\Entity\\EntityInterface",
            descendants: true,
            methods: &["load", "loadMultiple"],
            instead: "the entity type manager, and load through its storage",
        },
        // A config entity's own static finder, which `FieldStorageConfig` and `FieldConfig` declare
        // over the same storage.
        ContainerReach {
            class: "Drupal\\Core\\Config\\Entity\\ConfigEntityInterface",
            descendants: true,
            methods: &["loadByName"],
            instead: "the entity type manager, and load through its storage",
        },
    ],
};
