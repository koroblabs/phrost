//! Where a symbol is declared.
//!
//! The cross-file half of the view, and the half that does not care whether any file is open: a
//! class's location, a member's after inheritance, and what a class-like says about itself. That
//! independence is what makes goto give the same answer either way.

mod call;
mod constant_entry;
mod contracts;
mod filled_return;
mod receiver_out;
mod shape_test;
mod string_test;

pub(crate) use self::call::{ArgumentAssertion, CallFacts, arguments_from};

use std::sync::Arc;

use phrost_model::{Name, short_name};
use phrost_sema::{
    FlattenMemo as _, MemberKind, MemberProvenance, MemberTable, flatten_class_with,
};

use crate::{
    location::{ClassHeader, MemberModifiers, SymbolLocation},
    resolve::{HIERARCHY_DEPTH, Held, RelativeScope},
    view::{IndexView, member_symbol_kind},
};

/// How far to climb looking for the declaration an override inherits its type from.
///
/// A bound, not a guess about depth: Drupal's form hierarchies are six or seven deep with
/// interfaces beside them, and a cycle in a broken file must not be walked forever.
pub(crate) const INHERIT_DEPTH: u8 = 16;

/// What a `@return` tag said about the *shape* of the class a call named.
///
/// Two bits, both written in the tag and neither knowable from the declaration, which says `object`
/// or `array` or `?Node` and nothing about which class: `@return T` is the class,
/// `@return ?T` is the class or null, `@return T[]` is a container of it. Together rather than
/// separately because they compose -- `findInstanceOf` and `findFirstInstanceOf` sit beside each
/// other in one class and differ only in these -- and because a reader that took them one at a time
/// would put the container inside the null on one path and outside it on another.
impl<'a> IndexView<'a> {
    /// Where a class, interface, trait, or enum is declared.
    pub fn class_location(&self, fqn: &str) -> Option<SymbolLocation> {
        self.best_location(self.skeleton.class_declarers(fqn))
    }

    pub fn function_location(&self, fqn: &str) -> Option<SymbolLocation> {
        let key = self.skeleton.interner().lookup_key(fqn)?;
        self.best_location(self.skeleton.owners().function(&key))
    }

    /// Where a global or namespaced constant is declared.
    ///
    /// By name and not by folded key: PHP compares constant names exactly, so `JSON_THROW_ON_ERROR`
    /// and `json_throw_on_error` are two different constants and only one of them exists.
    pub fn constant_location(&self, fqn: &str) -> Option<SymbolLocation> {
        self.best_location(self.skeleton.owners().constant_named(fqn))
    }

    /// What a constant's value states: a type the syntax settled, or the parts one is read off --
    /// see `IndexView::constant_held`.
    pub(crate) fn constant_value(
        &self,
        fqn: &str,
    ) -> Option<&phrost_index::constant::ConstantValue> {
        let winner = *self
            .skeleton
            .rank(self.skeleton.owners().constant_named(fqn))
            .first()?;
        Some(
            self.skeleton
                .file(winner.file)?
                .constant_of(winner.symbol)?
                .value(),
        )
    }

    /// What a free function declares it returns.
    ///
    /// `None` when nothing declares the function, or when it declares no return type -- which is not
    /// `mixed`, and the difference is why this is an `Option` rather than a default.
    pub(crate) fn function_return(&self, fqn: &str) -> Option<phrost_ty::TypeId> {
        let key = self.skeleton.interner().lookup_key(fqn)?;
        let winner = *self
            .skeleton
            .rank(self.skeleton.owners().function(&key))
            .first()?;
        let file = self.skeleton.file(winner.file)?;
        file.function_of(winner.symbol)?
            .effective_ty(self.skeleton.types())
    }

    /// A free function's parameters, for a caller that needs the shape rather than the location.
    pub(crate) fn function_params(&self, fqn: &str) -> Option<Box<[phrost_sema::ParamRow]>> {
        let key = self.skeleton.interner().lookup_key(fqn)?;
        let winner = *self
            .skeleton
            .rank(self.skeleton.owners().function(&key))
            .first()?;
        let file = self.skeleton.file(winner.file)?;
        Some(file.function_of(winner.symbol)?.params().into())
    }

    /// Whether a free function is declared `#[\NoDiscard]`.
    ///
    /// The same lookup as [`Self::function_reads_own_arguments`] and for the same reason: the fact
    /// is on the row the declaration keeps, because a warm start never lowers the file that wrote
    /// it.
    pub(crate) fn function_is_no_discard(&self, fqn: &str) -> bool {
        let Some(key) = self.skeleton.interner().lookup_key(fqn) else {
            return false;
        };
        let Some(&winner) = self
            .skeleton
            .rank(self.skeleton.owners().function(&key))
            .first()
        else {
            return false;
        };
        self.skeleton
            .file(winner.file)
            .and_then(|file| file.function_of(winner.symbol))
            .is_some_and(|function| function.is_no_discard())
    }

    /// Whether a free function's body reads arguments its signature does not name.
    ///
    /// See `phrost_sema::MemberRow::reads_own_arguments`; the same fact, on the row a function
    /// declaration keeps.
    pub(crate) fn function_reads_own_arguments(&self, fqn: &str) -> bool {
        let Some(key) = self.skeleton.interner().lookup_key(fqn) else {
            return false;
        };
        let Some(&winner) = self
            .skeleton
            .rank(self.skeleton.owners().function(&key))
            .first()
        else {
            return false;
        };
        self.skeleton
            .file(winner.file)
            .and_then(|file| file.function_of(winner.symbol))
            .is_some_and(|function| function.reads_own_arguments())
    }

    /// Every member a class has, after inheritance and trait application.
    ///
    /// Memoized for the life of this view. Flattening walks every parent, trait, and interface of a
    /// class, and one request asks about the same few classes over and over -- a pass that checks
    /// every member access in a file asks about `$this` once per access. Shared rather than cloned,
    /// because a Drupal class's flattened table runs to hundreds of members.
    pub fn members(&self, class_fqn: &str) -> Arc<MemberTable> {
        // As written first, which needs no new string: a member lookup is asked for every access
        // in a file and folding the name allocated once per ask.
        if let Some(table) = self.flattened_as_written.borrow().get(class_fqn) {
            return Arc::clone(table);
        }
        let table = self.members_folded(class_fqn);
        self.flattened_as_written
            .borrow_mut()
            .insert(class_fqn.into(), Arc::clone(&table));
        table
    }

