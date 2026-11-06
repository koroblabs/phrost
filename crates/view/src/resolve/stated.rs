//! A property a class states in an array it writes, and `__get` answers from a row nobody declared.
//!
//! Eloquent's model: `$casts = ['published_at' => 'datetime']` makes `$post->published_at` a date,
//! and `$fillable = ['title']` makes `title` an attribute at all. The index keeps what each class
//! wrote; what a spelling *means* is the framework's vocabulary, read here -- see
//! [`phrost_index::StatedPropertiesRule`]. Asked after the member table and after an accessor, as
//! Laravel asks: a declared property is the declaration's, and an accessor outranks a cast.

use phrost_index::{StatedKind, StatedPropertiesRule, StatedValue};
use phrost_model::BuiltinTy;
use phrost_sema::MemberKind;

use crate::{
    location::SymbolLocation,
    resolve::{HIERARCHY_DEPTH, Held},
    view::IndexView,
};

/// The row a class or one of its parents states for a name, and who stated it.
struct Found {
    declarer: SymbolLocation,
    row: phrost_index::StatedRow,
}

impl IndexView<'_> {
    /// What such a property holds: the cast's type, nullable where the rule says a read can be
    /// null; `mixed` where nothing says more than that the property is there.
    pub(crate) fn stated_held(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<Held> {
        if kind != MemberKind::Property {
            return None;
        }
        let rule = self.stated_rule(class_fqn)?;
        // The column, where the migrations write one: what a row holds when nothing casts it, and
        // whether it can be null at all -- which is the half of a cast's answer the model cannot say.
        let column = self.model_column(class_fqn, name);
        let held = match self.stated_row(class_fqn, name) {
            Some(found) => match &found.row.value {
                StatedValue::Text(spelling) => self.cast_spelled(rule, spelling),
                StatedValue::Class(class) => {
                    // A cast of the project's own answers the null itself: the framework hands it
                    // a null column rather than answering for it, so what `get()` declares is the
                    // whole answer, nullable or not.
                    if let Some(returned) = self.custom_cast(rule, class) {
                        return Some(returned);
                    }
                    self.cast_class(rule, class)
                }
                StatedValue::Listed | StatedValue::Other => None,
            },
            None => match rule.key.as_ref() {
                Some(key) if *key.name == *name => {
                    // A class that changes its key -- `HasUuids`, a `$keyType` -- says what the key
                    // is not; the column, where a migration writes it, says what it is.
                    if self.key_changed(class_fqn, rule, key) {
                        return column.as_ref().map(Self::column_held);
                    }
                    self.stated_kind(&key.kind)
                }
                _ => match rule
                    .implicit
                    .iter()
                    .find(|(implicit, _)| &**implicit == name)
                {
                    Some((_, kind)) => self.stated_kind(kind),
                    // Nothing on the class names it, and the table has it.
                    None => return column.as_ref().map(Self::column_held),
                },
            },
        };
        let nullable = column
            .as_ref()
            .map_or(rule.nullable, |column| column.nullable);
        Some(match held {
            Some(held) if nullable => Self::nullable(held),
            Some(held) => held,
            None => column
                .as_ref()
                .map_or(Held::Builtin(BuiltinTy::Mixed), Self::column_held),
        })
    }

    /// Where such a property is stated: the name in the array that says so.
    pub(crate) fn stated_location(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<SymbolLocation> {
        if kind != MemberKind::Property {
            return None;
        }
        self.stated_rule(class_fqn)?;
        let Some(found) = self.stated_row(class_fqn, name) else {
            return self.column_location(class_fqn, name);
        };
        let signature = self
            .stated_held(class_fqn, kind, name)
            .zip(self.current().map(|open| open.hir()))
            .and_then(|(held, hir)| {
                self.render_held(
                    phrost_model::TextSize::default(),
                    hir,
                    &held,
                    crate::variables::Naming::Short,
                )
            })
            .map_or_else(|| format!("${name}"), |ty| format!("{ty} ${name}"));
        Some(SymbolLocation {
            file: found.declarer.file,
            fqn: found.row.name.clone(),
            kind: phrost_model::SymbolKind::Property,
            name_range: found.row.at,
            full_range: found.row.at,
            container: Some(found.declarer.fqn.clone()),
            signature: Some(signature.into()),
            member_modifiers: Some(crate::location::MemberModifiers {
                visibility: phrost_model::Visibility::Public,
                set_visibility: None,
                is_static: false,
                is_abstract: false,
                is_final: false,
                is_readonly: false,
            }),
            from_docblock: false,
            origin: None,
            docblock_range: None,
            deprecated: false,
        })
    }

    /// The properties a list should offer on `class_fqn`: every name it or a parent states, and
    /// the ones every such class has.
    pub(crate) fn stated_properties_of(&self, class_fqn: &str) -> Vec<crate::UndeclaredProperty> {
        let hir = self.current().map(|open| open.hir());
        self.stated_names(class_fqn)
            .into_iter()
            .map(|(name, declarer)| {
                let ty = self
                    .stated_held(class_fqn, MemberKind::Property, &name)
                    .zip(hir)
                    .and_then(|(held, hir)| {
                        self.render_held(
                            phrost_model::TextSize::default(),
                            hir,
                            &held,
                            crate::variables::Naming::Short,
                        )
                    });
                crate::UndeclaredProperty {
                    name,
                    ty: ty.map(Into::into),
                    declarer,
                    keyed: false,
                }
            })
            .collect()
    }

    /// Every name `class_fqn` or a parent states, and the ones every such class has, with the class
    /// whose body states each -- `None` for the ones nothing writes down.
    pub(crate) fn stated_names(
        &self,
        class_fqn: &str,
    ) -> Vec<(Box<str>, Option<phrost_model::Name>)> {
        let Some(rule) = self.stated_rule(class_fqn) else {
            return Vec::new();
        };
        let mut names: Vec<(Box<str>, Option<phrost_model::Name>)> = Vec::new();
        for (declarer, table) in self.stated_tables(class_fqn) {
            for row in &table.rows {
                if !names.iter().any(|(known, _)| **known == *row.name.as_str()) {
                    names.push((row.name.as_str().into(), Some(declarer.fqn.clone())));
                }
            }
        }
        for (implicit, _) in &rule.implicit {
            if !names.iter().any(|(known, _)| known == implicit) {
                names.push((implicit.clone(), None));
            }
        }
        if let Some(key) = rule.key.as_ref()
            && !self.key_changed(class_fqn, rule, key)
            && !names.iter().any(|(known, _)| *known == key.name)
        {
            names.push((key.name.clone(), None));
        }
        // And every column the table has, which is most of what a model's rows hold.
        for column in self.model_columns(class_fqn) {
            if !names.iter().any(|(known, _)| *known == column.name) {
                names.push((column.name, None));
            }
        }
        names
    }

    /// Whether the class, or a trait or a parent below the rule's class, declares one of the members
    /// that decide the key -- in which case the default key is not this one's.
    fn key_changed(
        &self,
        class_fqn: &str,
        rule: &StatedPropertiesRule,
        key: &phrost_index::StatedKeyRule,
    ) -> bool {
        let table = self.members(class_fqn);
        key.unless.iter().any(|member| {
            [MemberKind::Property, MemberKind::Method]
                .into_iter()
                .filter_map(|kind| table.find(kind, member))
                .any(|found| match &found.provenance {
                    phrost_sema::MemberProvenance::Inherited(owner) => {
                        !owner.as_str().eq_ignore_ascii_case(&rule.class)
                            && !self.descends_from(&rule.class, owner.as_str(), HIERARCHY_DEPTH)
                    }
                    phrost_sema::MemberProvenance::Interface(_) => false,
                    phrost_sema::MemberProvenance::Own
                    | phrost_sema::MemberProvenance::Trait(_) => {
                        !class_fqn.eq_ignore_ascii_case(&rule.class)
                    }
                })
        })
    }

    /// The rule speaking for a class, when one does.
    fn stated_rule(&self, class_fqn: &str) -> Option<&StatedPropertiesRule> {
        self.skeleton
            .returns()
            .stated_property_rules()
            .iter()
            .find(|rule| {
                class_fqn.eq_ignore_ascii_case(&rule.class)
                    || self.descends_from(class_fqn, &rule.class, HIERARCHY_DEPTH)
            })
    }

    /// What a name is stated as, nearest class first, and within one class a row that says what
    /// it holds before one that only lists it -- and the later of two such, which is Laravel's merge.
    fn stated_row(&self, class_fqn: &str, name: &str) -> Option<Found> {
        let mut listed = None;
        for (declarer, table) in self.stated_tables(class_fqn) {
            let mut rows = table.rows.iter().filter(|row| row.name.as_str() == name);
            if let Some(row) = rows
                .clone()
                .rfind(|row| !matches!(row.value, StatedValue::Listed))
            {
                return Some(Found {
                    declarer,
                    row: row.clone(),
                });
            }
            if listed.is_none()
                && let Some(row) = rows.next_back()
            {
                listed = Some(Found {
                    declarer: declarer.clone(),
                    row: row.clone(),
                });
            }
        }
        listed
    }

    /// Every row the class and its parents state that is still in force, nearest class first.
    ///
    /// A parent's row is shadowed when a nearer class wrote the same source: PHP does not merge a
    /// redeclared property's default with the parent's, and an overriding `casts()` replaces the
    /// parent's unless it calls it -- which, being a call, is not a literal this reads.
    fn stated_tables(&self, class_fqn: &str) -> Vec<(SymbolLocation, phrost_index::StatedTable)> {
        let mut tables = Vec::new();
        let mut shadowed: Vec<Box<str>> = Vec::new();
        let mut next = Some(class_fqn.to_owned());
        for _ in 0..HIERARCHY_DEPTH {
            let Some(class) = next.take().and_then(|fqn| self.class_location(&fqn)) else {
                break;
            };
            if let Some(table) = self.skeleton.stated_by(class.file, class.fqn.as_str()) {
                let rows = table
                    .rows
                    .iter()
                    .filter(|row| !shadowed.contains(&row.source))
                    .cloned()
                    .collect::<Vec<_>>();
                for row in &table.rows {
                    if !shadowed.contains(&row.source) {
                        shadowed.push(row.source.clone());
                    }
                }
                // A source written in this class shadows the parent's for every name, not only the
                // ones it repeats -- so it is recorded after the rows are taken, not before.
                tables.push((
                    class.clone(),
                    phrost_index::StatedTable {
                        declarer: table.declarer.clone(),
                        rows,
                        getters: Vec::new(),
                    },
                ));
            }
            next = self.class_header(class.fqn.as_str()).and_then(|header| {
                header
                    .extends
                    .first()
                    .map(|parent| parent.as_str().to_owned())
            });
        }
        tables
    }

    /// What a written spelling means: whole, then with a wrapping prefix taken off, then by what
    /// comes before its `:`.
    fn cast_spelled(&self, rule: &StatedPropertiesRule, spelling: &str) -> Option<Held> {
        let mut spelling = spelling.trim();
        if let Some(unwrapped) = rule
            .wrapping
            .iter()
            .find_map(|prefix| spelling.strip_prefix(&**prefix))
        {
            spelling = unwrapped;
        }
        let find = |wanted: &str| {
            rule.spellings
                .iter()
                .find(|(spelled, _)| spelled.eq_ignore_ascii_case(wanted))
                .map(|(_, kind)| kind)
        };
        let kind = find(spelling).or_else(|| find(spelling.split(':').next()?))?;
        self.stated_kind(kind)
    }

    /// What a class written as the cast means: an enum is itself, and a class the rule names is
    /// what it names.
    fn cast_class(&self, rule: &StatedPropertiesRule, class: &str) -> Option<Held> {
        if let Some((_, kind)) = rule
            .classes
            .iter()
            .find(|(named, _)| named.eq_ignore_ascii_case(class))
        {
            return self.stated_kind(kind);
        }
        let header = self.class_header(class)?;
        (header.kind == phrost_sema::ClassKind::Enum).then_some(Held::Class(header.fqn))
    }

    /// What a cast class of the project's own returns, where it implements the rule's interface.
    fn custom_cast(&self, rule: &StatedPropertiesRule, class: &str) -> Option<Held> {
        let (interface, method) = rule.custom.as_ref()?;
        let header = self.class_header(class)?;
        if !self.descends_from(header.fqn.as_str(), interface, HIERARCHY_DEPTH) {
            return None;
        }
        self.member_held(header.fqn.as_str(), MemberKind::Method, method)
    }

    fn stated_kind(&self, kind: &StatedKind) -> Option<Held> {
        match kind {
            StatedKind::Builtin(builtin) => Some(Held::Builtin(*builtin)),
            StatedKind::Class(class) => Some(Held::Class(self.class_location(class)?.fqn)),
        }
    }
}
