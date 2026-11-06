use std::sync::Arc;

use phrost_model::{Name, Visibility};

use crate::{
    member::MemberKind,
    source::{ClassFacts, ClassKind, ClassSource, TraitAdaptation, TraitAdaptationKind},
    table::{MemberConflict, MemberProvenance, MemberTable, ResolvedMember},
};

/// The interface every enum implements without declaring it.
pub const UNIT_ENUM: &str = "UnitEnum";

/// And the one a backed enum implements instead, which extends `UnitEnum`.
pub const BACKED_ENUM: &str = "BackedEnum";

/// Flattens a class into every member it has.
///
/// Always returns a table. A missing parent or a cycle marks the result incomplete rather than
/// failing: real projects reference classes that are not installed, and a partial table is still
/// what completion and hover should show.
pub fn flatten_class(source: &dyn ClassSource, fqn: &str) -> MemberTable {
    flatten_class_with(source, fqn, &())
}

/// The same, reading and filling a memo of tables built before.
///
/// Every complete table the walk builds -- the class asked for and each ancestor under it -- is
/// offered to the memo, and a class the memo already holds is taken from it rather than walked.
/// That is what makes a memo across requests pay: a project's classes are nearly all new to each
/// request, but the base classes under them are the same few, and without this the walk rebuilt
/// every one of them for each new class above.
pub fn flatten_class_with(
    source: &dyn ClassSource,
    fqn: &str,
    memo: &dyn FlattenMemo,
) -> MemberTable {
    Flattener {
        source,
        memo,
        stack: Vec::new(),
        built: rustc_hash::FxHashMap::default(),
    }
    .flatten(fqn)
}

/// Somewhere flattened tables are kept between walks.
///
/// Keyed by the folded name. Whoever implements it decides how long a table lives; the flattener
/// only asks and offers. The unit type is the memo that keeps nothing.
pub trait FlattenMemo {
    fn flattened(&self, key: &str) -> Option<Arc<MemberTable>>;
    fn remember_flattened(&self, key: Box<str>, table: &Arc<MemberTable>);
}

impl FlattenMemo for () {
    fn flattened(&self, _key: &str) -> Option<Arc<MemberTable>> {
        None
    }

    fn remember_flattened(&self, _key: Box<str>, _table: &Arc<MemberTable>) {}
}

struct Flattener<'a> {
    source: &'a dyn ClassSource,
    memo: &'a dyn FlattenMemo,
    /// Fully-qualified names currently being flattened, for cycle detection. Broken code really
    /// does contain `class A extends A`, and without this it hangs the server.
    ///
    /// Each entry says whether it was reached through a `@mixin`, because a loop through one is not
    /// broken code: Laravel's `Eloquent\Builder` implements a contract that is `@mixin` it.
    stack: Vec<(String, bool)>,
    /// Tables already built during *this* flatten, folded like every class lookup.
    ///
    /// A hierarchy is a graph and not a tree: an interface reached through two parents was flattened
    /// twice, and `@mixin` made that visible -- `ProphecyTrait` is `@mixin TestCase` and is used by
    /// classes that already extend it, so every test class flattened PHPUnit's largest class a
    /// second time. That was 0.4 ms on a request budgeted 12.
    ///
    /// Only complete tables are kept. One truncated by the cycle guard is a table *about this
    /// walk*, not about the class, and `absorb` marks anything containing one -- so the flag is the
    /// test.
    built: rustc_hash::FxHashMap<String, Arc<MemberTable>>,
}

