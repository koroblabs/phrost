//! What each kind of field holds, property by property.
//!
//! `$node->get('field_customer_type')->value` is a `string`, and nothing in any PHP signature says
//! so: `get()` hands back a `FieldItemListInterface` whose `__get` answers to every name, and what
//! `value` holds depends on which field the list came from. The chain that settles it runs through
//! two facts the project states in two different places -- the field's *kind*, written in
//! `field.storage.<entity type>.<name>.yml`, and what a kind holds, written in the
//! `propertyDefinitions()` of the class carrying `#[FieldType(id: ...)]`.
//!
//! # Why core's kinds are a table here and a project's own are not
//!
//! Both are declared the same way, in PHP, and only one of them is readable. phrost lowers bodies
//! for project code and deliberately not for vendor or core -- the memory rule §5 states -- so
//! `StoreSalesItem::propertyDefinitions()` in a custom module is in hand at ingest and
//! `StringItemBase::propertyDefinitions()` in core is not. What is written out below is therefore
//! core's own answer, read out of core rather than remembered: every row was extracted from the
//! `propertyDefinitions()` of the class the id names, with a parent's table merged in where the
//! body begins `$properties = parent::propertyDefinitions(...)`.
//!
//! [`Table::partial`] is the honest half of that. Three of core's kinds compute a property in a
//! branch this reading cannot follow, and a table that quietly omitted the key would state a shape
//! narrower than the truth -- so those say so, and only a complete table may become an
//! `array{...}`.

use phrost_model::BuiltinTy;

/// A kind of field, and what a value of it holds.
pub(super) struct Table {
    /// The `#[FieldType(id: ...)]` this describes.
    pub(super) kind: &'static str,
    /// One entry per property, in the order `propertyDefinitions()` writes them.
    pub(super) properties: &'static [(&'static str, TypedData)],
    /// Whether these are *all* of them.
    ///
    /// `false` where the declaration computes a property in a branch -- the entity reference
    /// family's `target_id`, whose type depends on the target entity type's id key. A partial table
    /// still answers for a property it names; it may not become a shape, because a shape claims
    /// that the keys it lists are the keys there are.
    pub(super) partial: bool,
}

/// One of Drupal's own type names, as `DataDefinition::create()` spells it.
///
/// Its own vocabulary rather than a [`BuiltinTy`], because the two do not line up: `timestamp` and
/// `datetime_iso8601` are both a PHP `string` or `int` at the seam and mean different things to
/// Drupal, and `any` means the framework declined to say. Kept as written so that a reader that
/// wants the distinction later has it, and mapped to PHP at the one place that has to answer in
/// PHP's vocabulary.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum TypedData {
    Text,
    Int,
    Float,
    Bool,
    /// A property the framework declares as `any`, or one whose own type name names a structure
    /// rather than a value -- `filter_format`, `layout_section`, a data reference. The property
    /// exists and this says nothing about what is in it, which is the honest answer and is what
    /// keeps the key in a shape rather than dropping it.
    Unsaid,
}

impl TypedData {
    /// The PHP type, for the reader that has to answer in PHP's vocabulary.
    pub(super) fn holds(self) -> Option<BuiltinTy> {
        match self {
            Self::Text => Some(BuiltinTy::String),
            Self::Int => Some(BuiltinTy::Int),
            Self::Float => Some(BuiltinTy::Float),
            Self::Bool => Some(BuiltinTy::Bool),
            Self::Unsaid => None,
        }
    }
}