    /// [`Self::members`] by the folded name, which is the table's identity.
    fn members_folded(&self, class_fqn: &str) -> Arc<MemberTable> {
        // Folded, because PHP looks classes up case-insensitively and two spellings must not build
        // the table twice.
        let key: Box<str> = class_fqn.to_ascii_lowercase().into();

        // Borrowed and released before flattening, which re-enters this view through nothing today
        // and must not be able to deadlock if it ever does.
        let hit = self.flattened.borrow().get(&key).map(Arc::clone);
        if let Some(table) = hit {
            return table;
        }
        // Then what an earlier request built, which the skeleton drops the moment a class it was
        // built from changes.
        if let Some(table) = self.skeleton.flattened(&key) {
            self.flattened.borrow_mut().insert(key, Arc::clone(&table));
            return table;
        }

        // The flattener offers every table it builds on the way -- this class's and each ancestor's
        // -- so the next class above the same base pays for its own members and nothing else.
        let table = Arc::new(flatten_class_with(self.skeleton, class_fqn, self.skeleton));
        self.flattened.borrow_mut().insert(key, Arc::clone(&table));
        table
    }

    /// Where a member is declared, following it back to the class or trait that declared it.
    ///
    /// The declaration site is the point: a method inherited from a grandparent or pulled in from a
    /// trait should navigate to where it was written, not to the class that was asked about.
    pub fn member_location(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<SymbolLocation> {
        // `Foo::class` is a magic constant every class-like has and none declares. What it names is
        // the class, which is also where goto should land and what hover should describe.
        if kind == MemberKind::ClassConstant && name.eq_ignore_ascii_case("class") {
            return self.class_location(class_fqn);
        }

        let table = self.members(class_fqn);
        // A name no declaration answers for may still be one the class's `__get` does. PHP's own
        // order -- see [`IndexView::magic_property_location`].
        let Some(member) = table.find(kind, name) else {
            return self
                .magic_property_location(class_fqn, kind, name)
                .or_else(|| self.declared_table_location(class_fqn, kind, name))
                .or_else(|| self.method_backed_location(class_fqn, kind, name))
                .or_else(|| self.accessor_location(class_fqn, kind, name))
                .or_else(|| self.stated_location(class_fqn, kind, name))
                .or_else(|| self.prefixed_method_location(class_fqn, kind, name))
                .or_else(|| self.macro_location(class_fqn, kind, name));
        };

        let declaring = match &member.provenance {
            MemberProvenance::Own => class_fqn,
            MemberProvenance::Trait(fqn)
            | MemberProvenance::Inherited(fqn)
            | MemberProvenance::Interface(fqn) => fqn.as_str(),
        };
        let owner = self.class_location(declaring)?;

        Some(SymbolLocation {
            file: owner.file,
            fqn: member.row.name.clone(),
            // The kind that was found, not the kind that was asked for: `Stage::Shipped` is asked
            // about as a constant and is a case, and describing it as a constant would put `const`
            // in front of something PHP writes as `case`.
            kind: member_symbol_kind(member.row.kind),
            name_range: member.row.name_range,
            full_range: member.row.full_range,
            container: Some(owner.fqn),
            from_docblock: member.row.from_docblock,
            deprecated: member.row.modifiers.is_deprecated(),
            signature: Some(member.row.signature(self.skeleton.types()).into()),
            member_modifiers: Some(MemberModifiers {
                visibility: member.visibility,
                set_visibility: member.row.set_visibility,
                is_static: member.row.modifiers.is_static(),
                is_abstract: member.row.modifiers.is_abstract(),
                is_final: member.row.modifiers.is_final(),
                is_readonly: member.row.modifiers.is_readonly(),
            }),
            // Omitted when the member is the class's own: "own" is the default a reader already
            // assumes, and a hover line that says nothing new is a line they learn to skip.
            origin: match &member.provenance {
                MemberProvenance::Own => None,
                other => Some(other.label().into()),
            },
            docblock_range: member.row.docblock_range,
        })
    }

    /// The class a member of `class_fqn` holds, from its declared type.
    ///
    /// A property's declared type or a method's return type -- the same stored field, because the
    /// question is the same: what does reading this give you. Already resolved to a fully-qualified
    /// name at ingest, so this is a lookup rather than a second resolution.
    pub fn member_class(&self, class_fqn: &str, kind: MemberKind, name: &str) -> Option<Name> {
        let held = self.member_held(class_fqn, kind, name)?;
        match &held {
            Held::Object(_) => None,
            Held::Stored { ty, scope } => self.class_of_stored(*ty, scope.as_ref()),
            Held::Class(name) | Held::Applied { class: name, .. } => Some(name.clone()),
            // A declaration's stored type is never one of these -- every one is produced while
            // walking a body, and this reads a declaration.
            Held::Builtin(_)
            | Held::Written(_)
            | Held::Nullable(_)
            | Held::ListOf(_)
            | Held::MapOf { .. }
            | Held::Record(_)
            | Held::Callable(_)
            | Held::OneOf(_)
            | Held::AllOf(_) => None,
        }
    }

    /// The class a member's *element* names, when the member's own type names none.
    ///
    /// `@var Rule[]` on a property names no class -- there is no `Rule[]` to jump to -- and the only
    /// declaration a reader asking where this type is written could want is `Rule`. The same rule
    /// [`VariableInfo::element_class`](crate::VariableInfo::element_class) states for a local, and
    /// it is asked only where the type is not a class itself: a class that iterates is still the
    /// answer to "where is this type declared".
    pub fn member_element_class(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<Name> {
        if self.member_class(class_fqn, kind, name).is_some() {
            return None;
        }
        let Held::Stored { ty, scope } = self.member_held(class_fqn, kind, name)? else {
            return None;
        };
        let element = self.element_of_stored(ty)?;
        self.class_of_stored(element, scope.as_ref())
    }

    /// Whether `class_fqn` declares this member itself rather than inheriting it.
    ///
    /// The difference between an override and the thing being overridden. Without it, asking which
    /// classes implement an interface method answers with the interface's own declaration once per
    /// subclass -- because that is where an inherited member is declared, and [`Self::member_location`]
    /// says so correctly.
    ///
    /// Read off the class's own rows, which are exactly what a flattened table marks as its own,
    /// rather than off the flattened table: a walk down a hierarchy asks this of every subtype, and
    /// flattening each of Laravel's models -- hundreds of members inherited from `Model` -- to learn
    /// whether one line is in its body was most of a 500 ms `prepareRename`.
    pub fn declares_member(&self, class_fqn: &str, kind: MemberKind, name: &str) -> bool {
        let Some(location) = self.class_location(class_fqn) else {
            return false;
        };
        let Some(class) = self.skeleton.file(location.file).and_then(|file| {
            file.classes()
                .iter()
                .find(|class| class.fqn() == &location.fqn)
        }) else {
            return false;
        };
        class.members().iter().any(|row| {
            row.kind.shares_namespace_with(kind)
                && match kind.is_case_insensitive() {
                    true => row.name.as_str().eq_ignore_ascii_case(name),
                    false => row.name.as_str() == name,
                }
        })
    }

    /// The classes a member of `class_fqn` holds, which is more than one when its declared type is
    /// an intersection.
    ///
    /// `private Application&MockObject $application` is how every mock in a modern PHPUnit test is
    /// declared, and the value has both APIs -- so a list of what can be reached through it is the
    /// two tables together rather than either one.
    pub fn member_classes(&self, class_fqn: &str, kind: MemberKind, name: &str) -> Vec<Name> {
        let Some(held) = self.member_held(class_fqn, kind, name) else {
            return Vec::new();
        };
        match &held {
            Held::Object(_) => Vec::new(),
            Held::Stored { ty, scope } => match self.class_of_stored(*ty, scope.as_ref()) {
                Some(one) => vec![one],
                None => self.stored_intersection(*ty, scope.as_ref()),
            },
            Held::Class(name) | Held::Applied { class: name, .. } => vec![name.clone()],
            Held::Builtin(_)
            | Held::Written(_)
            | Held::Nullable(_)
            | Held::ListOf(_)
            | Held::MapOf { .. }
            | Held::Record(_)
            | Held::Callable(_)
            | Held::OneOf(_)
            | Held::AllOf(_) => Vec::new(),
        }
    }

    /// The type a member holds, whichever declaration said so, with the context `self` and `static`
    /// need.
    ///
    /// The type rather than the class, because a chain may go on to ask what iterating it yields --
    /// and `array<Repo>` names no class while saying exactly that.
    pub(crate) fn member_held(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<Held> {
        self.member_held_on(class_fqn, kind, name, None, &[])
    }

    /// The class a framework forwards `class_fqn`'s call to `name` to, and the arguments that
    /// instance is created with, when the member was reached through an implied `@mixin`.
    ///
    /// Read off the classes the table was built from, since the forwarding is declared on an
    /// ancestor -- `Model` -- and inherited by every class below it.
    pub(crate) fn implied_forward(
        &self,
        class_fqn: &str,
        called_on: Option<&Name>,
        kind: MemberKind,
        name: &str,
    ) -> Option<(Name, Vec<Held>)> {
        self.implied_forward_to(class_fqn, called_on, |mixin| {
            self.members(mixin).find(kind, name).is_some()
        })
    }

    /// The same, for a call no member table answers: the mixin is the first one `answers` accepts.
    pub(crate) fn implied_forward_to(
        &self,
        class_fqn: &str,
        called_on: Option<&Name>,
        answers: impl Fn(&str) -> bool,
    ) -> Option<(Name, Vec<Held>)> {
        let returns = self.skeleton.returns();
        let types = self.skeleton.types();
        self.members(class_fqn)
            .contributors()
            .iter()
            .find_map(|contributor| {
                let (mixins, arguments) = returns.implied_mixins_of(contributor.as_str());
                // Never to a class the value already is. A contract bound to its own
                // implementation -- `Cache\Repository` and the repository that implements it --
                // reaches the implementation's contributors again from inside it, and a member the
                // implementation itself forwards (`lock()`, through its own `@mixin`) was then
                // forwarded to the same class until the stack ran out.
                let mixin = mixins.iter().find(|mixin| {
                    !class_fqn.eq_ignore_ascii_case(mixin.fqn.as_str())
                        && !self.descends_from(
                            class_fqn,
                            mixin.fqn.as_str(),
                            crate::resolve::HIERARCHY_DEPTH,
                        )
                        && answers(mixin.fqn.as_str())
                })?;
                // Parameterised by the class the call was made on, which is the only argument a
                // framework states: `Builder<static>`.
                let with_static = arguments.iter().any(|&id| {
                    matches!(types.get(id), phrost_ty::TypeRef::Generic { base, .. }
                    if matches!(types.get(*base), phrost_ty::TypeRef::Named(named)
                        if named.as_str().eq_ignore_ascii_case(mixin.fqn.as_str())))
                });
                let arguments = match with_static {
                    true => vec![Held::Class(match called_on {
                        Some(called) => called.clone(),
                        None => self.members_owner(class_fqn)?,
                    })],
                    false => Vec::new(),
                };
                Some((mixin.fqn.clone(), arguments))
            })
    }

    /// Whether a stored type says only that the value is an object.
    ///
    /// Exactly the builtin and not a nullable or a union holding one: `?object` still says the
    /// value may be absent, which a body cannot take back.
    fn names_only_object(&self, ty: phrost_ty::TypeId) -> bool {
        matches!(
            self.skeleton.types().get(ty),
            phrost_ty::TypeRef::Builtin(phrost_model::BuiltinTy::Object)
        )
    }

    /// The same, told which class the call was made on when that is not the class the member was
    /// found through.
    ///
    /// `called_on` is PHP's late static binding, and it differs from `class_fqn` for exactly one
    /// spelling: `parent::m()` looks the member up on the parent while `static` still means the class
    /// the code is written in.
    pub(crate) fn member_held_on(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
        called_on: Option<Name>,
        arguments: &[Held],
    ) -> Option<Held> {
        let table = self.members(class_fqn);
        let Some(member) = table.find(kind, name) else {
            return self
                .magic_property_held(class_fqn, kind, name)
                .or_else(|| match kind {
                    MemberKind::Property => self.declared_table_held(class_fqn, name),
                    _ => None,
                })
                .or_else(|| self.method_backed_held(class_fqn, kind, name))
                .or_else(|| self.accessor_held(class_fqn, kind, name))
                .or_else(|| self.stated_held(class_fqn, kind, name))
                .or_else(|| self.prefixed_method_held(class_fqn, kind, name, arguments))
                .or_else(|| self.fluent_held(class_fqn, kind, name, called_on.as_ref(), arguments))
                // Before `__call`, which is what runs a macro and says nothing about any one of them:
                // Laravel's `Macroable::__call` is declared `mixed`.
                .or_else(|| self.macro_held(class_fqn, kind, name, called_on.as_ref()))
                .or_else(|| self.magic_accessor_held(class_fqn, kind, name));
        };

        // A call a framework forwards to a fresh instance of another class answers what that class's
        // method does, on that instance: `User::where(...)` is `Builder<User>::where(...)`, so its
        // `$this` is the builder and not the model. See `ImpliedMixin`.
        if member.forwarded
            && let Some((mixin, arguments)) =
                self.implied_forward(class_fqn, called_on.as_ref(), kind, name)
        {
            return self.member_held_on(mixin.as_str(), kind, name, None, &arguments);
        }

        // An enum case is an instance of its enum. PHP's rule, and the one member whose type no
        // declaration writes -- so `effective_ty` has nothing to give and would end the chain.
        if member.row.kind == MemberKind::EnumCase {
            return Some(Held::Class(self.members_owner(class_fqn)?));
        }

        // Who wrote the declaration decides what `self` means. A trait's method behaves as if it had
        // been written in the composing class, which is PHP's rule and not an approximation of it.
        let mut declaring = match &member.provenance {
            MemberProvenance::Own | MemberProvenance::Trait(_) => None,
            MemberProvenance::Inherited(fqn) | MemberProvenance::Interface(fqn) => {
                Some(fqn.clone())
            }
        };

        // A type read off the *body* is an implementation detail of whatever the declaration says,
        // and an override with no declaration of its own inherits one. So the climb happens first
        // there, and only what nothing above declares falls back to the body.
        //
        // Drupal's `Select::execute()` is the shape: `SelectInterface::execute()` declares
        // `@return StatementInterface|null` and the concrete class overrides it with no docblock,
        // so reading the body answered the non-nullable thing it happens to hand back -- and every
        // `$query->execute()->fetchAll()` read as safe. PHPStan reports 97 of them on one checkout.
        let from_body = member.row.doc_ty_from_body;
        let declared = self.declared_ty(&member.row).filter(|_| !from_body);

        // `object` is the widest object type PHP has, so a class the body *describes* is strictly
        // more precise and nothing is given up by preferring it. `refined` already ranks a docblock
        // above an `object` hint for exactly that reason; this is the same question asked for the
        // third source, and the declaration is kept where the body describes nothing.
        //
        // Drupal is written this way: `/** @return object */ protected function
        // utilitiesManager(): object { return \Drupal::service('utilities.manager'); }`. Lowering
        // cannot name the class -- it is in the service catalog, which is this layer's -- so the
        // body is stored as the chain it returns and read back here. Before this,
        // `$this->utilitiesManager()->` resolved to nothing at all.
        if declared.is_some_and(|ty| self.names_only_object(ty))
            && let Some(found) = self.returned_by_body(class_fqn, kind, name, called_on.clone())
        {
            return Some(found);
        }

        // A generic class a framework release declared bare where later ones write its arguments:
        // Laravel 11's `Builder::paginate()`. See `FilledReturn`. Before the body, which for
        // `simplePaginate()` hands back the concrete `Paginator` of nothing in particular.
        if let Some(ty) = declared
            && let phrost_ty::TypeRef::Named(hinted) = self.skeleton.types().get(ty)
            && let Some(filled) =
                self.filled_return(class_fqn, declaring.as_ref(), name, hinted, arguments)
        {
            return Some(filled);
        }

        // A generic class named bare -- `author(): BelongsTo` -- says what the value is and not what
        // it holds, and a body handing back that class with its arguments filled says the rest. Only
        // that: the hint is a guarantee, so a body describing anything else is ignored rather than
        // allowed to contradict it.
        //
        // A subclass says it too, through its own `@extends`: Laravel's factory stubs wrote `@return
        // \Illuminate\Database\Eloquent\Factories\Factory` over every state method, whose body is
        // `return $this->state(...)` -- a `static`, so on a `UserFactory` the `UserFactory` that
        // `@extends Factory<User>`. Read as the bare `Factory` it was a factory of `Model`, and every
        // `User::factory()->unverified()->create()` in Monica's tests was a dynamic model.
        if let Some(ty) = declared
            && let phrost_ty::TypeRef::Named(hinted) = self.skeleton.types().get(ty)
            && self
                .class_templates(hinted.as_str())
                .is_some_and(|templates| !templates.is_empty())
            && let Some(found) = self.returned_by_body(class_fqn, kind, name, called_on.clone())
            && let Some(built) = match &found {
                Held::Applied { class, .. } => Some(class.clone()),
                // A subclass, in whichever spelling the step handed back: a `static` arrives
                // stored, with the class it was reached through as its scope.
                Held::Class(class) => Some(class.clone()),
                Held::Stored { ty, scope } => self.class_of_stored(*ty, scope.as_ref()),
                _ => None,
            }
            .filter(|built| {
                matches!(found, Held::Applied { .. })
                    || !built.as_str().eq_ignore_ascii_case(hinted.as_str())
            })
            && (built.as_str().eq_ignore_ascii_case(hinted.as_str())
                || self.descends_from(built.as_str(), hinted.as_str(), HIERARCHY_DEPTH))
        {
            return Some(found);
        }

        let ty = match declared {
            Some(ty) => ty,
            None => match self.inherited_ty(class_fqn, kind, name, INHERIT_DEPTH) {
                Some((inherited, from)) => {
                    // The type came from further up, so that is where a `self` inside it was
                    // written.
                    declaring = Some(from);
                    inherited
                }
                // Nothing written anywhere above it either. What is left is the body, which said
                // what it hands back by handing it back -- see `returned_by_body`. Last, so a
                // declaration that states its type is never second-guessed by its own code.
                None => match self.declared_ty(&member.row) {
                    // Read off this body at ingest, and nothing above it disagreed.
                    Some(ty) if from_body => ty,
                    _ => return self.returned_by_body(class_fqn, kind, name, called_on),
                },
            },
        };
        // A `@template` of whoever declared the member says nothing on its own: it is a name that
        // resolves to no class, and what it means is whatever the class it was reached through
        // supplied for it. When nothing did -- no argument on the value, no `@extends` between the
        // two, no bound on the template -- the honest answer is nothing rather than a class called
        // `T`, which is what a reader would otherwise be shown members of.
        // The class that wrote the declaration, which for a member of its own is the class asked
        // about. Both need the substitution: an own `@return T` has no argument to find and falls
        // to the template's bound.
        // Two different questions have been one field here, and for a trait they part company. What
        // `self` means is the composing class, which is PHP's rule and why a trait's member has no
        // `declaring`. Whose `@template` a name refers to is the *trait*, because that is where the
        // tag is written -- and what the class supplied for it is on its own `@use Holder<Widget>`,
        // which the substitution walk already follows. Reading both off one name meant a templated
        // trait member answered nothing at all, while the identical `@extends` shape answered.
        let owner = declaring.as_ref().map_or(class_fqn, Name::as_str);
        let owner = match &member.provenance {
            MemberProvenance::Trait(fqn) => fqn.as_str(),
            _ => owner,
        };
        // A docblock the override carried from above was written there, so its templates are that
        // class's, and it is that class an `@implements Getter<Foo>` supplies them to. Only when the
        // carried docblock is what the declaration reads: a hint that won is the override's own.
        let owner = match &member.doc_from {
            Some(from) if declared.is_some() && declared == member.row.doc_ty => from.as_str(),
            _ => owner,
        };
        // The class the member table belongs to, which is where a member of its own was written --
        // never the class the call was made on. They differ for `parent::m()`, and reading `called`
        // there made `Base::m(): self` called from a `Child` answer `Child`.
        let found_on = self.members_owner(class_fqn)?;
        let scope = RelativeScope {
            declaring: declaring.clone().unwrap_or_else(|| found_on.clone()),
            called: called_on.clone().unwrap_or_else(|| found_on.clone()),
        };

        // A docblock naming one of the owner's templates beats the hint written beside it, which is
        // a choice `TypeTable::refined` cannot make: a template name is a `Named` and looks like a
        // class to it. The hint is kept as `written`, because nothing may supply the template and
        // what the declaration wrote is then a real type and the better answer.
        //
        // `ty` is already the effective type, which prefers a docblock over `mixed` -- so where the
        // docblock is a bare template, keeping it kept the template itself, and a `@return T` beside
        // `: mixed` rendered as a variable called `T` on every receiver that supplied nothing. The
        // native hint is what was written there. A template *inside* a container stays: `@return
        // Iterator<int, T>` is iterated to the template's bound further on, which the bare hint
        // would lose.
        let (ty, written) = match self.template_return(owner, &member.row) {
            Some(doc) => {
                let bare = match self.skeleton.types().get(ty) {
                    phrost_ty::TypeRef::Nullable(inner) => *inner,
                    _ => ty,
                };
                match self.template_named(owner, bare) {
                    Some(_) => (doc, member.row.ty),
                    None => (doc, Some(ty)),
                }
            }
            None => (ty, None),
        };

        let (ty, nullable) = match self.declared_through(class_fqn, owner, ty, arguments, &scope) {
            Declared::Held(held) => return Some(held),
            Declared::Type { ty, nullable } => (ty, nullable),
            // Nothing supplied it. What the declaration wrote beside the tag is a real type and is
            // what a reader had before the tag was preferred; only where there is no hint at all is
            // the honest answer nothing rather than a class called `T`.
            Declared::Unbound => (written?, false),
        };

        let held = Held::Stored {
            ty: self.contracted(class_fqn, kind, name, ty),
            scope: Some(scope),
        };
        Some(match nullable {
            true => Self::nullable(held),
            false => held,
        })
    }

    /// A declared type as a value of `class_fqn` holds it, with `owner`'s templates put in place.
    ///
    /// One ladder for every reader of a declaration written in a supertype -- a member's type and an
    /// inherited parameter's. The parameter had none: an override's untyped `$value` under the
    /// interface's `@param TSet|null $value` read as a class called `TSet`, and an `@implements
    /// CastsAttributes<Address>` above it never reached the body.
    pub(crate) fn declared_through(
        &self,
        class_fqn: &str,
        owner: &str,
        ty: phrost_ty::TypeId,
        arguments: &[Held],
        scope: &RelativeScope,
    ) -> Declared {
        if let Some(substituted) =
            self.substituted_by_receiver(class_fqn, owner, ty, arguments, scope)
        {
            return Declared::Held(substituted);
        }

        let under_null = match self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Nullable(inner) => Some(*inner),
            _ => None,
        };
        // Under the null first: nothing substituted there means the type stands as written, nulls
        // and all -- unless what stands is a template name, which is no answer at all.
        if let Some(substituted) =
            under_null.and_then(|inner| self.substituted_template(class_fqn, owner, inner))
        {
            return Declared::Type {
                ty: substituted,
                nullable: true,
            };
        }
        match self.substituted_template(class_fqn, owner, ty) {
            Some(substituted) => Declared::Type {
                ty: substituted,
                nullable: false,
            },
            None if self.mentions_template(owner, under_null.unwrap_or(ty)) => Declared::Unbound,
            None => Declared::Type {
                ty,
                nullable: false,
            },
        }
    }

    /// What a declared type becomes once the receiver's own arguments are put in it: the receiver
    /// handed back, a container of a template, a union holding one, the template itself, or a class
    /// carrying it -- each under a declared null.
    ///
    /// One ladder for both readers of a declaration. The conditional-return reader had a copy that
    /// knew only the bare template, so `@return ($id is array ? list<TModel> : TModel|null)` read
    /// `?TModel` on every receiver that supplied `TModel`, while a plain `@return TModel|null` beside
    /// it answered.
    pub(crate) fn substituted_by_receiver(
        &self,
        class_fqn: &str,
        owner: &str,
        ty: phrost_ty::TypeId,
        arguments: &[Held],
        scope: &RelativeScope,
    ) -> Option<Held> {
        // `static`, `self` and `$this` hand the receiver back, so what comes out holds whatever
        // went in. Before every substitution below, none of which can see it: a relative word is
        // not a template name, and resolving it to a class threw the arguments away -- which is how
        // `$collection->filter(...)->first()` answered `object` on the fluent shape every framework
        // is written in.
        if let Some(receiver) = self.the_receiver_back(ty, scope, class_fqn, arguments) {
            return Some(receiver);
        }

        // A container of a template, whose base is a builtin and so has no class to hang the
        // argument off: `@return T[]` and `@return list<T>`. Before the union, because a `?T[]` is a
        // nullable around this rather than a union of templates.
        if let Some(listed) = self.list_of_supplied(class_fqn, owner, ty, arguments) {
            return Some(listed);
        }

        // A union with a template in it, which every substitution below declines: a union is not a
        // template name. `@return TDefault|TInput` is 38 of one vendor tree's templated returns.
        //
        // Only when there *is* a template in it. Asked of every union, this claimed the answer for
        // `PDOStatement|false` too -- rebuilding it operand by operand into the same thing and
        // returning before `contracted` below, which is where a promise that a declared sentinel
        // never arrives is applied. That made the contract dead for every union-typed return, and
        // nothing showed it while the only contracts written were about a `?T`.
        if self.mentions_template(owner, ty)
            && let Some(substituted) =
                self.substituted_union(class_fqn, owner, ty, arguments, scope)
        {
            return Some(substituted);
        }

        // An object shape whose properties name a template, which is a template the value fills:
        // a branch of `BelongsToMany::findOrFail()` reaches here an intersection operand at a time,
        // and the shape kept whole put a `TPivotModel` nothing declares in the message about it.
        if let phrost_ty::TypeRef::ObjectShape { fields } = self.skeleton.types().get(ty)
            && self.mentions_template(owner, ty)
        {
            return Some(self.substituted_object(class_fqn, owner, *fields, arguments, scope));
        }

        // Under the null, when there is one. `@return ?T` is a swap for `T` with a null the
        // declaration added on top, and a reader has nowhere to put a rebuilt `?Foo` -- the table it
        // would have to intern into is open for reading only, which is what `Held::Nullable` is for.
        let under_null = match self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Nullable(inner) => Some(*inner),
            _ => None,
        };
        // What the value itself carries, before anything its class declares. Returned as it stands:
        // an argument is a concrete type, so there is no template left in it to substitute.
        if let Some(supplied) =
            self.supplied_argument_at(class_fqn, owner, under_null.unwrap_or(ty), arguments)
        {
            return Some(match under_null.is_some() {
                true => Self::nullable(supplied),
                false => supplied,
            });
        }
        // The same argument, one level in: `@return Collection<T>` puts the value's `T` inside a
        // container rather than handing it back, and what comes out is an instance of that container
        // carrying it. See [`IndexView::applied_with`].
        if let Some(applied) = self.applied_with(
            class_fqn,
            owner,
            under_null.unwrap_or(ty),
            arguments,
            Some(scope),
        ) {
            return Some(match under_null.is_some() {
                true => Self::nullable(applied),
                false => applied,
            });
        }
        // Two hops, where an ancestor's template was filled with a type written in the receiver's
        // own: `Relation::getResults(): TResult`, which `HasMany<TRelatedModel, ...>` fills with
        // `Collection<int, TRelatedModel>` -- so on a `HasMany<Comment, Post>` the substitution
        // above lands on that, and the receiver's `Comment` has to go into it second. Stopping at the
        // first hop answered `Collection<int, TRelatedModel>`, a container of a class nothing
        // declares, for every relation Laravel has.
        //
        // The argument as written, unlike `substituted_template`, which falls back to a bound when
        // what it finds names nothing real -- and what an ancestor was filled with may name nothing
        // real until the receiver's arguments are in it: `BelongsTo` fills `TResult` with
        // `?TRelated`, which is a `?User` one step later.
        if !owner.eq_ignore_ascii_case(class_fqn)
            && let Some(position) = self.template_named(owner, under_null.unwrap_or(ty))
            && let Some(through) = self.type_argument(
                class_fqn,
                owner,
                position,
                crate::generics::CHASE,
                crate::generics::Landing::Bound,
            )
            && self.mentions_template(class_fqn, through)
            && let Some(held) =
                self.substituted_by_receiver(class_fqn, class_fqn, through, arguments, scope)
        {
            return Some(match under_null.is_some() {
                true => Self::nullable(held),
                false => held,
            });
        }
        None
    }

    /// The receiver itself, for a method declared to hand it back.
    ///
    /// `static`, `self` and `@return $this` all mean *this very object*, so a `Box<Row>` that went
    /// in is a `Box<Row>` that comes out -- and resolving the word to a class name, which is all
    /// [`IndexView::class_of_relative`] can do, dropped the arguments on the floor. Every fluent
    /// interface is written this way: `$query->where(...)->orderBy(...)`, `$collection->filter(...)`,
    /// PSR-7's `withHeader`. One step through any of them and what the container held was gone.
    ///
    /// Only when the word names **the very class the arguments belong to**. They are what the value
    /// supplied for `class_fqn`'s own templates, and `parent` -- or a `self` written in a supertype
    /// -- names a different set of them, which the value says nothing about. A miss there rather
    /// than a guess, which is the direction every substitution beside this one takes.
    pub(crate) fn the_receiver_back(
        &self,
        ty: phrost_ty::TypeId,
        scope: &RelativeScope,
        class_fqn: &str,
        arguments: &[Held],
    ) -> Option<Held> {
        if arguments.is_empty() {
            return None;
        }
        // `@return ?static` is written, and the null belongs to the declaration rather than to what
        // the receiver held.
        let (ty, nullable) = match self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Nullable(inner) => (*inner, true),
            _ => (ty, false),
        };
        let phrost_ty::TypeRef::Relative(relative) = self.skeleton.types().get(ty) else {
            return None;
        };
        let named = self.class_of_relative(*relative, scope)?;
        if !named.as_str().eq_ignore_ascii_case(class_fqn) {
            return None;
        }
        let held = Held::Applied {
            class: named,
            arguments: arguments.to_vec().into_boxed_slice(),
        };
        Some(match nullable {
            true => Self::nullable(held),
            false => held,
        })
    }

