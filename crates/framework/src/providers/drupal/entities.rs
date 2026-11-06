//! Which entity a storage handler loads.
//!
//! The one Drupal idiom that no signature can describe. `getStorage('node')` returns an
//! `EntityStorageInterface`, and `load()` on it returns an `EntityInterface` -- both true, both
//! useless, because the answer a person wants is `Node`. The class in between cannot carry it
//! either: `SqlContentEntityStorage` is the storage for most content entity types at once, so there
//! is no per-entity class to hang a return type on. Only the id says which, and it is written one
//! call earlier.

use super::places;
use crate::rule::{
    AttributeArgument, AttributeKey, DerivedKey, Forwarded, ForwardedArgument, NonNullReturn,
    NullFact,
};

/// Where an entity type id is a key. Filled from the entity declarations in the project.
///
/// # Why this catalog is closed
///
/// Because both halves of what it takes are now there, and each was measured on one real Drupal 10
/// checkout before being built -- see
/// [`FrameworkProvider::closed_catalogs`](crate::FrameworkProvider::closed_catalogs) for what a
/// wrong entry costs.
///
/// **The key set is complete: 111 of 111.** Drupal moved entity types from docblock annotations to
/// attributes in 10.2 and contrib lags by years, so a checkout holds both spellings at once. That
/// project declares 107 ids as `#[ContentEntityType]` / `#[ConfigEntityType]` and 11 as
/// `@ContentEntityType` / `@ConfigEntityType`, overlapping in 7 -- modules that carry an attribute
/// and a legacy annotation together. Reading only the attribute left four ids -- `behavior_settings`,
/// `ctools_wizard_test_config_entity`, `fake_block_content`, `paragraphs_library_item` -- in no
/// catalog, and closing it then would have called each of them undeclared.
///
/// Both spellings are read in lowering, off the docblock mago has already parsed, and arrive as one
/// kind of fact. Not here: deriving a class's name from its file would mean parsing PHP in this
/// crate, and a bootstrap scan is not the per-file claim that an edit can withdraw.
///
/// **And something can now report it.** A catalog is reachable from a report through an argument
/// binding, an array key, or -- since this one needs it -- the call named by [`FORWARDED`], where
/// the id is written one call earlier than the method that uses it. `getStorage('node')` is that
/// call, and its own signature is what makes reading it safe: the report only fires where the call
/// hands back the `EntityStorageInterface` these rules are written on.
///
/// Measured after both: `getStorage` is written with a literal at 57 sites in the project's own
/// code, over 7 distinct ids, and the catalog holds 7 of 7.
///
/// # Two shortcuts that are not shortcuts
///
/// Neither of these is a way to skip the above, and both look like one line.
///
/// Binding `getDefinition` to this catalog: 133 of its 165 literals on that checkout are service and
/// plugin ids, because it is declared on every plugin manager and on the container. Textbook
/// "sometimes a key", and against a closed catalog that is an error on correct code.
///
/// Adding an [`ArgumentBinding`](crate::rule::ArgumentBinding) for `getStorage` to make it
/// reportable: `returned_from_argument` would then claim `getStorage('node')` *returns* a `Node`
/// rather than a storage handler, breaking every `$storage->` chain in the project.
pub(super) const CATALOG: &str = "entity";

/// Storage methods that return the entity type the storage was asked for.
///
/// Declared on `EntityStorageInterface`, which is what `getStorage()` promises, so every storage
/// class below it is covered without naming one.
///
/// Both shapes, because both are the same question asked of one or of many. `load()` gives one and
/// may give null; `loadMultiple()` and `loadByProperties()` give an array of them and never null,
/// which is why the rule carries [`Forwarded`] rather than reading the shape off the declaration --
/// `EntityInterface` and `EntityInterface[]` are equally silent about *which* entity, so there is
/// nothing there to read.
pub(super) static FORWARDED: &[ForwardedArgument] = &[
    ForwardedArgument {
        class: "Drupal\\Core\\Entity\\EntityStorageInterface",
        method: "load",
        through: "getStorage",
        catalog: CATALOG,
        yields: Forwarded::Instance,
    },
    // `loadUnchanged` is `load` with the persistent cache skipped, and returns the same thing.
    ForwardedArgument {
        class: "Drupal\\Core\\Entity\\EntityStorageInterface",
        method: "loadUnchanged",
        through: "getStorage",
        catalog: CATALOG,
        yields: Forwarded::Instance,
    },
    // A new, unsaved one of the same class -- and the shape differs from `load`: `create()` cannot
    // return null, which the declaration already says, so keeping the declaration's shape is enough
    // to get both right from one rule.
    ForwardedArgument {
        class: "Drupal\\Core\\Entity\\EntityStorageInterface",
        method: "create",
        through: "getStorage",
        catalog: CATALOG,
        yields: Forwarded::Instance,
    },
    // The array-returning half. `loadMultiple()` with no argument is every entity of the type, and
    // `loadByProperties()` is a filtered set; both are keyed by entity id and both declare only
    // `EntityInterface[]`, which is what made `reset($storage->loadByProperties(..))` an
    // `EntityInterface` where the caller had asked for a `UserInterface`.
    ForwardedArgument {
        class: "Drupal\\Core\\Entity\\EntityStorageInterface",
        method: "loadMultiple",
        through: "getStorage",
        catalog: CATALOG,
        yields: Forwarded::ManyOf,
    },
    ForwardedArgument {
        class: "Drupal\\Core\\Entity\\EntityStorageInterface",
        method: "loadByProperties",
        through: "getStorage",
        catalog: CATALOG,
        yields: Forwarded::ManyOf,
    },
];