/// The kinds Drupal core declares.
///
/// Read out of core, not remembered -- see the module's own note. A checkout whose core declares a
/// kind this does not list contributes nothing for it rather than something wrong, and a project's
/// own kinds do not appear here at all: those are read from the declaration, which is the source
/// this cannot reach for core.
pub(super) static CORE: &[Table] = &[
    Table {
        kind: "boolean",
        properties: &[("value", TypedData::Bool)],
        partial: false,
    },
    Table {
        kind: "changed",
        properties: &[("value", TypedData::Int)],
        partial: false,
    },
    Table {
        kind: "comment",
        properties: &[
            ("status", TypedData::Int),
            ("cid", TypedData::Int),
            ("last_comment_timestamp", TypedData::Int),
            ("last_comment_name", TypedData::Text),
            ("last_comment_uid", TypedData::Int),
            ("comment_count", TypedData::Int),
        ],
        partial: false,
    },
    Table {
        kind: "created",
        properties: &[("value", TypedData::Int)],
        partial: false,
    },
    Table {
        kind: "daterange",
        properties: &[
            ("value", TypedData::Text),
            ("start_date", TypedData::Unsaid),
            ("end_value", TypedData::Text),
            ("end_date", TypedData::Unsaid),
        ],
        partial: false,
    },
    Table {
        kind: "datetime",
        properties: &[("value", TypedData::Text), ("date", TypedData::Unsaid)],
        partial: false,
    },
    Table {
        kind: "decimal",
        properties: &[("value", TypedData::Text)],
        partial: false,
    },
    Table {
        kind: "email",
        properties: &[("value", TypedData::Text)],
        partial: false,
    },
    // The reference family's `target_id` is written in a branch on whether the target entity type
    // has an integer id, so its type is not one thing and this reading does not follow the branch.
    Table {
        kind: "entity_reference",
        properties: &[("entity", TypedData::Unsaid)],
        partial: true,
    },
    Table {
        kind: "file",
        properties: &[
            ("entity", TypedData::Unsaid),
            ("display", TypedData::Bool),
            ("description", TypedData::Text),
        ],
        partial: true,
    },
    Table {
        kind: "file_uri",
        properties: &[("value", TypedData::Text), ("url", TypedData::Text)],
        partial: false,
    },
    Table {
        kind: "float",
        properties: &[("value", TypedData::Float)],
        partial: false,
    },
    Table {
        kind: "image",
        properties: &[
            ("entity", TypedData::Unsaid),
            ("alt", TypedData::Text),
            ("title", TypedData::Text),
            ("width", TypedData::Int),
            ("height", TypedData::Int),
        ],
        partial: true,
    },
    Table {
        kind: "integer",
        properties: &[("value", TypedData::Int)],
        partial: false,
    },
    Table {
        kind: "language",
        properties: &[("value", TypedData::Text), ("language", TypedData::Unsaid)],
        partial: false,
    },
    Table {
        kind: "layout_section",
        properties: &[("section", TypedData::Unsaid)],
        partial: false,
    },
    Table {
        kind: "link",
        properties: &[
            ("uri", TypedData::Text),
            ("resolvable_uri", TypedData::Unsaid),
            ("title", TypedData::Text),
        ],
        partial: false,
    },
    Table {
        kind: "list_float",
        properties: &[("value", TypedData::Float)],
        partial: false,
    },
    Table {
        kind: "list_integer",
        properties: &[("value", TypedData::Int)],
        partial: false,
    },
    Table {
        kind: "list_string",
        properties: &[("value", TypedData::Text)],
        partial: false,
    },
    // `MapItem` declares none: an arbitrary map is what the kind is for.
    Table {
        kind: "map",
        properties: &[],
        partial: true,
    },
    Table {
        kind: "password",
        properties: &[
            ("value", TypedData::Text),
            ("existing", TypedData::Text),
            ("pre_hashed", TypedData::Bool),
        ],
        partial: false,
    },
    Table {
        kind: "path",
        properties: &[
            ("alias", TypedData::Text),
            ("pid", TypedData::Int),
            ("langcode", TypedData::Text),
        ],
        partial: false,
    },
    Table {
        kind: "string",
        properties: &[("value", TypedData::Text)],
        partial: false,
    },
    Table {
        kind: "string_long",
        properties: &[("value", TypedData::Text)],
        partial: false,
    },
    Table {
        kind: "telephone",
        properties: &[("value", TypedData::Text)],
        partial: false,
    },
    Table {
        kind: "text",
        properties: &[
            ("value", TypedData::Text),
            ("format", TypedData::Unsaid),
            ("processed", TypedData::Text),
        ],
        partial: false,
    },
    Table {
        kind: "text_long",
        properties: &[
            ("value", TypedData::Text),
            ("format", TypedData::Unsaid),
            ("processed", TypedData::Text),
        ],
        partial: false,
    },
    Table {
        kind: "text_with_summary",
        properties: &[
            ("value", TypedData::Text),
            ("format", TypedData::Unsaid),
            ("processed", TypedData::Text),
            ("summary", TypedData::Text),
            ("summary_processed", TypedData::Text),
        ],
        partial: false,
    },
    Table {
        kind: "timestamp",
        properties: &[("value", TypedData::Int)],
        partial: false,
    },
    Table {
        kind: "uri",
        properties: &[("value", TypedData::Text)],
        partial: false,
    },
    Table {
        kind: "uuid",
        properties: &[("value", TypedData::Text)],
        partial: false,
    },
];

#[cfg(test)]
mod tests {
    use super::{CORE, TypedData};

    /// One kind per id, because two rows for one id is two answers to one question and the reader
    /// would take whichever came first.
    #[test]
    fn no_kind_is_declared_twice() {
        let mut seen: Vec<&str> = CORE.iter().map(|table| table.kind).collect();
        let total = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), total, "a kind is written twice");
    }

    /// The two rows everybody gets wrong, which is what a test is for: Drupal stores a `decimal` as
    /// its digits and a `datetime` as the ISO spelling, so both are a `string` and neither is the
    /// thing its name suggests.
    #[test]
    fn the_kinds_whose_php_type_surprises_are_written_out() {
        let holds = |kind: &str, property: &str| {
            CORE.iter()
                .find(|table| table.kind == kind)
                .and_then(|table| table.properties.iter().find(|(name, _)| *name == property))
                .map(|(_, typed)| *typed)
        };
        assert_eq!(holds("decimal", "value"), Some(TypedData::Text));
        assert_eq!(holds("datetime", "value"), Some(TypedData::Text));
        assert_eq!(holds("timestamp", "value"), Some(TypedData::Int));
        assert_eq!(holds("boolean", "value"), Some(TypedData::Bool));
        // A property whose own type names a structure rather than a value says nothing about what
        // is in it, and stays in the table: the key exists whatever it holds.
        assert_eq!(holds("text", "format"), Some(TypedData::Unsaid));
    }

    /// A partial table may answer for a property it names and may never become a shape.
    #[test]
    fn the_reference_family_says_it_is_partial() {
        for kind in ["entity_reference", "file", "image"] {
            let table = CORE.iter().find(|table| table.kind == kind).expect(kind);
            assert!(table.partial, "{kind} computes `target_id` in a branch");
        }
        let string = CORE
            .iter()
            .find(|table| table.kind == "string")
            .expect("string");
        assert!(!string.partial);
    }
}
