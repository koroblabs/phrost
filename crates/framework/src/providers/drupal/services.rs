//! Drupal's container: which ids exist, and how code asks for one.
//!
//! Two facts, and the second is why the first is worth reading. `\Drupal::service()` is declared
//! `object` and `ContainerInterface::get()` `?object`, so the id a caller writes is the only thing in
//! the source that says what comes back -- and on a real project that one gap is the root of the
//! largest cluster of untyped values there is.

use std::path::Path;

use crate::{
    rule::{
        ArgumentBinding, ArgumentClassReturn, CatalogEntry, DefaultedReturn, NonNullReturn,
        NullFact,
    },
    services::{ServiceDefinition, resolve_aliases, services_in},
};

/// The name of the catalog these entries go into.
///
/// A word this provider chose, not a concept the layers below know. They hold named tables and look
/// keys up in them; that a table is called `service` means nothing to them.
pub(super) const CATALOG: &str = "service";

/// The name of the catalog the container's *parameters* go into.
///
/// A second catalog rather than a second kind of key in the first, because the two are looked up by
/// different methods and a service id is not a parameter name: `get('serializer.formats')` is not a
/// service and reporting it as a near miss of one would be wrong in both directions.
pub(super) const PARAMETERS: &str = "parameter";

/// The two ways Drupal code reaches into the container.
///
/// `Drupal` has no namespace -- it is deliberately global, so that `\Drupal::service()` is short --
/// and the container is Symfony's interface, which is what every `create()` in the project types its
/// parameter as.
///
/// `get()` declares `?object`, and the null is real only for the non-throwing invalid-behaviour
/// argument nobody passes; the default throws. So a resolved id is a class and not a `?class`, which
/// is what the catalog gives.
pub(super) static BINDINGS: &[ArgumentBinding] = &[
    ArgumentBinding {
        class: "Symfony\\Component\\DependencyInjection\\ContainerInterface",
        method: "getParameter",
        at: 0,
        suffix: "",
        catalog: PARAMETERS,
    },
    ArgumentBinding {
        class: "Drupal",
        method: "service",
        at: 0,
        suffix: "",
        catalog: CATALOG,
    },
    ArgumentBinding {
        class: "Symfony\\Component\\DependencyInjection\\ContainerInterface",
        method: "get",
        at: 0,
        suffix: "",
        catalog: CATALOG,
    },
];

/// The container's own accessor, which is two methods wearing one signature.
///
/// `\Drupal::classResolver($class = NULL)` is declared
/// `\Drupal\Core\DependencyInjection\ClassResolverInterface|object`, and its docblock says which
/// branch is which: "The class resolver **or if `$class` is provided**, a class instance with a given
/// class definition." So a call that provides nothing gets the resolver, and the union that has to
/// cover both cases leaves `\Drupal::classResolver()->getInstanceFromDefinition(...)` -- the spelling
/// every procedural Drupal file uses -- with no type at all.
///
/// The method's own body settles it, which is the same standard of evidence the non-null returns in
/// [`super::database`] are held to:
///
/// ```php
/// if ($class) {
///   return static::getContainer()->get('class_resolver')->getInstanceFromDefinition($class);
/// }
/// return static::getContainer()->get('class_resolver');
/// ```
///
/// The guard is truthiness, so `classResolver(NULL)` takes the resolver branch too -- this fires only
/// on a call that writes no argument at all, which is the narrower of the two and always right.
/// `class_resolver` is registered in `core/core.services.yml` as
/// `Drupal\Core\DependencyInjection\ClassResolver`, which implements the interface named here.
///
/// And the same rule is what Drupal's own static-analysis extension applies: phpstan-drupal's
/// `DrupalClassResolverDynamicStaticReturnTypeExtension` is `if (0 === count($args)) return new
/// ObjectType(ClassResolverInterface::class);` and nothing else.
///
/// Measured on one real project: 15 member accesses with no receiver type, all of them the bare call.
pub(super) static DEFAULTED_RETURNS: &[DefaultedReturn] = &[DefaultedReturn {
    class: "Drupal",
    method: "classResolver",
    unless_given: 0,
    then: "Drupal\\Core\\DependencyInjection\\ClassResolverInterface",
}];