    /// The type a declaration states, with a docblock that names a *subclass* preferred over the
    /// native hint.
    ///
    /// PHPStan's rule, and it is load-bearing on framework code:
    ///
    /// ```php
    /// /** @var \Drupal\app_core\Entity\User|null */
    /// protected ?UserInterface $user = NULL;
    /// ```
    ///
    /// The hint is the interface the framework hands out and the docblock is the author saying which
    /// class the value really is. Reading the hint instead reports every call to the subclass's own
    /// API as undefined -- ten of the forty-three undefined-method findings on one real project were
    /// this one property.
    ///
    /// Only downward. A docblock naming a class the hint's class has no relation to is a
    /// disagreement rather than extra information, and there the hint wins: it is what the runtime
    /// enforces.
    fn declared_ty(&self, row: &phrost_sema::MemberRow) -> Option<phrost_ty::TypeId> {
        let chosen = row.effective_ty(self.skeleton.types())?;
        let (Some(native), Some(doc)) = (row.ty, row.doc_ty) else {
            return Some(chosen);
        };
        // Interning makes this the whole of the common case: a docblock that restates the hint is
        // the same id, and most docblocks beside a typed declaration restate it.
        if native == doc || chosen != native {
            return Some(chosen);
        }

        // `None` for the scope, so a relative type on either side declines rather than being
        // resolved against a class this does not know it was reached through. Declining keeps what
        // the declaration already gave: answering `None` here would read as "this member has no
        // type", which sends the caller climbing to the interface above -- and `self` in an
        // interface's own `: self` means the interface, so a whole fluent chain lands one level too
        // high. That is what happened the first time this was written.
        let (Some(doc_class), Some(native_class)) = (
            self.class_of_stored(doc, None),
            self.class_of_stored(native, None),
        ) else {
            return Some(chosen);
        };
        match self.descends_from(doc_class.as_str(), native_class.as_str(), HIERARCHY_DEPTH) {
            true => Some(doc),
            false => Some(chosen),
        }
    }

