//! Which fields an entity has.
//!
//! `$node->get('field_item_amount')` is the commonest string a Drupal project writes that names
//! something no PHP declaration mentions: 229 of them in the one measured here, more than the routes
//! and permissions put together. A field is configuration, so the only statement that it exists is a
//! `field.storage.<entity type>.<field name>.yml` -- and the name is in the file's own name, which is
//! what makes reading them cheap.
//!
//! # What this cannot narrow
//!
//! Every field of every entity type is offered, not the ones this receiver has. Two reasons, and the
//! second is the one that matters: the catalog is keyed by name alone, and a bundle is not knowable
//! from a type anyway. `$node` is a `Node`, not a `Node` of bundle `item`, so even a per-bundle index
//! could not say which fields *this* value has. An offer that is a superset is honest where a report
//! would not be, which is why there is no report here.
//!
//! That holds for [`MAGIC`] too, and it is what makes it safe: a content entity has a `__get`, so
//! every property name on one is already unreportable, and saying what the names in this catalog
//! hold does not make the rest any more knowable than it was.

use std::path::{Path, PathBuf};

use crate::rule::{
    ArgumentBinding, AttributeArgument, CatalogKind, DeclaredTable, MagicName, MagicProperty,
    PlaceEntry, PropertyTable, TabledAccess,
};

/// Where a field name is a key. A word this provider chose.
pub(super) const CATALOG: &str = "field";

/// The two field reads that are written as a property and declared nowhere.
///
/// `$node->field_amount` is `ContentEntityBase::__get()`, and it is the same read as
/// `$node->get('field_amount')` -- the spelling with no argument to bind to. `$item->entity` is the
/// entity a reference field points at, reached through `FieldItemListInterface`'s own `__get`, which
/// forwards to the first item in the list.
///
/// # Why `entity` is only an `EntityInterface`
///
/// Because nothing in the project says otherwise. A configurable reference field records its
/// `target_type` in its storage YAML, but the commonest ones -- `uid`, `revision_uid`, the whole of
/// an entity type's base fields -- are declared in `baseFieldDefinitions()`, which is PHP that has to
/// run. And by the time a list is in hand the field name is gone: `get('uid')` returns a
/// `FieldItemListInterface` that remembers nothing about which field produced it, so there is no key
/// left to look up even for the fields configuration does describe. `EntityInterface` is a superset
/// and every real call site here annotates the concrete class itself; naming `User` for `uid` would
/// be a guess that is wrong wherever a site overrode it.
pub(super) static MAGIC: &[MagicProperty] = &[
    // Declared on `FieldableEntityInterface` for the reason the bindings are: every content entity
    // is below it, and a config entity -- which has no fields and no such accessor -- is not.
    MagicProperty {
        class: "Drupal\\Core\\Entity\\FieldableEntityInterface",
        name: MagicName::KeyOf(CATALOG),
        holds: "Drupal\\Core\\Field\\FieldItemListInterface",
        // An entity always has a list for a field it has. `__get` asks `get()`, which throws for a
        // field the entity does not have rather than handing back nothing.
        nullable: false,
    },
    // The list, whose `__get` is documented as delegating to the first item.
    MagicProperty {
        class: "Drupal\\Core\\Field\\FieldItemListInterface",
        name: MagicName::Exactly("entity"),
        holds: "Drupal\\Core\\Entity\\EntityInterface",
        // An empty field has no first item, and a reference whose target was deleted resolves to
        // nothing. Both are null, and both are what the `if ($file)` after every one of these reads
        // is there for.
        nullable: true,
    },
    // And the item itself, reached by iterating the list. The same property, one level down.
    MagicProperty {
        class: "Drupal\\Core\\Field\\FieldItemInterface",
        name: MagicName::Exactly("entity"),
        holds: "Drupal\\Core\\Entity\\EntityInterface",
        nullable: true,
    },
];

/// What a field's own value is called, and what each name holds.
///
/// The read `MAGIC` above cannot answer: `->value` is a `string` on one field and an `int` on the
/// next, and the only thing separating the two is the name written two links back. So it is a
/// lookback rather than a property with one type -- see [`TabledAccess`].
///
/// Both classes, because a list's `__get` forwards to its first item and code writes it either way:
/// `$node->get('field_when')->value` and `$node->get('field_when')->first()->value` read the same
/// property. Only the list has a `getValue()`, which hands back every delta at once.
pub(super) static TABLED: &[TabledAccess] = &[
    TabledAccess {
        class: "Drupal\\Core\\Field\\FieldItemListInterface",
        through: "get",
        catalog: CATALOG,
        rows: Some("getValue"),
    },
    TabledAccess {
        class: "Drupal\\Core\\Field\\FieldItemInterface",
        through: "get",
        catalog: CATALOG,
        rows: None,
    },
];