/// The other half of the class resolver: what it hands back once it has been asked for something.
///
/// `ClassResolverInterface::getInstanceFromDefinition($definition)` is declared `@return object` and
/// documented as "Returns a class instance with a given class definition". Its body is the whole
/// evidence, the same standard the contracts above are held to:
///
/// ```php
/// if ($this->container->has($definition)) { $instance = $this->container->get($definition); }
/// else {
///   if (!class_exists($definition)) { throw new \InvalidArgumentException(...); }
///   $instance = is_subclass_of($definition, ContainerInjectionInterface::class)
///     ? $definition::create($this->container)
///     : new $definition();
/// }
/// ```
///
/// So a `$definition` naming a class produces an instance of that class or nothing at all -- the
/// `class_exists` guard throws rather than returning something else. phpstan-drupal's
/// `DrupalClassResolverReturnType` reads the same argument for the same reason.
///
/// The parameter is documented as "a class name **or a service name**", and the service reading is
/// deliberately left out. It could be had for nothing by binding this method to [`CATALOG`] the way
/// `\Drupal::service()` is -- and that would be wrong: the container catalog is closed, so binding
/// it here would make every `getInstanceFromDefinition(Foo::class)` in the project a service id
/// nothing registers, and say so. The two readings do not otherwise collide: a service id is not a
/// name PHP declares, so it resolves to no class and this stays silent, and where a container
/// registers a service under its own class name both readings are the same class anyway.
///
/// Measured on one real project: 12 call sites, all of them writing `Foo::class`.
pub(super) static ARGUMENT_CLASS_RETURNS: &[ArgumentClassReturn] = &[
    ArgumentClassReturn {
        class: "Drupal\\Core\\DependencyInjection\\ClassResolverInterface",
        method: "getInstanceFromDefinition",
        argument: 0,
    },
    // `$container->get(ReverseContainer::class)` -- the container's own way of asking for a
    // service registered under its class name, and how every `create()` in Drupal that wants an
    // autowired service is written. 296 sites on one real checkout, two of them in the project's
    // own modules and the rest in the core and contrib it reads all day.
    //
    // The declaration says it in a docblock and says it in a shape nothing here can read:
    // `@return ($id is class-string<C> ? (B is 0|1 ? C|object : C|object|null) : ...)`, a
    // conditional keyed on a *type test* over a template and nested two deep -- and even read
    // perfectly its branch is `C|object`, so a member lookup would have to find the method on
    // `object` too and decline. The contract states the same fact in the form this index holds.
    //
    // It does not collide with the catalog binding on the same method above. A service id is not
    // a name PHP declares, so it resolves to no class and this stays silent; a class name is not
    // a service id, so the catalog stays silent; and where a container registers a service under
    // its own class name -- which is the whole point of this spelling -- both say the same class.
    ArgumentClassReturn {
        class: "Symfony\\Component\\DependencyInjection\\ContainerInterface",
        method: "get",
        argument: 0,
    },
];

/// Every service id the installed code registers, and the class it names.
///
/// Read from the files Drupal itself loads and no others: `core/core.services.yml`, and one
/// `<name>.services.yml` per extension. That is `DrupalKernel`'s own rule, so the catalog holds what
/// the container would hold -- and it costs no walk of its own, because the extensions are the ones
/// already found for their namespaces.
///
/// Deliberately not read: `drush.services.yml`, which Drush loads into a container of its own, and
/// `sites/*/services.yml`, which arrives through a `container_yamls` setting this does not parse.
/// Their ids are not answerable, and answering with a class from a container the code cannot reach
/// would be worse than saying nothing.
pub(super) fn catalog_entries(
    docroot: &Path,
    extensions: &[std::path::PathBuf],
) -> Vec<CatalogEntry> {
    let mut definitions = Vec::new();
    // Which file each id was last defined in, which is the one that wins.
    let mut declared_in = std::collections::HashMap::<String, std::path::PathBuf>::new();

    // Core first, so that a module redefining a core id wins -- which is the order the container is
    // built in, and the reason `catalog_entry` lets a later definition replace an earlier one.
    let mut files = vec![docroot.join("core/core.services.yml")];
    for info in extensions {
        let Some(directory) = info.parent() else {
            continue;
        };
        let Some(name) = super::extensions::machine_name(info) else {
            continue;
        };
        files.push(directory.join(format!("{name}.services.yml")));
    }
    for file in files {
        let before = definitions.len();
        read_into(&mut definitions, &file);
        for definition in &definitions[before..] {
            declared_in.insert(definition.id.clone(), file.clone());
        }
    }

    let mut found = resolve_aliases(definitions)
        .into_iter()
        .map(|(key, class)| CatalogEntry {
            catalog: CATALOG,
            declared_in: declared_in.get(&key).cloned(),
            key,
            names: class.map(crate::CatalogName::Class),
        })
        .collect::<Vec<_>>();
    found.extend(parameter_entries(docroot, extensions));
    found
}