    /// The interned name of a class, for putting in a scope rather than for locating it.
    pub(crate) fn members_owner(&self, class_fqn: &str) -> Option<Name> {
        self.class_location(class_fqn).map(|location| location.fqn)
    }

    /// What a boolean method's docblock proves about its receiver, when it returns `when_true`.
    ///
    /// Looked up through the flattened table, so a tag written on an interface answers for every
    /// class that implements it -- which is where these are written: `Application::isBulkOrder`
    /// carries one and seven of its siblings do too.
    pub(crate) fn member_assertion(
        &self,
        class_fqn: &str,
        method: &str,
        when_true: bool,
    ) -> Option<phrost_ty::TypeId> {
        let table = self.members(class_fqn);
        let row = &table.find(MemberKind::Method, method)?.row;
        let phrost_sema::Contract::Receiver {
            ty,
            when_true: proves,
        } = row.contract.as_deref()?
        else {
            return None;
        };

        // A tag says what one return value proves. The other branch of the same `if` proves nothing
        // -- not the negation, which names no class.
        (*proves == when_true).then_some(*ty)
    }

    /// A member's declaration rendered with the type *resolution* would answer, or `None` when its
    /// own declaration already says.
    ///
    /// An override that states no return is not saying the return is unknown -- it is saying it has
    /// not changed, which is what `{@inheritdoc}` means and how most of Drupal's API is written. So
    /// `member_held_on` climbs for it, and a card that did not showed
    /// `public function get($field_name)` at a site where go-to-type-definition landed on
    /// `FieldItemListInterface`: one declaration described two ways.
    ///
    /// A type read off the *body* does not stop the climb either, for the reason `member_held_on`
    /// gives -- `SelectInterface::execute()` declares `@return StatementInterface|null` and the
    /// concrete class overrides it with no docblock, so the body's non-nullable answer is the one
    /// thing a reader must not be shown.
    ///
    /// For hover, which describes one member per request and can afford the climb. The signature a
    /// completion list renders is the declaration's own and stays that way: a hundred climbs per
    /// keystroke is what the flattener's note about carrying contracts was written to avoid.
    pub fn resolved_signature(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
        called_on: Option<&str>,
    ) -> Option<String> {
        let table = self.members(class_fqn);
        let member = table.find(kind, name)?;
        if self
            .declared_ty(&member.row)
            .filter(|_| !member.row.doc_ty_from_body)
            .is_some()
        {
            return None;
        }

        let (ty, declared_by) = self.inherited_ty(class_fqn, kind, name, INHERIT_DEPTH)?;
        // `self` in an inherited type means the class that wrote it; `static` means the class the
        // call was made *on*, which only a caller holding the receiver can say.
        //
        // `None` renders the keyword, and that is the honest answer for a card describing a
        // declaration: hover is asked about a `SymbolLocation`, whose container is the class that
        // *declared* the member and not the one somebody called it through. Reading it as the
        // called class put `?EntityBase` on the card for `User::load($id)` -- a class that is
        // abstract, so `static` can never be one -- while go-to-type-definition at the same site
        // landed on `User`. Two answers about one value, disagreeing, which is what
        // `a-type-definition-lands-on-what-hover-names` was written to find.
        let scope = called_on.map(|called| crate::resolve::RelativeScope {
            called: self
                .members_owner(called)
                .unwrap_or_else(|| declared_by.clone()),
            declaring: declared_by,
        });
        let returns = self.render_stored(ty, scope.as_ref(), crate::variables::Naming::Short);
        Some(
            member
                .row
                .signature_returning(self.skeleton.types(), Some(&returns)),
        )
    }

