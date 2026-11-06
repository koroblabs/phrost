//! A model's columns: the table its rows are in, as the project's migrations leave it.
//!
//! Which table is the framework's `getTable()`: the one the nearest class in the hierarchy names, or
//! the one the model's own short name makes -- the class the value *is*, since the runtime asks
//! `class_basename($this)`. What each column holds is the raw value the database hands back; a cast
//! the model states is read first, and takes the column's nullability -- see `stated.rs`.

use phrost_index::schema::Column;
use phrost_model::BuiltinTy;

use crate::{
    resolve::{HIERARCHY_DEPTH, Held},
    view::IndexView,
};

impl IndexView<'_> {
    /// The table `class_fqn`'s rows are in, where it is a model at all.
    pub(crate) fn model_table(&self, class_fqn: &str) -> Option<std::sync::Arc<str>> {
        if self.skeleton.returns().written_schema().is_empty() {
            return None;
        }
        if let Some(known) = self.model_tables.borrow().get(class_fqn) {
            return known.clone();
        }
        let found = self.table_of_model(class_fqn).map(std::sync::Arc::from);
        self.model_tables
            .borrow_mut()
            .insert(class_fqn.into(), found.clone());
        found
    }

    fn table_of_model(&self, class_fqn: &str) -> Option<String> {
        let rule = self.skeleton.returns().written_schema().first()?;
        if !self.descends_from(class_fqn, &rule.model, HIERARCHY_DEPTH) {
            return None;
        }
        // The nearest class naming one, below the framework's.
        let mut class = self.class_location(class_fqn)?.fqn;
        for _ in 0..HIERARCHY_DEPTH {
            if class.as_str().eq_ignore_ascii_case(&rule.model) {
                break;
            }
            let location = self.class_location(class.as_str())?;
            if let Some(table) = self.skeleton.named_table(location.file, class.as_str()) {
                return Some(table.to_owned());
            }
            let Some(parent) = self
                .class_header(class.as_str())
                .and_then(|header| header.extends.first().cloned())
            else {
                break;
            };
            class = parent;
        }
        Some((rule.default_table)(phrost_model::short_name(class_fqn)))
    }

    /// The column `name` of `class_fqn`'s table, as the migrations leave it.
    pub(crate) fn model_column(&self, class_fqn: &str, name: &str) -> Option<Column> {
        let table = self.model_table(class_fqn)?;
        self.skeleton
            .schema()
            .table(&table)?
            .iter()
            .find(|column| &*column.name == name)
            .cloned()
    }

    /// Every column of `class_fqn`'s table, in the order the migrations added them.
    pub(crate) fn model_columns(&self, class_fqn: &str) -> Vec<Column> {
        let Some(table) = self.model_table(class_fqn) else {
            return Vec::new();
        };
        self.skeleton
            .schema()
            .table(&table)
            .map(<[Column]>::to_vec)
            .unwrap_or_default()
    }

    /// What reading a column hands back before anything casts it.
    pub(crate) fn column_held(column: &Column) -> Held {
        let held = Held::Builtin(column.holds);
        match column.nullable && column.holds != BuiltinTy::Mixed {
            true => Self::nullable(held),
            false => held,
        }
    }

    /// Where a column `class_fqn`'s table has is defined: the name in the migration that last
    /// defined it, or renamed it to what it is.
    pub(crate) fn column_location(
        &self,
        class_fqn: &str,
        name: &str,
    ) -> Option<crate::location::SymbolLocation> {
        let column = self.model_column(class_fqn, name)?;
        let fqn = self.skeleton.interner().lookup_name(name)?;
        let signature = self
            .current()
            .and_then(|open| {
                self.render_held(
                    phrost_model::TextSize::default(),
                    open.hir(),
                    &Self::column_held(&column),
                    crate::variables::Naming::Short,
                )
            })
            .map_or_else(|| format!("${name}"), |ty| format!("{ty} ${name}"));
        Some(crate::location::SymbolLocation {
            file: column.file,
            fqn,
            kind: phrost_model::SymbolKind::Property,
            name_range: column.at,
            full_range: column.at,
            container: Some(self.class_location(class_fqn)?.fqn),
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
}