/// Every container parameter the same files declare, with the shape of its value.
///
/// The reason this is worth reading at all: `getParameter()` declares
/// `array|bool|string|int|float|\UnitEnum|null`, because a signature cannot say which kind *this*
/// parameter holds -- so `getParameter('serializer.formats')` handed to an `array $formats` reads as
/// a value that may be null and may be a bool. The YAML says it is a list. Sixteen parameters are
/// declared across the reference checkout's core and modules.
fn parameter_entries(docroot: &Path, extensions: &[std::path::PathBuf]) -> Vec<CatalogEntry> {
    let mut files = vec![docroot.join("core/core.services.yml")];
    for info in extensions {
        let Some(directory) = info.parent() else {
            continue;
        };
        let Some(name) = super::extensions::machine_name(info) else {
            continue;
        };
        files.push(directory.join(format!("{name}.services.yml")));
    }

    let mut found = Vec::new();
    for path in files {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        found.extend(
            crate::services::parameters_in(&text)
                .into_iter()
                .map(|parameter| CatalogEntry {
                    catalog: PARAMETERS,
                    key: parameter.name,
                    names: Some(crate::CatalogName::Shape(parameter.shape)),
                    declared_in: Some(path.clone()),
                }),
        );
    }
    found
}

/// Reads one file, if it is there.
///
/// Bytes rather than a string: a service file is machine-written and ASCII, but a hand-edited one can
/// carry a stray byte in a comment, and `read_to_string` would fail on it and drop every service the
/// file declares. That exact failure once cost Drupal's whole classmap.
fn read_into(into: &mut Vec<ServiceDefinition>, path: &Path) {
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    into.extend(services_in(&String::from_utf8_lossy(&bytes)));
}

/// The request stack, whose `?Request` a Drupal application always has one of.
///
/// Symfony's class and Drupal's guarantee, which is why it is named here rather than being a fact
/// about Symfony: `RequestStack::getCurrentRequest()` is null for an empty stack, and Drupal pushes
/// a request before any of a project's own code runs -- `DrupalKernel::handle()` for a web request,
/// and drush pushes one for a command. A project reaching the stack from outside both is reaching
/// it from nowhere.
///
/// Beside the container in this file because that is what it is: a service the container hands out,
/// and the same shape of gap as `ContainerInterface::get()` returning `?object`.
pub(super) static NON_NULL_RETURNS: &[NonNullReturn] = &[
    NonNullReturn {
        class: "Symfony\\Component\\HttpFoundation\\RequestStack",
        method: "getCurrentRequest",
        null: NullFact::OnlyWhen(
            "nothing pushed a request, which is outside a kernel or a command",
        ),
    },
    // The container itself, for the argument the catalog cannot answer. `get('current_user')` is a
    // catalog key and comes back as the class the id names; `get(OneTimeAuthentication::class)` is
    // a class-string, which the argument-class rule reads -- and that composes with the
    // declaration's own `?object`, so the null went back on. It is the same null the file header
    // describes: real only for the non-throwing invalid-behaviour argument, and the default throws.
    //
    // What this knowingly misses is `get($id, ContainerInterface::NULL_ON_INVALID_REFERENCE)`,
    // which really can be null. Measured on the reference project: **zero** such calls in its own
    // code and 8 in the whole of Drupal core, none of them user code. A rule that removes a null
    // can only ever miss one, which is what makes the trade allowed.
    NonNullReturn {
        class: "Symfony\\Component\\DependencyInjection\\ContainerInterface",
        method: "get",
        null: NullFact::OnlyWithArgument {
            at: 1,
            prose: "the call passes NULL_ON_INVALID_REFERENCE, and the default throws",
        },
    },
    // `getParameter()` is deliberately *not* here, and the measurement is why. Its declaration is
    // `array|bool|string|int|float|\UnitEnum|null`, and taking only the null off leaves a union
    // that `array $formats` still refuses -- so a `possibly-null-argument` warning became an
    // `incompatible-argument` error on the same two lines, which is not silence and reads worse.
    // What it needs is the *type*, which means the container's own parameter list: the provider
    // already reads `*.services.yml` for services, and a `parameters:` value says its own shape
    // (`serializer.formats: [json, xml]` is a sequence). That is a feature, not a null to remove.
];