    /// The type an override inherits from the declaration above it.
    ///
    /// `{@inheritdoc}`, which is how most of Drupal's API is written: `ConfirmFormBase::buildForm`
    /// has neither a return hint nor an `@return`, and `FormInterface::buildForm`, four levels up,
    /// has `@return array`. An override that says nothing about its type is not saying the type is
    /// unknown -- it is saying the type has not changed.
    ///
    /// Not answered by asking an ancestor's flattened table, which is the shape this first took and
    /// the reason it did not work: flattening prefers the derived declaration at every level, so the
    /// parent's table hands back the parent's own untyped row. The recursion has to keep climbing
    /// while what it finds still has no type.
    ///
    /// Returns the class that declared the type as well, because `self` written in an inherited type
    /// means the class it was written in and not the one it was found through.
    pub(crate) fn inherited_ty(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
        depth: u8,
    ) -> Option<(phrost_ty::TypeId, Name)> {
        if depth == 0 {
            return None;
        }
        let header = self.class_header(class_fqn)?;

        // The order PHP looks in: the parent class, then interfaces, then used traits.
        for ancestor in header
            .extends
            .iter()
            .chain(header.implements.iter())
            .chain(header.uses.iter())
        {
            if let Some(member) = self.members(ancestor.as_str()).find(kind, name)
                // An ancestor whose own type came off its body is not a declaration either: the
                // climb is looking for what somebody *wrote*, and a body two levels down says no
                // more about the contract than the one in hand. `Extender::execute()` calling
                // `parent::execute()` found the parent's body and stopped one interface short.
                && !member.row.doc_ty_from_body
                // Nor is a member a `@mixin` forwards: it was declared by another class entirely,
                // and an interface that is `@mixin` its own implementation -- Laravel's
                // `Contracts\Eloquent\Builder` -- would hand every method the query builder's type.
                && !member.forwarded
                && let Some(ty) = member.row.effective_ty(self.skeleton.types())
            {
                let declared_by = match &member.provenance {
                    MemberProvenance::Own | MemberProvenance::Trait(_) => ancestor.clone(),
                    MemberProvenance::Inherited(fqn) | MemberProvenance::Interface(fqn) => {
                        fqn.clone()
                    }
                };
                return Some((ty, declared_by));
            }
            if let Some(found) = self.inherited_ty(ancestor.as_str(), kind, name, depth - 1) {
                return Some(found);
            }
        }

        None
    }