/// The attributes that declare an entity type, and the argument holding its id.
///
/// Both of Drupal's kinds, because both are storable and both are reached the same way:
/// `getStorage('node')` and `getStorage('node_type')` differ only in that one is content and one is
/// configuration. Their attributes are `Drupal\Core\Entity\Attribute\ContentEntityType` and
/// `...\ConfigEntityType`, written by their short names everywhere in real code.
///
/// The revision and translation variants are not separate attributes -- they are arguments of these
/// two -- so this is the whole list rather than a sample of it.
///
/// Two names and not four, because naming one here reaches both of its spellings: `id = "node"` in
/// a `@ContentEntityType` docblock annotation is the same declaration as `id: 'node'` in the
/// attribute, and lowering reads the older one for exactly the names this list holds. Which is what
/// makes the key set complete -- see [`CATALOG`].
pub(super) static ATTRIBUTE_KEYS: &[AttributeKey] = &[
    AttributeKey {
        attribute: "ContentEntityType",
        argument: AttributeArgument::Named("id"),
        catalog: CATALOG,
    },
    AttributeKey {
        attribute: "ConfigEntityType",
        argument: AttributeArgument::Named("id"),
        catalog: CATALOG,
    },
];

/// The routes an entity type's link templates generate, which nothing writes down.
///
/// Drupal makes one route per entry of `links`, and the name is mechanical -- `EntityBase::toUrl()`
/// spells it `"entity.{$this->entityTypeId}." . str_replace(['-', 'drupal:'], ['_', ''], $rel)`, so
/// `'edit-form' => '/node/{node}/edit'` on the `node` entity type is the route
/// `entity.node.edit_form`. The rewrite list below is that `str_replace`, in that order.
///
/// These are 277 route names on the reference project against 1 052 the YAML declares, and they are
/// the ones its own code writes most: `entity.node.canonical` alone is 42 call sites. Every one of
/// them was a key the route catalog did not hold, which meant no hover, and -- because the near-miss
/// report is deliberately quiet about a key that simply is not there -- no report for a typo of one
/// either. That silence is what this closes.
///
/// Both spellings for [`ATTRIBUTE_KEYS`]'s reason: a checkout mid-migration carries the attribute and
/// the older annotation at once, and reading one of them leaves the other's entity types with no
/// routes. Fifty-six of the fifty-nine that declare links are attributes here and eight are
/// annotations, which is exactly the ratio that made reading only one look like it worked.
///
/// A `create` link template is in the list too, and is one of the places this can be wrong: what a
/// route provider actually registers is up to the entity type's handlers, so a link template with no
/// provider behind it derives a route Drupal does not have. That direction is a **missed** report
/// rather than a false one -- the key resolves where it should have been unknown -- which is the
/// same trade every catalog here makes, and the opposite of what reading the id out of a doc example
/// would have cost.
pub(super) static DERIVED_KEYS: &[DerivedKey] = &[
    DerivedKey {
        attribute: "ContentEntityType",
        keys_of: "links",
        named_after: "id",
        spelling: "entity.{name}.{key}",
        rewrite: &[("drupal:", ""), ("-", "_")],
        catalog: places::ROUTES,
    },
    DerivedKey {
        attribute: "ConfigEntityType",
        keys_of: "links",
        named_after: "id",
        spelling: "entity.{name}.{key}",
        rewrite: &[("drupal:", ""), ("-", "_")],
        catalog: places::ROUTES,
    },
];

/// Entity accessors declared nullable whose null a *loaded* entity cannot hold.
///
/// The same rule [`database`](super::database) states, read from core the same way, and the reason
/// to be careful is the same: a wrong entry here silences a real null rather than a noisy one. So
/// each of these is null in exactly one state, and that state is one an entity read out of storage
/// or handed over by a route is not in.
///
/// - `EntityInterface::uuid()` is `?string`. `ContentEntityBase::uuid()` reads the `uuid` entity
///   key, whose base field definition supplies a value at `create()`, and `EntityBase::uuid()`
///   returns the property the storage fills before a config entity is ever handed out. So the null
///   belongs to an entity type declaring no `uuid` key at all, which is a test fixture rather than
///   anything a project loads.
/// - `FileInterface::getFileUri()` and `getFilename()` are `?string` because `File` reads them off
///   its own fields, and neither field is marked required -- `File::create([])->getFileUri()` really
///   is null. What makes removing the null worth it anyway is what the null would *be*: a file
///   entity whose whole purpose is the path it points at, pointing nowhere. Every call site on one
///   real project hands the result straight to `hash_file`, `realpath` or `preg_match`, where a null
///   fails on the first run and says so -- against eight reports on every save, which is the cost
///   that gets a checker turned off.
///
/// **`id()` and `label()` are deliberately not here.** `id()` is null for every entity between
/// `create()` and `save()`, which is a state code is routinely in, so removing that null is leniency
/// rather than a fact about the framework -- and the null it would hide is a saved-nothing bug that
/// surfaces far from its cause. `label()` is null for an entity type with no label key, and there
/// are such types in core. Three reports between them, and neither is noise.
pub(super) static NON_NULL_RETURNS: &[NonNullReturn] = &[
    NonNullReturn {
        class: "Drupal\\Core\\Entity\\EntityInterface",
        method: "uuid",
        null: NullFact::OnlyWhen("the entity type declares no uuid key, which is a test fixture"),
    },
    NonNullReturn {
        class: "Drupal\\file\\FileInterface",
        method: "getFileUri",
        null: NullFact::OnlyWhen("the file has no URI yet, as between create() and save()"),
    },
    NonNullReturn {
        class: "Drupal\\file\\FileInterface",
        method: "getFilename",
        null: NullFact::OnlyWhen("the file has no name yet, as between create() and save()"),
    },
];