#[cfg(test)]
mod tests {
    use phrost_testkit::MaterializedFixture;

    use crate::{providers::drupal::Drupal, registry::FrameworkProvider};

    /// The container's own ids, of everything the provider contributes.
    ///
    /// Filtered by catalog, because the seam takes one list and more than one module fills it: an
    /// unfiltered reading of it makes this test fail whenever a neighbour learns to contribute
    /// something, which says nothing about the container.
    fn entries(fixture: &MaterializedFixture) -> Vec<String> {
        let mut found = Drupal
            .project(fixture.root())
            .catalog_entries()
            .into_iter()
            .filter(|entry| entry.catalog == super::CATALOG)
            .map(|entry| {
                format!(
                    "{} => {}",
                    entry.key,
                    match &entry.names {
                        Some(crate::CatalogName::Class(class)) => class.as_str(),
                        Some(crate::CatalogName::Shape(shape)) => shape.as_str(),
                        None => "(no class declared)",
                    }
                )
            })
            .collect::<Vec<_>>();
        found.sort();
        found
    }

    /// The files Drupal loads, and only those.
    #[test]
    fn core_and_every_extension_contribute_their_own_file() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /docroot/core/core.services.yml
services:
  messenger:
    class: Drupal\Core\Messenger\Messenger

//- /docroot/core/modules/node/node.info.yml
name: Node

//- /docroot/core/modules/node/node.services.yml
services:
  node.grant_storage:
    class: Drupal\node\NodeGrantDatabaseStorage

//- /docroot/modules/custom/mine/mine.info.yml
name: Mine

//- /docroot/modules/custom/mine/mine.services.yml
services:
  mine.manager:
    class: Drupal\mine\Manager

//- /docroot/modules/custom/mine/drush.services.yml
services:
  mine.commands:
    class: Drupal\mine\Commands
"#,
        );

        assert_eq!(
            entries(&fixture),
            [
                "messenger => Drupal\\Core\\Messenger\\Messenger",
                "mine.manager => Drupal\\mine\\Manager",
                "node.grant_storage => Drupal\\node\\NodeGrantDatabaseStorage",
            ],
            "drush's own container is not Drupal's, so its ids are not answerable"
        );
    }

    /// A module may redefine a core id, and the container the code runs against has the module's.
    #[test]
    fn a_module_that_redefines_a_core_id_is_read_after_core() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /docroot/core/core.services.yml
services:
  messenger:
    class: Drupal\Core\Messenger\Messenger

//- /docroot/modules/custom/mine/mine.info.yml
name: Mine

//- /docroot/modules/custom/mine/mine.services.yml
services:
  messenger:
    class: Drupal\mine\LoudMessenger
"#,
        );

        let found = Drupal.project(fixture.root()).catalog_entries();
        let messenger = found
            .iter()
            .filter(|entry| entry.key == "messenger")
            .collect::<Vec<_>>();
        assert_eq!(messenger.len(), 2, "both are emitted, in load order");
        assert_eq!(
            match &messenger.last().expect("two entries").names {
                Some(crate::CatalogName::Class(class)) => Some(class.as_str()),
                _ => None,
            },
            Some("Drupal\\mine\\LoudMessenger"),
            "the module's definition is emitted last, so it is the one that lands"
        );
    }

    #[test]
    fn a_project_that_is_not_drupal_contributes_nothing() {
        let fixture = MaterializedFixture::parse("//- /index.php\n<?php");
        assert_eq!(Drupal.project(fixture.root()).catalog_entries(), Vec::new());
    }
}