    /// The declaration a member overrides: the same member on the nearest ancestor that declares it.
    ///
    /// What `{@inheritdoc}` needs. Searched in the order PHP looks: the parent class first, then
    /// interfaces, then used traits. Each ancestor's table is itself flattened, so one step here can
    /// cross several levels — the answer is the nearest declaration above, not the nearest parent.
    pub fn overridden_member(
        &self,
        class_fqn: &str,
        kind: MemberKind,
        name: &str,
    ) -> Option<SymbolLocation> {
        let header = self.class_header(class_fqn)?;

        header
            .extends
            .iter()
            .chain(header.implements.iter())
            .chain(header.uses.iter())
            .find_map(|ancestor| self.member_location(ancestor.as_str(), kind, name))
    }

    /// The best-ranked declarer of a name, as a location.
    pub(crate) fn best_location(
        &self,
        declarers: &[phrost_index::Declarer],
    ) -> Option<SymbolLocation> {
        let winner = *self.skeleton.rank(declarers).first()?;
        self.located(winner.file, winner.symbol)
    }

    /// The declaration of `fqn` in one named file, rather than the project's winner for that name.
    ///
    /// For every caller that has already decided *which* declaration it means. The catalog answers
    /// a different question -- which of the claimants on a name wins -- and a caller that asks it
    /// while holding a file gets the winner's range, in the winner's file. Two of them did: the
    /// implementer list rendered one file twice, and `twice-declared` underlined a range belonging
    /// to the other file, which on a shorter one is a position that does not exist.
    pub(crate) fn declared_in(
        &self,
        file: phrost_model::FileId,
        fqn: &Name,
    ) -> Option<SymbolLocation> {
        let index = self
            .skeleton
            .file(file)?
            .scope()
            .symbols()
            .iter()
            .position(|symbol| &symbol.fqn == fqn)?;
        self.located(file, u32::try_from(index).ok()?)
    }