/// A field type the project declares itself, whose `propertyDefinitions()` says what it holds.
///
/// The half [`super::field_types`] cannot reach: core's plugin bodies are not lowered and a custom
/// module's are, so what is written out there is core's answer and what is read here is everything
/// the project adds to it. On one real checkout that is seventy-three kinds --
/// `store_sales`, `shipping_info`, `order_term` -- and every one of them the plain shape.
///
/// The names are Drupal's typed-data ids, and the two everybody gets wrong are here for the reason
/// [`super::field_types`] gives: a `decimal` is read back as its digits and a `datetime_iso8601`
/// is the stored spelling, so both are a `string`. A name not listed leaves the property in the
/// table saying nothing, which keeps the key.
pub(super) static DECLARED: &[DeclaredTable] = &[DeclaredTable {
    attribute: "FieldType",
    argument: AttributeArgument::Named("id"),
    method: "propertyDefinitions",
    // `DataReferenceTargetDefinition` is the same shape and the same argument: it is what an
    // entity reference's `target_id` is built with.
    builders: &["DataDefinition", "DataReferenceTargetDefinition"],
    kinds: &[
        ("string", phrost_model::BuiltinTy::String),
        ("email", phrost_model::BuiltinTy::String),
        ("uri", phrost_model::BuiltinTy::String),
        ("decimal", phrost_model::BuiltinTy::String),
        ("datetime_iso8601", phrost_model::BuiltinTy::String),
        ("integer", phrost_model::BuiltinTy::Int),
        ("timestamp", phrost_model::BuiltinTy::Int),
        ("float", phrost_model::BuiltinTy::Float),
        ("boolean", phrost_model::BuiltinTy::Bool),
    ],
    catalog: CATALOG,
}];

/// What kind of field each one the configuration declares is.
///
/// Half the join: `field_customer_type` is a `list_string`, which nothing in PHP says and its own
/// `field.storage.*.yml` does. The other half is [`property_tables`], which says what a
/// `list_string` holds -- keyed by kind, because four hundred fields on a real checkout share
/// thirty of them.
pub(super) fn catalog_kinds(root: &Path, extensions: &[PathBuf]) -> Vec<CatalogKind> {
    let mut found = Vec::new();
    for (field, kind, _) in declared_fields(root, extensions) {
        found.push(CatalogKind {
            catalog: CATALOG,
            key: field,
            kind,
        });
    }
    // A field of one name on two entity types is one key, exactly as `catalog_places` treats it.
    // Sorted so which one wins is the same on every run, and the first kept.
    found.sort_by(|left, right| left.key.cmp(&right.key));
    found.dedup_by(|later, first| later.key == first.key);
    found
}

/// What each kind of field holds, property by property.
///
/// Core's kinds, read out of core -- see [`super::field_types`] for why they are a table here and a
/// project's own are not. Only the kinds this checkout actually uses, because a table for a kind no
/// field is of answers nothing and is still resident.
pub(super) fn property_tables(root: &Path, extensions: &[PathBuf]) -> Vec<PropertyTable> {
    let mut kinds: Vec<String> = declared_fields(root, extensions)
        .into_iter()
        .map(|(_, kind, _)| kind)
        .collect();
    kinds.sort();
    kinds.dedup();

    kinds
        .into_iter()
        .filter_map(|kind| {
            let table = super::field_types::CORE
                .iter()
                .find(|table| table.kind == kind)?;
            Some(PropertyTable {
                catalog: CATALOG,
                kind,
                properties: table
                    .properties
                    .iter()
                    .map(|(name, holds)| ((*name).to_owned(), holds.holds()))
                    .collect(),
                complete: !table.partial,
            })
        })
        .collect()
}

/// Every field the configuration declares: its name, its kind, and the file that says so.
///
/// One walk for the two readers above, because both want the same directories and the same files
/// and walking them twice is the shape §8 records for the four surveys of one tree.
fn declared_fields(root: &Path, extensions: &[PathBuf]) -> Vec<(String, String, PathBuf)> {
    let mut found = Vec::new();
    for directory in config_directories(root, extensions) {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some((_, field)) = storage_name(name) else {
                continue;
            };
            let Some(kind) = kind_of(&path) else {
                continue;
            };
            found.push((field.to_owned(), kind, path));
        }
    }
    found
}