impl Flattener<'_> {
    fn flatten(&mut self, fqn: &str) -> MemberTable {
        self.flatten_via(fqn, false)
    }

    fn flatten_via(&mut self, fqn: &str, through_a_mixin: bool) -> MemberTable {
        let mut table = MemberTable::default();

        let Some(facts) = self.source.class(fqn) else {
            table.mark_incomplete();
            return table;
        };

        if let Some(depth) = self
            .stack
            .iter()
            .position(|(seen, _)| seen.eq_ignore_ascii_case(fqn))
        {
            let loops_through_a_mixin =
                through_a_mixin || self.stack[depth + 1..].iter().any(|&(_, mixin)| mixin);
            if loops_through_a_mixin {
                table.cut(Some(depth));
            } else {
                table.mark_incomplete();
            }
            return table;
        }

        let key = fqn.to_ascii_lowercase();
        if let Some(built) = self.built.get(&key) {
            return MemberTable::clone(built);
        }
        if let Some(kept) = self.memo.flattened(&key) {
            return MemberTable::clone(&kept);
        }
        self.stack.push((key.clone(), through_a_mixin));

        table.note_contributor(facts.fqn.clone());

        // Reverse precedence order: each step overwrites the previous one, so the last writer wins.
        // Own beats traits, traits beat the parent, and the parent beats interface requirements.
        self.merge_enum_interfaces(&mut table, &facts);
        self.merge_interfaces(&mut table, &facts);
        self.merge_parents(&mut table, &facts);
        self.merge_traits(&mut table, &facts);
        Self::merge_own(&mut table, &facts);
        Self::open(&mut table, &facts);
        self.merge_tag_sources(&mut table, &facts);
        // Last, and filling gaps rather than overwriting: a `@mixin` says a member answers at
        // runtime, not that it is the declaration. Coming last is also what makes it affordable --
        // by now the table knows which classes it already has.
        self.merge_mixins(&mut table, &facts);

        self.stack.pop();
        table.heal_at(self.stack.len());
        table.sort();
        if table.is_incomplete() || table.cut_at().is_some() {
            return table;
        }
        let shared = Arc::new(table);
        self.built.insert(key.clone(), Arc::clone(&shared));
        self.memo.remember_flattened(key.into_boxed_str(), &shared);
        MemberTable::clone(&shared)
    }

    /// The interfaces an enum implements without saying so.
    ///
    /// PHP gives every enum `UnitEnum` and every backed one `BackedEnum`, which is where `$name`,
    /// `$value`, `cases()`, `from()` and `tryFrom()` come from. Written nowhere in the source, so
    /// without this an enum has no members but its own cases -- and `$stage->value`, one of the most
    /// common things anyone writes with an enum, resolves to nothing.
    ///
    /// Taken from the standard library rather than synthesized here. A fabricated member would need
    /// a fabricated declaration range, and `$name` would navigate to somewhere that does not exist;
    /// the real interface has a real range, a real signature, and its own documentation.
    fn merge_enum_interfaces(&mut self, table: &mut MemberTable, facts: &ClassFacts<'_>) {
        if facts.kind != ClassKind::Enum {
            return;
        }

        let implicit = match facts.enum_backing {
            Some(_) => BACKED_ENUM,
            None => UNIT_ENUM,
        };
        // The interned name comes from the declaration itself rather than being made up here: a
        // fabricated one would compare equal but belong to no file, and provenance is what
        // navigation follows. No declaration means the standard library is not installed, which is
        // an incomplete table and not an empty one.
        let Some(name) = self.source.class(implicit).map(|facts| facts.fqn.clone()) else {
            table.mark_incomplete();
            return;
        };

        let inherited = self.flatten(implicit);
        self.absorb(table, inherited, |provenance| match provenance {
            MemberProvenance::Own => MemberProvenance::Interface(name.clone()),
            other => other,
        });

        // `$value` is the enum's own backing type, not the `int|string` the interface has to promise
        // for every backed enum at once. Retyped rather than replaced, so the declaration a reader is
        // sent to is still the real one in the standard library -- which is where `$value` is
        // declared, however specific this enum's is.
        if let Some(backing) = facts.enum_backing
            && let Some(index) = table.position(MemberKind::Property, "value")
        {
            let mut member = table.members()[index].clone();
            member.row.ty = Some(backing);
            // The backing type is the whole answer, so a docblock beside it has nothing to refine.
            member.row.doc_ty = None;
            table.replace(index, member);
        }
    }

    /// `@mixin` members are reachable but never declared: they lose to everything real.
    ///
    /// So they are added last and only where nothing is there already. The tag says these answer at
    /// runtime; it does not say they are the declaration, and overwriting a real one would send goto
    /// to the wrong file.
    ///
    /// A mixin the table already reaches is skipped whole, which is the difference between this
    /// being affordable and not. `ProphecyTrait` is `@mixin TestCase` and is used by classes that
    /// already extend it -- so without the check every test class copied PHPUnit's largest member
    /// list a second time to change nothing.
    fn merge_mixins(&mut self, table: &mut MemberTable, facts: &ClassFacts<'_>) {
        // Not on a trait. A trait is not something anyone holds: what is held is the class that
        // uses it, and there the tag is a claim about *that* class's hierarchy -- `ProphecyTrait` is
        // `@mixin TestCase` and every class using it already extends TestCase.
        //
        // Applying it here instead put PHPUnit's largest member list inside the trait's table, and
        // every class composing the trait then copied it to change nothing: completion p95 went from
        // 0.39 ms to 0.72. A class writing the tag itself is the case worth having and is unaffected.
        if facts.kind == ClassKind::Trait {
            return;
        }

        let written = facts.mixins.iter().map(|mixin| (mixin, false));
        let implied = facts.implied_mixins.iter().map(|mixin| (mixin, true));
        // What this class's own mixins brought in, so a later one of them does not displace an
        // earlier one: the first tag written answers, as it always did.
        let mut own: Vec<(MemberKind, Name)> = Vec::new();
        for (mixin, implied) in written.chain(implied) {
            if table
                .contributors()
                .iter()
                .any(|seen| seen.as_str().eq_ignore_ascii_case(mixin.fqn.as_str()))
            {
                continue;
            }

            let inherited = self.flatten_via(mixin.fqn.as_str(), true);
            // Only what an instance of the mixin would expose. A `@mixin` forwards calls; it does
            // not open up what the class kept to itself.
            let visible = MemberTable::retain_inheritable(inherited);
            if visible.is_incomplete() {
                table.mark_incomplete();
            }
            table.cut(visible.cut_at());
            for contributor in visible.contributors() {
                table.note_contributor(contributor.clone());
            }
            for member in visible.members() {
                // A member some other mixin forwards, reached through what this class inherits, is
                // the nearer mixin's to answer: `Relation` writes `@mixin Builder<TRelatedModel>`
                // and implements a contract that is `@mixin` the *query* builder, and at runtime its
                // `__call` hands `paginate()` to the Eloquent builder -- whose `paginate()` says
                // which model the page holds, where the query builder's says nothing. Kept, the
                // contract's came first and every relation's page was of nothing.
                let displaced = match table.position(member.row.kind, member.name.as_str()) {
                    Some(index)
                        if !implied
                            && table.members()[index].forwarded
                            && !own.iter().any(|(kind, name)| {
                                *kind == member.row.kind
                                    && name.as_str().eq_ignore_ascii_case(member.name.as_str())
                            }) =>
                    {
                        Some(index)
                    }
                    Some(_) => continue,
                    None => None,
                };
                // What a mixin forwards is a call on an instance, through `__call`, and `new` never
                // goes there: a class with no constructor of its own takes no arguments, whatever
                // its mixin's constructor asks for. Taken in, it was the signature every `new Pager()`
                // was checked against -- three errors on two lines PHP runs without a word -- and
                // the declaration every such `new` was listed as calling.
                if member.row.kind == MemberKind::Method
                    && member.name.as_str().eq_ignore_ascii_case("__construct")
                {
                    continue;
                }
                // What a framework implies is a forwarded *call* -- `Model::__call` hands it to a
                // builder, and `Model::__get` reads an attribute or a relation, never the builder's
                // properties. Taken in, the query builder's public `$from` answered for a model's
                // own `from()` relation, and `$orders`, `$columns` and `$joins` would for any model
                // with a relation of that name.
                if implied && member.row.kind != MemberKind::Method {
                    continue;
                }
                let mut member = member.clone();
                if member.provenance == MemberProvenance::Own {
                    member.provenance = MemberProvenance::Inherited(mixin.fqn.clone());
                }
                member.forwarded = true;
                own.push((member.row.kind, member.name.clone()));
                match displaced {
                    Some(index) => table.replace(index, member),
                    None => Self::insert(table, member),
                }
            }
        }
    }

    /// Interface members are requirements: they lose to any real declaration.
    fn merge_interfaces(&mut self, table: &mut MemberTable, facts: &ClassFacts<'_>) {
        // An interface's own `extends` list is its parents, which `merge_parents` walks. Only a
        // class or enum has a separate implements list to fold in here.
        for interface in facts.implements {
            let inherited = self.flatten(interface.fqn.as_str());
            self.absorb(table, inherited, |provenance| match provenance {
                MemberProvenance::Own => MemberProvenance::Interface(interface.fqn.clone()),
                other => other,
            });
        }
    }

    fn merge_parents(&mut self, table: &mut MemberTable, facts: &ClassFacts<'_>) {
        for parent in facts.extends {
            let inherited = self.flatten(parent.fqn.as_str());
            let from_interface = facts.kind == ClassKind::Interface;

            // Private members are not inherited. A child declaring the same name is not overriding
            // anything, and treating it as an override would report the wrong declaration site.
            let visible = MemberTable::retain_inheritable(inherited);
            self.absorb(table, visible, |provenance| match provenance {
                MemberProvenance::Own if from_interface => {
                    MemberProvenance::Interface(parent.fqn.clone())
                }
                MemberProvenance::Own => MemberProvenance::Inherited(parent.fqn.clone()),
                // A member the parent got from a trait is still declared in that trait, and that
                // is where navigation should land.
                other => other,
            });
        }
    }

    /// Applies each used trait, honouring `insteadof` exclusions and `as` clauses.
    fn merge_traits(&mut self, table: &mut MemberTable, facts: &ClassFacts<'_>) {
        // Each trait's own table is kept, unfiltered, for the alias step below.
        let mut per_trait: Vec<(Name, MemberTable)> = Vec::new();

        // What this class's own `use` list has contributed so far, by member and by the trait that
        // *declared* it. Kept here rather than read back off the table's provenance, because that
        // provenance cannot tell a trait of this class from one a *parent* used: a member the
        // parent got from a trait is still recorded as that trait's, which is right for navigation
        // and wrong for this. Laravel's `UuidUserStub` is the case -- one trait of its own declares
        // `newUniqueId` and the parent got another from a trait, which PHP resolves in the trait's
        // favour and does not call a collision.
        let mut contributed: Vec<(MemberKind, String, Name)> = Vec::new();

        for used in facts.uses {
            let provided = self.flatten(used.fqn.as_str());

            // A missing or cyclic trait has to mark the table incomplete just as a missing parent
            // does. Merging its members without saying so would report a short table as complete.
            if provided.is_incomplete() {
                table.mark_incomplete();
            }
            table.cut(provided.cut_at());
            for contributor in provided.contributors() {
                table.note_contributor(contributor.clone());
            }

            let visible = MemberTable::retain(provided.clone_shallow(), |member| {
                !is_excluded(facts.adaptations, &used.fqn, member)
            });

            // Two traits declaring the same member is fatal in PHP unless something resolves it.
            // Report it and keep the first, rather than inventing a winner the runtime would
            // reject. What counts as one was run against PHP 8.5.10 rather than assumed, and four
            // of its answers are why this is not simply "the name is already here":
            //
            // * the **declaring** trait is what collides, not the one the class named -- two used
            //   traits that both `use Base` are offering one method however many paths reach it;
            // * an **abstract** member is a requirement rather than a body, so it collides with
            //   nothing and the concrete one wins;
            // * the class's **own** declaration settles it, and it is merged after this, so it has
            //   to be asked for here;
            // * a member the **parent** holds does not enter into it at all -- a trait overrides an
            //   inherited method, which is what `contributed` is for.
            for member in visible.members() {
                let declarer = match &member.provenance {
                    MemberProvenance::Trait(fqn) => fqn.clone(),
                    _ => used.fqn.clone(),
                };
                let folded = member.name.as_str().to_ascii_lowercase();
                let already = contributed
                    .iter()
                    .find(|(kind, name, _)| *kind == member.row.kind && *name == folded);
                if let Some((.., first)) = already {
                    let same_declaration = *first == declarer;
                    let requirement = member.row.modifiers.is_abstract()
                        || table
                            .position(member.row.kind, member.name.as_str())
                            .is_some_and(|index| {
                                table.members()[index].row.modifiers.is_abstract()
                            });
                    let settled_here = facts.members.iter().any(|own| {
                        own.kind == member.row.kind
                            && own.name.as_str().eq_ignore_ascii_case(member.name.as_str())
                    });
                    if !same_declaration && !requirement && !settled_here {
                        table.record_conflict(MemberConflict {
                            member: member.name.clone(),
                            kind: member.row.kind,
                            first: first.clone(),
                            second: declarer,
                        });
                        continue;
                    }
                    // A requirement gives way to a body and nothing else moves, which is what every
                    // other arm here does: the first declaration stays.
                    if !(requirement && !member.row.modifiers.is_abstract()) {
                        continue;
                    }
                } else {
                    contributed.push((member.row.kind, folded, declarer));
                }

                let mut member = member.clone();
                member.provenance = match member.provenance {
                    MemberProvenance::Own => MemberProvenance::Trait(used.fqn.clone()),
                    other => other,
                };
                Self::insert(table, member);
            }

            per_trait.push((used.fqn.clone(), provided));
        }

        Self::apply_aliases(table, facts, &per_trait);
    }

    /// `A::m as [visibility] [alias];`
    ///
    /// An alias *adds* a name; it does not rename, so the original stays reachable. A bare
    /// visibility change modifies the exposed member in place.
    ///
    /// When the clause names a trait, the method is taken from **that trait**, not from the merged
    /// table. That is the whole point of the canonical idiom:
    ///
    /// ```php
    /// use Alpha, Beta {
    ///     Alpha::run insteadof Beta;
    ///     Beta::run as legacyRun;
    /// }
    /// ```
    ///
    /// `Beta::run` lost the name `run`, and the alias is how it gets re-exposed. Looking it up in
    /// the merged table would find Alpha's version and report the wrong declaration site.
    fn apply_aliases(
        table: &mut MemberTable,
        facts: &ClassFacts<'_>,
        per_trait: &[(Name, MemberTable)],
    ) {
        for adaptation in facts.adaptations {
            let TraitAdaptationKind::Alias { alias, visibility } = &adaptation.kind else {
                continue;
            };

            let source = match &adaptation.trait_fqn {
                Some(trait_fqn) => per_trait
                    .iter()
                    .find(|(name, _)| name.as_str().eq_ignore_ascii_case(trait_fqn.as_str()))
                    .and_then(|(name, provided)| {
                        let member =
                            provided.find(MemberKind::Method, adaptation.method.as_str())?;
                        let mut member = member.clone();
                        member.provenance = MemberProvenance::Trait(name.clone());
                        Some(member)
                    }),
                // A bare `m as n` is only valid when exactly one used trait declares `m`, so the
                // merged table already holds the right one.
                None => table
                    .find(MemberKind::Method, adaptation.method.as_str())
                    .cloned(),
            };

            let Some(mut member) = source else { continue };
            if let Some(visibility) = visibility {
                member.visibility = *visibility;
            }

            match alias {
                Some(alias) => {
                    member.name = alias.clone();
                    Self::insert(table, member);
                }
                None => Self::insert(table, member),
            }
        }
    }

    fn merge_own(table: &mut MemberTable, facts: &ClassFacts<'_>) {
        for row in facts.members {
            Self::insert(
                table,
                ResolvedMember {
                    name: row.name.clone(),
                    visibility: row.visibility,
                    row: row.clone(),
                    provenance: MemberProvenance::Own,
                    forwarded: false,
                    opened: false,
                    doc_from: None,
                },
            );
        }
    }

    /// Marks the methods a framework says this class's `__call` reopens. After the class's own
    /// members, so the mark lands on whichever declaration the class ends up with -- its own, or the
    /// one it inherits or takes from a trait.
    fn open(table: &mut MemberTable, facts: &ClassFacts<'_>) {
        for name in facts.opened {
            if let Some(index) = table.position(MemberKind::Method, name) {
                let mut member = table.members()[index].clone();
                member.opened = true;
                table.replace(index, member);
            }
        }
    }

    /// Puts the declarations a class's `@method` tags were generated from in place of the tags.
    ///
    /// Laravel's facades: `Cache::remember()` is tagged `@method static mixed remember(string $key,
    /// ..., \Closure $callback)`, written by a script from `Repository::remember()` with its
    /// `@template TCacheValue` and `\Closure(): TCacheValue` dropped -- so what the closure returns
    /// never reached the value. The call runs the repository's method through `__callStatic`, which
    /// is what `forwarded` says, and a tag the source does not declare stays as written.
    fn merge_tag_sources(&mut self, table: &mut MemberTable, facts: &ClassFacts<'_>) {
        for source in facts.tagged_from {
            let flattened = self.flatten_via(source.fqn.as_str(), true);
            table.cut(flattened.cut_at());
            for contributor in flattened.contributors() {
                table.note_contributor(contributor.clone());
            }
            for index in 0..table.members().len() {
                let tagged = &table.members()[index];
                if tagged.row.kind != MemberKind::Method
                    || !tagged.row.from_docblock
                    || tagged.provenance != MemberProvenance::Own
                {
                    continue;
                }
                let Some(real) = flattened.find(MemberKind::Method, tagged.name.as_str()) else {
                    continue;
                };
                if real.visibility != Visibility::Public {
                    continue;
                }
                let mut member = real.clone();
                member.name = tagged.name.clone();
                if member.provenance == MemberProvenance::Own {
                    member.provenance = MemberProvenance::Inherited(source.fqn.clone());
                }
                member.forwarded = true;
                table.replace(index, member);
            }
        }
    }

    /// Folds one already-flattened table into another, rewriting provenance as it goes.
    fn absorb(
        &mut self,
        table: &mut MemberTable,
        source: MemberTable,
        rewrite: impl Fn(MemberProvenance) -> MemberProvenance,
    ) {
        if source.is_incomplete() {
            table.mark_incomplete();
        }
        table.cut(source.cut_at());
        for contributor in source.contributors() {
            table.note_contributor(contributor.clone());
        }
        for member in source.members() {
            let mut member = member.clone();
            member.provenance = rewrite(member.provenance);
            Self::insert(table, member);
        }
    }

    fn insert(table: &mut MemberTable, mut member: ResolvedMember) {
        match table.position(member.row.kind, member.name.as_str()) {
            Some(index) => {
                // A declaration that repeats a signature and not the docblock above it is not
                // withdrawing what the one it overrides promised: that is what `{@inheritdoc}` means,
                // and what every implementation of a documented interface looks like --
                // `ContainerInterface::get()` carries the `class-string` contract and not one of the
                // classes implementing it repeats it.
                //
                // Carried here rather than looked up when read, because reading is the hot path:
                // climbing a hierarchy per member access cost goto p95 0.41 ms -> 0.88 ms, and the
                // table is built once per class per request.
                //
                // By position for the one contract that names a position. An override that reorders
                // its parameters and drops the docblock would carry a claim about the wrong one --
                // which is a thing nobody does, and the alternative is dropping the promise for
                // everybody who writes the override PHP asks for.
                //
                // Only where the override wrote no `@return` of its own, though: one that did made a
                // different promise, and the contract above is about the declaration it replaced.
                // Eloquent's `Collection::map()` returns `Support\Collection<TKey, TMapValue>|
                // static<TKey, TMapValue>`, which is no contract shape -- and carrying the parent's
                // `static<TKey, TMapValue>` over it read every `->get()->map(fn () => [...])` as a
                // collection of models holding arrays.
                let wrote_a_return = (member.row.doc_ty.is_some() && !member.row.doc_ty_from_body)
                    || member.row.doc_ty_refused;
                if member.row.contract.is_none() && !wrote_a_return {
                    member.row.contract = table.members()[index].row.contract.clone();
                }
                // And the same for the type the docblock adds. `public function getFields(): array`
                // under `{@inheritdoc}` is the whole Drupal idiom: the hint says nothing about what
                // the array holds and the `@return Field[]` that does is on the interface.
                // `effective_ty` still ranks it against the override's native hint, so a promise
                // about a different type loses to what the runtime enforces.
                //
                // Only onto an override that declared a native type of its own. One that declared
                // nothing already reaches the overridden declaration through `inherited_ty`, and
                // that path carries *where* the type was written -- which a `@template` cannot do
                // without: `@return Y` means nothing until something says it is `Second`'s `Y` and
                // that `@implements Second<Baz>` filled it in.
                //
                // With where it was written, which a `@template` cannot do without either.
                let above = &table.members()[index];
                if member.row.ty.is_some()
                    && member.row.doc_ty.is_none()
                    && above.row.doc_ty.is_some()
                {
                    member.row.doc_ty = above.row.doc_ty;
                    member.doc_from = above
                        .doc_from
                        .clone()
                        .or_else(|| above.provenance.declarer().cloned());
                }
                Self::carry_param_docs(&mut member, &table.members()[index]);
                // `__call` is inherited with the rest, and runs whichever declaration `$this` has.
                member.opened |= table.members()[index].opened;
                table.replace(index, member);
            }
            None => table.push(member),
        }
    }

    /// Carries the overridden declaration's `@param` types onto an override that wrote none.
    ///
    /// The other half of the rule above, and it was missing for a year while the return half
    /// worked -- the shape §8 calls two constructors of one rule, here with the two halves of one
    /// signature as the two constructors. `FileSystemInterface::deleteRecursive()` writes
    /// `@param string $path` over an untyped parameter and `FileSystem::deleteRecursive()` writes
    /// `{@inheritdoc}` over the same one, which is the Drupal idiom in both directions; a caller
    /// reaching the implementation -- which is what `\Drupal::service('file_system')` hands back --
    /// saw a parameter with no type at all, so `realpath()`'s `false` passed into it unremarked.
    /// Measured against the project's own PHPStan at level 8, this is the largest of the three
    /// causes behind the sentinel arguments it reports and phrost did not.
    ///
    /// By position, for the reason the contract above is: an override that reorders its parameters
    /// and drops the docblock is a thing nobody writes, and refusing to carry would drop the
    /// promise for everybody who writes the override PHP asks for. A parameter that wrote its own
    /// `@param` keeps it -- *written*, not resolved, which is what [`ParamRow::documented`] is for
    /// -- and a native hint keeps ranking against whatever is carried, so this only ever fills a
    /// slot the override left empty.
    ///
    /// The name as well as the position, because an override that renames a parameter is the one
    /// edit that makes the positional match a claim about the wrong value.
    ///
    /// Unlike the return, a parameter with no native type has no `inherited_ty` path to reach the
    /// overridden declaration by, which is why this fills that case too rather than leaving it.
    fn carry_param_docs(member: &mut ResolvedMember, overridden: &ResolvedMember) {
        if member.row.kind != MemberKind::Method {
            return;
        }
        for (position, param) in member.row.params.iter_mut().enumerate() {
            // `documented` and not `doc_ty`, and that is the whole precision of this rule: a
            // parameter whose own `@param` names a template interns to nothing, and reading the
            // empty slot as "said nothing" carries an interface's coarser claim over an
            // implementation's more careful one. See `ParamRow::documented`.
            if param.documented || param.doc_ty.is_some() {
                continue;
            }
            // And only into a slot that is *entirely* empty. A native hint is the declaration
            // saying something about what it takes, and on the input side a narrower claim from
            // above contradicts it rather than refining it -- which is the direction §8 records
            // four times over. Symfony's `BooleanToStringTransformer::transform(mixed $value)`
            // implements a `@param TValue` on a `@template`d interface, and carrying over the
            // `mixed` put a class nothing declares on a parameter its author had opened to
            // everything. The return half may refine a native hint because a return may only get
            // more precise; this one may not, and that asymmetry is the rule rather than a
            // shortfall of it.
            if param.ty.is_some() {
                continue;
            }
            let Some(above) = overridden.row.params.get(position) else {
                break;
            };
            if !param
                .name
                .as_str()
                .eq_ignore_ascii_case(above.name.as_str())
            {
                continue;
            }
            param.doc_ty = above.doc_ty;
        }
    }
}

