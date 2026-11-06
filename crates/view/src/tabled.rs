//! A property whose type is a table lookup, keyed by a string written two links back.
//!
//! `$node->get('field_when')->value` is a `string` and `$node->get('field_count')->value` is an
//! `int`, and the only difference between the two reads is the argument of a call that has already
//! happened. No signature can say it: `get()` hands back one interface whose `__get` answers to
//! every name.
//!
//! # Why it is a lookback rather than a property with a type
//!
//! `magic_property_held` beside this answers the other half, and its own contract states the
//! division: a magic property holds *one* class for every name its accessor answers to. Here every
//! name holds something different, so what settles it is not the name but the key -- and the key is
//! on an earlier link, exactly where `returned_through` finds the one that says which entity a
//! storage loads.
//!
//! # The two readings of one table
//!
//! A property, and the whole table at once. `getValue()` hands back one entry per delta, each
//! holding every property, so what it answers is a list of the shape -- the same fact read the
//! other way round, and the reason both are interned rather than built here: a shape has no arm in
//! [`Held`], and the type table is open for reading only at a request.

use phrost_model::Name;

use phrost_sema::MemberKind;

use crate::{SymbolLocation, resolve::Held, view::IndexView};

impl IndexView<'_> {
    /// What a property on a tabled receiver holds, when a framework says the receiver is one.
    ///
    /// `produced_by` is the call that made the receiver and the literal it was given -- the same
    /// pair `returned_through` takes, and asked by the caller for the same reason: the lookback is
    /// a walk, and it is worth doing only once something says this class has a table at all.
    pub(crate) fn tabled_property(
        &self,
        class_fqn: &str,
        name: &str,
        produced_by: Option<(&Name, &Name)>,
    ) -> Option<Held> {
        let (through, key) = produced_by?;
        let contracts = self.skeleton.returns();
        for binding in contracts.tabled_bindings() {
            if !binding.through.eq_ignore_ascii_case(through.as_str())
                || !self.tabled(class_fqn, &binding.class)
            {
                continue;
            }
            // A property the table names, whatever it says about it: a `None` there means the
            // declaration named a type PHP has no word for, and the honest answer is that the
            // property is there and nothing is claimed about it -- which is what declining says,
            // because the declared type then stands.
            let Some(rows) = contracts.property_table(&binding.catalog, key.as_str()) else {
                continue;
            };
            if let Some((_, holds)) = rows.properties.iter().find(|(held, _)| &**held == name) {
                return holds.map(Held::Builtin);
            }
        }
        None
    }

    /// The whole table at once, for the method that hands back one entry per row.
    ///
    /// Only a table that says it is complete: a shape claims the names it lists are the names there
    /// are, and Drupal's reference family computes a property in a branch nothing reads -- so a
    /// shape built from what is left would state a type narrower than the truth, which is how a
    /// checker reports working code.
    pub(crate) fn tabled_rows(
        &self,
        class_fqn: &str,
        method: &str,
        produced_by: Option<(&Name, &Name)>,
    ) -> Option<Held> {
        let (through, key) = produced_by?;
        let contracts = self.skeleton.returns();
        for binding in contracts.tabled_bindings() {
            if binding.rows.as_deref() != Some(method)
                || !binding.through.eq_ignore_ascii_case(through.as_str())
                || !self.tabled(class_fqn, &binding.class)
            {
                continue;
            }
            if let Some(shape) = self.skeleton.property_shape(&binding.catalog, key.as_str()) {
                return Some(Held::Stored {
                    ty: shape.rows,
                    scope: None,
                });
            }
        }
        None
    }

    /// What a property holds on an instance of the class whose own body states its table.
    ///
    /// `#[FieldType(id: 'refund')] class RefundItem` writes `$properties['enabled'] =
    /// DataDefinition::create('boolean')`, and an instance of it is one value of that kind -- so
    /// `$this->enabled` inside it and `$item->enabled` on one read the same table a field list's key
    /// does, with no key to look back for. Its own table first, then each parent's: a subclass is
    /// the same kind unless it states another.
    ///
    /// Asked only where the member table has no such property, which is where PHP's `__get` is what
    /// answers -- a declared property is the declaration's.
    pub(crate) fn declared_table_held(&self, class_fqn: &str, name: &str) -> Option<Held> {
        let (_, table, index) = self.declared_table_entry(class_fqn, name)?;
        table.properties[index].1.map(Held::Builtin)
    }

    /// Where the table [`Self::declared_table_held`] reads states the property: the key of
    /// `$properties['enabled'] = ...`, which is the only declaration the name has anywhere.
    pub(crate) fn declared_table_location(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<SymbolLocation> {
        if kind != MemberKind::Property {
            return None;
        }
        let (declarer, table, index) = self.declared_table_entry(class_fqn, name)?;
        let at = *table.written_at.get(index)?;
        let holds = table.properties[index].1;
        Some(SymbolLocation {
            file: declarer.file,
            fqn: self.skeleton.interner().lookup_name(name)?,
            kind: phrost_model::SymbolKind::Property,
            name_range: at,
            full_range: at,
            container: Some(declarer.fqn),
            signature: Some(match holds {
                Some(holds) => format!("{} ${name}", holds.as_str()).into(),
                None => format!("${name}").into(),
            }),
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

    /// Whether the only declaration `name` has on `class_fqn` is a key of such a table.
    ///
    /// For a rename, which has to refuse one: the same name is a column in the declaration's
    /// `schema()` and a key every field of the kind is read through, and no reference walk reaches
    /// either.
    pub fn declared_only_by_a_table(&self, class_fqn: &str, name: &str) -> bool {
        self.members(class_fqn)
            .find(MemberKind::Property, name)
            .is_none()
            && self.declared_table_entry(class_fqn, name).is_some()
    }

    /// The class stating the table that holds `name` for `class_fqn`, the table, and the name's
    /// place in it. Its own table first, then each parent's: a subclass is the same kind unless it
    /// states another.
    fn declared_table_entry(
        &self,
        class_fqn: &str,
        name: &str,
    ) -> Option<(SymbolLocation, &phrost_index::FoundTable, usize)> {
        self.declared_tables(class_fqn)
            .into_iter()
            .find_map(|(class, table)| {
                let index = table
                    .properties
                    .iter()
                    .position(|(held, _)| &**held == name)?;
                Some((class, table, index))
            })
    }

    /// Every table that speaks for an instance of `class_fqn`, nearest first: its own, then each
    /// parent's for as long as the one below it says it is partial.
    ///
    /// A complete table speaks for the kind whole, so a name it does not hold is not one a parent's
    /// may answer instead. A partial one -- `$properties = parent::propertyDefinitions($d)` -- says
    /// the parent's keys are there too.
    fn declared_tables(&self, class_fqn: &str) -> Vec<(SymbolLocation, &phrost_index::FoundTable)> {
        let mut tables = Vec::new();
        let Some(mut class) = self.class_location(class_fqn) else {
            return tables;
        };
        for _ in 0..crate::resolve::HIERARCHY_DEPTH {
            if let Some(table) = self
                .skeleton
                .table_declared_by(class.file, class.fqn.as_str())
            {
                let complete = table.complete;
                tables.push((class.clone(), table));
                if complete {
                    break;
                }
            }
            let Some(parent) = self
                .class_header(class.fqn.as_str())
                .and_then(|header| header.extends.first().cloned())
            else {
                break;
            };
            let Some(next) = self.class_location(parent.as_str()) else {
                break;
            };
            class = next;
        }
        tables
    }

    /// The properties a table states on a receiver being completed.
    ///
    /// Two places a table speaks for one, the same two goto reads: the class's own body, for an
    /// instance of a kind -- `$item->` on one, `$this->` inside it -- and the kind a key names, for a
    /// list a keyed call produced -- `$node->get('field_address')->`, or a local that list was put
    /// in. The call is read off the written chain rather than a parsed path, because the statement
    /// under the cursor does not parse; for a local it is read off what the local was assigned,
    /// which does.
    ///
    /// Nearest first and each name once, so a name a parent's table also writes is the subclass's.
    pub(crate) fn tabled_properties_of(
        &self,
        offset: phrost_model::TextSize,
        root: crate::ChainRoot<'_>,
        steps: &[crate::ChainStep<'_>],
        class_fqn: &str,
    ) -> Vec<crate::UndeclaredProperty> {
        if !self.has_tabled_reads() {
            return Vec::new();
        }
        let mut found: Vec<crate::UndeclaredProperty> = Vec::new();
        let mut offer =
            |name: &str, holds: Option<phrost_model::BuiltinTy>, declarer: &Option<Name>| {
                if !found.iter().any(|known| &*known.name == name) {
                    found.push(crate::UndeclaredProperty {
                        name: name.into(),
                        ty: holds.map(|holds| holds.as_str().into()),
                        declarer: declarer.clone(),
                        keyed: false,
                    });
                }
            };

        for (class, table) in self.declared_tables(class_fqn) {
            let declarer = Some(class.fqn);
            for (name, holds) in &table.properties {
                offer(name, *holds, &declarer);
            }
        }

        if let Some((through, key)) = self.written_producing_call(offset, root, steps) {
            let contracts = self.skeleton.returns();
            for binding in contracts.tabled_bindings() {
                if !binding.through.eq_ignore_ascii_case(&through)
                    || !self.tabled(class_fqn, &binding.class)
                {
                    continue;
                }
                let Some(rows) = contracts.property_table(&binding.catalog, &key) else {
                    continue;
                };
                let declarer = contracts
                    .catalog_kind(&binding.catalog, &key)
                    .and_then(|kind| self.skeleton.table_declaring(&binding.catalog, kind))
                    .and_then(|table| self.class_location(&table.declarer))
                    .map(|class| class.fqn);
                for (name, holds) in &rows.properties {
                    offer(name, *holds, &declarer);
                }
            }
        }
        found
    }

    /// The method and key that produced the value a written chain ends at, when a call with a
    /// literal first argument did: the chain's last step, or -- for a bare local -- the call the
    /// local was assigned from.
    fn written_producing_call(
        &self,
        offset: phrost_model::TextSize,
        root: crate::ChainRoot<'_>,
        steps: &[crate::ChainStep<'_>],
    ) -> Option<(Box<str>, Box<str>)> {
        match steps.last() {
            Some(crate::ChainStep::Member {
                name,
                kind: MemberKind::Method,
                arguments,
            }) => Some(((*name).into(), arguments.first()?.as_deref()?.into())),
            Some(_) => None,
            None => {
                let crate::ChainRoot::Local(local) = root else {
                    return None;
                };
                let hir = self.current()?.hir();
                let path = phrost_body::ValuePath {
                    root: phrost_body::PathRoot::Local(phrost_body::local_name(
                        hir, offset, local,
                    )?),
                    steps: Vec::new(),
                    may_be_absent: false,
                };
                let (through, key) = self.producing_call(offset, hir, &path, 0)?;
                Some((through.as_str().into(), key.as_str().into()))
            }
        }
    }

    /// Which class owns a member reached at a site: [`Self::owner_of_member`], and -- for a property
    /// read off a list of a kind the project declares itself -- the item class whose table states it.
    ///
    /// `$entity->get('field_address')->state` is read through `FieldItemListInterface`, whose
    /// `__get` answers every name; the table of the field's kind says what `state` holds, and the
    /// class that declared that kind is where it was written. So goto lands on the table's key, and
    /// the access is one the index resolves rather than one the magic excuses. Asked at the site
    /// rather than folded into `owner_of_member`, because the key is on an earlier link of the path
    /// and only the site has the path.
    pub(crate) fn owner_at_site(
        &self,
        offset: phrost_model::TextSize,
        hir: &phrost_hir::FileHir,
        receiver: &phrost_body::ValuePath,
        held: &Held,
        kind: MemberKind,
        name: &str,
    ) -> Option<Name> {
        let owner = self.owner_of_member(offset, hir, held, kind, name)?;
        // A method a framework finds under a prefix on the builder's model.
        if kind == MemberKind::Method
            && self.members(owner.as_str()).find(kind, name).is_none()
            && let Some(model) = self.prefixed_owner(hir, owner.as_str(), held, name)
        {
            return Some(model);
        }
        if kind != MemberKind::Property
            || !self.has_tabled_reads()
            || self.members(owner.as_str()).find(kind, name).is_some()
        {
            return Some(owner);
        }
        let declarer = self
            .producing_call(offset, hir, receiver, receiver.steps.len())
            .and_then(|(through, key)| self.tabled_declarer(owner.as_str(), name, &through, &key));
        Some(declarer.unwrap_or(owner))
    }

    /// The class whose own table states `name` for the kind a tabled read's key is of.
    fn tabled_declarer(
        &self,
        class_fqn: &str,
        name: &str,
        through: &Name,
        key: &Name,
    ) -> Option<Name> {
        let contracts = self.skeleton.returns();
        for binding in contracts.tabled_bindings() {
            if !binding.through.eq_ignore_ascii_case(through.as_str())
                || !self.tabled(class_fqn, &binding.class)
            {
                continue;
            }
            let Some(kind) = contracts.catalog_kind(&binding.catalog, key.as_str()) else {
                continue;
            };
            let Some(table) = self.skeleton.table_declaring(&binding.catalog, kind) else {
                continue;
            };
            if table.properties.iter().any(|(held, _)| &**held == name) {
                return Some(self.class_location(&table.declarer)?.fqn);
            }
        }
        None
    }

    /// Whether a receiver is the class a binding speaks for, or below it.
    fn tabled(&self, class_fqn: &str, binding: &str) -> bool {
        class_fqn.eq_ignore_ascii_case(binding)
            || self.descends_from(class_fqn, binding, crate::resolve::HIERARCHY_DEPTH)
    }

    /// Whether any framework contracted a tabled read at all.
    ///
    /// The gate in front of the lookback, which walks back through the path and its roots. A
    /// project with no such framework pays one emptiness test per property read and nothing else;
    /// the same shape `returned_through`'s caller uses, and for the same reason.
    pub(crate) fn has_tabled_reads(&self) -> bool {
        !self.skeleton.returns().tabled_bindings().is_empty()
    }
}