/// The `type:` a field storage file states.
fn kind_of(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    crate::keys::field_at_root(&String::from_utf8_lossy(&bytes), "type")
}

/// The three methods that take a field name.
///
/// Declared on `FieldableEntityInterface`, so every content entity is covered by one entry each --
/// and a config entity, which has no fields, is not.
pub(super) static BINDINGS: &[ArgumentBinding] = &[
    ArgumentBinding {
        class: "Drupal\\Core\\Entity\\FieldableEntityInterface",
        method: "get",
        at: 0,
        suffix: "",
        catalog: CATALOG,
    },
    ArgumentBinding {
        class: "Drupal\\Core\\Entity\\FieldableEntityInterface",
        method: "set",
        at: 0,
        suffix: "",
        catalog: CATALOG,
    },
    ArgumentBinding {
        class: "Drupal\\Core\\Entity\\FieldableEntityInterface",
        method: "hasField",
        at: 0,
        suffix: "",
        catalog: CATALOG,
    },
];

/// Every field the project's configuration declares.
///
/// From `field.storage.*` and not `field.field.*`: the storage entity is the field itself, one per
/// entity type, while a `field.field.*` is that field attached to one bundle -- so the storage files
/// are the declarations and there are fewer of them. On the measured project, 381 against 601.
pub(super) fn catalog_places(root: &Path, extensions: &[PathBuf]) -> Vec<PlaceEntry> {
    let mut found = Vec::new();

    for directory in config_directories(root, extensions) {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some((entity_type, field)) = storage_name(name) else {
                continue;
            };
            found.push(PlaceEntry {
                catalog: CATALOG,
                key: field.to_owned(),
                detail: describe(&path, entity_type),
                path,
                // The `field_name:` line, found below; the top of the file if it is not there.
                line: 0,
                character: 0,
            });
        }
    }

    // A field of the same name on two entity types is one key here, and which file it points at has
    // to be the same on every run: sorted by path, and the first wins because `catalog_place`
    // overwrites.
    found.sort_by(|left, right| left.key.cmp(&right.key).then(right.path.cmp(&left.path)));
    for entry in &mut found {
        place_of(entry);
    }
    found
}

/// The entity type and field name a storage file's own name states.
fn storage_name(file: &str) -> Option<(&str, &str)> {
    let rest = file.strip_prefix("field.storage.")?.strip_suffix(".yml")?;
    let (entity_type, field) = rest.split_once('.')?;
    (!entity_type.is_empty() && !field.is_empty()).then_some((entity_type, field))
}

/// The field's type and the entity it is on, for the line beside it in a list.
///
/// `string on node` rather than the name alone, because 381 field names with nothing beside them is a
/// list nobody can choose from.
fn describe(path: &Path, entity_type: &str) -> String {
    match kind_of(path) {
        Some(kind) => format!("{kind} on {entity_type}"),
        None => entity_type.to_owned(),
    }
}

/// Moves the entry to the line its `field_name:` is written on.
fn place_of(entry: &mut PlaceEntry) {
    let Ok(bytes) = std::fs::read(&entry.path) else {
        return;
    };
    let text = String::from_utf8_lossy(&bytes);
    for (number, line) in text.lines().enumerate() {
        if line.starts_with("field_name:") {
            entry.line = u32::try_from(number).unwrap_or(0);
            return;
        }
    }
}

/// Where the project keeps its exported configuration -- see [`super::config::exported_directories`]
/// -- plus what each extension ships, which is how a module declares its own fields.
fn config_directories(root: &Path, extensions: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = super::config::exported_directories(root);
    for info in extensions {
        let Some(directory) = info.parent() else {
            continue;
        };
        found.push(directory.join("config/install"));
        found.push(directory.join("config/optional"));
    }
    found
}

#[cfg(test)]
mod tests {
    use phrost_testkit::MaterializedFixture;

    use crate::{providers::drupal::Drupal, registry::FrameworkProvider};

    fn fields(fixture: &MaterializedFixture) -> Vec<String> {
        let mut found = Drupal
            .project(fixture.root())
            .catalog_places()
            .into_iter()
            .filter(|entry| entry.catalog == super::CATALOG)
            .map(|entry| {
                format!(
                    "{} [{}] {}:{}",
                    entry.key, entry.detail, entry.line, entry.character
                )
            })
            .collect::<Vec<_>>();
        found.sort();
        found
    }