/// Whether an `insteadof` clause takes this member away from `trait_fqn`.
fn is_excluded(adaptations: &[TraitAdaptation], trait_fqn: &Name, member: &ResolvedMember) -> bool {
    adaptations.iter().any(|adaptation| {
        let TraitAdaptationKind::Precedence { excluded } = &adaptation.kind else {
            return false;
        };

        adaptation
            .method
            .as_str()
            .eq_ignore_ascii_case(member.name.as_str())
            && excluded
                .iter()
                .any(|loser| loser.as_str().eq_ignore_ascii_case(trait_fqn.as_str()))
    })
}

impl MemberTable {
    /// Drops the members a subclass does not inherit.
    ///
    /// Only private ones, and only because PHP says so: a parent's private member is not part of
    /// the child's table, and pretending otherwise makes the child's own declaration look like an
    /// override of something it cannot see.
    fn retain_inheritable(table: Self) -> Self {
        Self::retain(table, |member| member.visibility != Visibility::Private)
    }

    fn retain(mut table: Self, keep: impl Fn(&ResolvedMember) -> bool) -> Self {
        let kept = table
            .members()
            .iter()
            .filter(|member| keep(member))
            .cloned()
            .collect::<Vec<_>>();
        table.set_members(kept);
        table
    }
}