    /// One declaration, named by the file it is in rather than by a name to be looked up.
    ///
    /// The half of [`Self::best_location`] that has nothing to do with ranking, shared because a
    /// caller that already knows *which* declaration it means must not go back through the catalog
    /// for it: two files declaring one name have one winner there, so `implementers_of` rendered
    /// the winner twice and dropped the other. See `implementers.rs`.
    pub(crate) fn located(
        &self,
        file_id: phrost_model::FileId,
        symbol_index: u32,
    ) -> Option<SymbolLocation> {
        let file = self.skeleton.file(file_id)?;
        let symbol = file.scope().symbols().get(symbol_index as usize)?;

        Some(SymbolLocation {
            file: file_id,
            fqn: symbol.fqn.clone(),
            kind: symbol.kind,
            name_range: symbol.name_range,
            full_range: symbol.full_range,
            container: None,
            deprecated: symbol.is_deprecated(),
            // A top-level symbol is never declared by a tag: `@method` and `@property` describe
            // members of a class and nothing else.
            from_docblock: false,
            // Only a function has one here. A class's shape is a separate projection, and a
            // constant has nothing to render beyond its name.
            signature: file.function_of(symbol_index).map(|function| {
                function
                    .signature(short_name(symbol.fqn.as_str()), self.skeleton.types())
                    .into()
            }),
            member_modifiers: None,
            origin: None,
            docblock_range: symbol.docblock_range,
        })
    }