    /// Both places configuration lives, and neither found by guessing where it is.
    #[test]
    fn the_sync_directory_and_an_extensions_own_config_are_both_read() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /docroot/modules/custom/mine/mine.info.yml
name: Mine
type: module

//- /docroot/modules/custom/mine/config/install/field.storage.node.field_shipped.yml
id: node.field_shipped
field_name: field_shipped
entity_type: node
type: boolean

//- /config/default/core.extension.yml
module:
  mine: 0

//- /config/default/field.storage.node.field_amount.yml
langcode: en
id: node.field_amount
field_name: field_amount
entity_type: node
type: decimal
"#,
        );

        assert_eq!(
            fields(&fixture),
            [
                // The line `field_name:` is on, so a jump selects the name and not the file.
                "field_amount [decimal on node] 2:0",
                "field_shipped [boolean on node] 1:0",
            ]
        );
    }

    /// The two halves of the join, each read out of the checkout, and each shown whole.
    ///
    /// The custom kind is the point of the last field: a project declares its own field types in
    /// PHP, so `field_own` is *of* a kind here and no table for that kind is written -- the honest
    /// answer for a declaration this layer cannot read, and a gap the index closes with the body.
    #[test]
    fn every_field_says_its_kind_and_every_core_kind_says_what_it_holds() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /config/default/core.extension.yml
module: {}

//- /config/default/field.storage.node.field_when.yml
field_name: field_when
entity_type: node
type: datetime

//- /config/default/field.storage.node.field_count.yml
field_name: field_count
entity_type: node
type: integer

//- /config/default/field.storage.node.field_also_count.yml
field_name: field_also_count
entity_type: node
type: integer

//- /config/default/field.storage.node.field_photo.yml
field_name: field_photo
entity_type: node
type: image

//- /config/default/field.storage.node.field_own.yml
field_name: field_own
entity_type: node
type: store_sales
"#,
        );
        let project = Drupal.project(fixture.root());

        let mut kinds = project
            .catalog_kinds()
            .into_iter()
            .map(|found| format!("{} is a {}", found.key, found.kind))
            .collect::<Vec<_>>();
        kinds.sort();
        assert_eq!(
            kinds,
            [
                "field_also_count is a integer",
                "field_count is a integer",
                "field_own is a store_sales",
                "field_photo is a image",
                "field_when is a datetime",
            ]
        );

        let mut tables = project
            .property_tables()
            .into_iter()
            .map(|table| {
                let names = table
                    .properties
                    .iter()
                    .map(|(name, holds)| match holds {
                        Some(holds) => format!("{name}: {}", holds.as_str()),
                        None => format!("{name}: ?"),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let complete = if table.complete { "" } else { " (partial)" };
                format!("{}{complete} -- {names}", table.kind)
            })
            .collect::<Vec<_>>();
        tables.sort();
        assert_eq!(
            tables,
            [
                // A `datetime` is stored as the ISO spelling, which is a string and not a date.
                "datetime -- value: string, date: ?",
                // A partial table answers for a name it holds and may never become a shape: the
                // reference family computes `target_id` in a branch nothing here follows.
                "image (partial) -- entity: ?, alt: string, title: string, width: int, height: int",
                // One table for the two fields of that kind, which is the point of keying by kind.
                "integer -- value: int",
            ],
            "no table for `store_sales`, whose declaration is PHP this layer cannot read"
        );
    }

    /// The sync directory is found by the file Drupal requires in it, so a project that keeps its
    /// configuration somewhere this does not look contributes nothing rather than something wrong.
    #[test]
    fn a_directory_with_no_marker_is_not_a_configuration_directory() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /config/somewhere/field.storage.node.field_amount.yml
field_name: field_amount
entity_type: node
type: decimal
"#,
        );

        assert!(fields(&fixture).is_empty());
    }

    #[test]
    fn a_file_that_is_not_a_field_storage_is_skipped() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /config/default/core.extension.yml
module: {}

//- /config/default/field.field.node.item.field_amount.yml
field_name: field_amount

//- /config/default/node.type.item.yml
type: item
"#,
        );

        assert!(
            fields(&fixture).is_empty(),
            "a bundle attachment is not the field's declaration, and a node type is not a field"
        );
    }
}