    /// What a class-like declaration says about itself.
    ///
    /// Facts, not text. Built from the skeleton's resolved relations rather than from source, so it
    /// reads the same for a vendor file nobody has open — and composing a header a person reads is
    /// the feature layer's job, because a rendered string here would hand every consumer this
    /// crate's idea of formatting whether it suited them or not.
    /// What kind of class-like declaration a name is: a class, an interface, a trait, an enum.
    ///
    /// Beside [`Self::class_header`] rather than read off one, because the caller that needs this
    /// asks it of every name in a file and a header clones the supertype lists to build itself.
    /// The kind is one field of the row that is already in hand.
    pub fn class_kind(&self, fqn: &str) -> Option<phrost_sema::ClassKind> {
        let location = self.class_location(fqn)?;
        let file = self.skeleton.file(location.file)?;
        file.classes()
            .iter()
            .find(|class| class.fqn() == &location.fqn)
            .map(|class| class.kind())
    }

    /// Whether a class is declared `#[Attribute]`.
    ///
    /// Beside [`Self::class_kind`] and for its reason: the caller asks it of every name it is about
    /// to rank, and a header clones three supertype lists to answer one bit.
    pub fn class_is_attribute(&self, fqn: &str) -> bool {
        let Some(location) = self.class_location(fqn) else {
            return false;
        };
        let Some(file) = self.skeleton.file(location.file) else {
            return false;
        };
        file.classes()
            .iter()
            .find(|class| class.fqn() == &location.fqn)
            .is_some_and(|class| class.is_attribute())
    }

    pub fn class_header(&self, fqn: &str) -> Option<ClassHeader> {
        let location = self.class_location(fqn)?;
        let file = self.skeleton.file(location.file)?;
        let class = file
            .classes()
            .iter()
            .find(|class| class.fqn() == &location.fqn)?;

        let names = |targets: &[phrost_sema::ClassRef]| {
            targets
                .iter()
                .map(|target| target.fqn.clone())
                .collect::<Vec<_>>()
        };

        // PHP gives every enum `UnitEnum` and a backed one `BackedEnum` without either being
        // written -- it refuses `enum E implements UnitEnum` outright. Named only when the standard
        // library declares it, the member table's own condition.
        let implied = (class.kind() == phrost_sema::ClassKind::Enum)
            .then(|| match class.is_backed_enum() {
                true => phrost_sema::BACKED_ENUM,
                false => phrost_sema::UNIT_ENUM,
            })
            .and_then(|implicit| self.class_location(implicit))
            .map(|interface| interface.fqn);

        Some(ClassHeader {
            fqn: location.fqn,
            kind: class.kind(),
            // An enum cannot be extended, so it is closed whether or not the word is written.
            is_final: class.is_final() || class.kind() == phrost_sema::ClassKind::Enum,
            is_readonly: class.is_readonly(),
            is_attribute: class.is_attribute(),
            modifiers: class.modifier_words(),
            extends: names(class.extends()),
            implements: names(class.implements()),
            uses: names(class.uses()),
            implied,
        })
    }
}

impl IndexView<'_> {
    /// The classes a trait says every class composing it is: its `@mixin`, `@require-extends` and
    /// `@require-implements` tags. Empty for anything that is not a trait, which is how a caller
    /// asks without first asking which kind it has.
    pub(crate) fn what_a_trait_user_is(&self, fqn: &str) -> Vec<Name> {
        let Some(location) = self.class_location(fqn) else {
            return Vec::new();
        };
        let Some(file) = self.skeleton.file(location.file) else {
            return Vec::new();
        };
        file.classes()
            .iter()
            .find(|class| class.fqn() == &location.fqn)
            .filter(|class| class.kind() == phrost_sema::ClassKind::Trait)
            .map(|class| {
                class
                    .mixins()
                    .iter()
                    .map(|mixin| mixin.fqn.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// What a free function promises beyond its signature.
    ///
    /// Its own lookup rather than a shared one, because the two are found in different tables: a
    /// method on a class, a function by the name it was declared with.
    pub(crate) fn function_contract(&self, fqn: &str) -> Option<&phrost_sema::Contract> {
        let key = self.skeleton.interner().lookup_key(fqn)?;
        let winner = *self
            .skeleton
            .rank(self.skeleton.owners().function(&key))
            .first()?;
        self.skeleton
            .file(winner.file)?
            .function_of(winner.symbol)?
            .contract()
    }
}

/// What [`IndexView::declared_through`] made of a declared type.
pub(crate) enum Declared {
    /// Rebuilt around what the value supplied, which a reader holds rather than interns.
    Held(Held),
    /// A type from the table, under a null the declaration added when `nullable`.
    Type {
        ty: phrost_ty::TypeId,
        nullable: bool,
    },
    /// A template nothing supplied and nothing bounds: no type at all, rather than a class named
    /// after it.
    Unbound,
}
