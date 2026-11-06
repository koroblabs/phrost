//! What a class supplied for a supertype's template parameters.
//!
//! PHP has no generics, so a container states what it holds in a docblock and a subclass states what
//! it filled in: `@template T` on the declaration, `@extends Bar<Baz>` on whoever extended it. A
//! member whose type is `T` therefore says nothing on its own -- it is a name resolving to no class
//! -- and what it means depends on which class the member was reached through.
//!
//! # A bare name, or a class around one
//!
//! `@return T` is substituted by swapping it, which is all a reader can do with an interned type:
//! building a new one needs the table open for writing. `@return Collection<T>` is substituted a
//! second way -- into a [`Held::Applied`](crate::resolve::Held::Applied), which is built rather than
//! interned and so needs no table at all. See [`applied_with`](IndexView::applied_with).
//!
//! `@return list<T>` is still not, because its base is a builtin and there is no class to hang the
//! arguments off. The bound is what answers there, which is what
//! [`iterated_element`](IndexView::iterated_element) reads -- fewer members than the real type has and
//! never more.

use phrost_model::{Name, short_name};
use phrost_ty::TypeId;

use crate::view::IndexView;

/// How far up a hierarchy an argument may be chased.
///
/// `Foo extends Second<Baz, Boo>`, `Second<T, Y> extends First<T>`, `First<T>` declaring the member
/// -- two hops for one answer, and a broken file can name a cycle.
pub(crate) const CHASE: u8 = 8;

/// What a chase up a hierarchy answers when it lands on one of the asking class's own templates.
///
/// Two questions share the walk. A caller holding a value's arguments wants the template itself, so
/// it can put the argument in: `ScalarNodeDefinition<NodeBuilder>` reaches `NodeDefinition::end():
/// TParent` through `VariableNodeDefinition`, and answering with the bound there threw the
/// `NodeBuilder` away -- only a template with no bound got through, by accident. A caller asking
/// about a value with no arguments wants the bound, which is what such a value promises; iterating a
/// bare `NodeList<T of Node|NamespaceNode>` through `IteratorAggregate` to `Traversable` is that.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Landing {
    Template,
    Bound,
}

impl<'a> IndexView<'a> {
    /// The type `class_fqn` supplied where `owner` declared `ty` as one of its templates.
    ///
    /// `None` when `ty` is not a template of `owner`, or when nothing between them said what it is.
    /// Both are the same answer to a caller: the declaration stands as written.
    pub(crate) fn substituted_template(
        &self,
        class_fqn: &str,
        owner: &str,
        ty: TypeId,
    ) -> Option<TypeId> {
        self.substituted(class_fqn, owner, ty, CHASE)
            .filter(|found| self.names_something_real(*found))
    }

    /// The argument the *value* carries where `owner` declared `ty` as one of its templates.
    ///
    /// Before anything the declaration says, and that order is the whole point: `@param
    /// Collection<Item> $items` is about this container, `@extends` is about every value of the
    /// class, and a bound is about every argument the class could ever be given. Only the first
    /// answers `$items->get()`.
    ///
    /// A `Held` rather than a type id, because an argument written on a parameter in the file being
    /// read lives in that file's arena and cannot be turned into an interned id by a reader.
    pub(crate) fn supplied_argument_at(
        &self,
        class_fqn: &str,
        owner: &str,
        ty: TypeId,
        arguments: &[crate::resolve::Held],
    ) -> Option<crate::resolve::Held> {
        if arguments.is_empty() {
            return self.argument_named_after(class_fqn, owner, ty);
        }
        let position = self.template_named(owner, ty)?;
        self.supplied_argument_by_position(class_fqn, owner, position, arguments)
    }

    /// The same, for `owner`'s template at `position` rather than one a type names.
    pub(crate) fn supplied_argument_by_position(
        &self,
        class_fqn: &str,
        owner: &str,
        position: usize,
        arguments: &[crate::resolve::Held],
    ) -> Option<crate::resolve::Held> {
        // Two hops, because the value's arguments are for `class_fqn`'s own templates and the member
        // may be declared further up. `Items<Item> extends Holder<T>` carries one argument for
        // `Items`, and `Holder`'s member asks about `Holder`'s template -- so the class's own
        // `@extends` says which of its templates fills that slot, and the value says what that one is.
        let mine = match class_fqn.eq_ignore_ascii_case(owner) {
            true => position,
            false => {
                let through =
                    self.type_argument(class_fqn, owner, position, CHASE, Landing::Template)?;
                self.template_position(class_fqn, through)?
            }
        };
        // `Collection<Page>` gives a `<TKey, TValue>` container one argument, and it is the value --
        // the convention `IteratedSlot::written_position` reads a `foreach` by. Read by position it
        // was the key, so `foreach ($chapter->pages as $page)` typed the page and
        // `$chapter->pages->first()` typed nothing. Only for a container: elsewhere one argument is
        // the first template, as written.
        let declared = self.class_templates(class_fqn).map_or(0, <[_]>::len);
        if arguments.len() == 1 && declared > 1 && !self.iterates_its_properties(class_fqn) {
            return (mine + 1 == declared).then(|| arguments[0].clone());
        }
        arguments.get(mine).cloned()
    }

    /// The argument a framework names after the class itself, where nothing up the chain wrote one:
    /// Laravel's `UserFactory` makes an `App\Models\User` by its name -- see the seam's
    /// `NamedTemplateArgument`. The first of the framework's candidates the index holds, or nothing,
    /// and the declaration's bound then stands.
    ///
    /// The owner is the gate, compared before anything is climbed: almost no member is a framework
    /// class's own.
    fn argument_named_after(
        &self,
        class_fqn: &str,
        owner: &str,
        ty: TypeId,
    ) -> Option<crate::resolve::Held> {
        let rule = self
            .skeleton
            .returns()
            .named_arguments()
            .iter()
            .find(|rule| owner.eq_ignore_ascii_case(&rule.class))?;
        let position = self.template_named(owner, ty)?;
        let templates = self.class_templates(owner)?;
        if templates.get(position)?.name.as_str() != &*rule.template
            || class_fqn.eq_ignore_ascii_case(owner)
            || self
                .type_argument(class_fqn, owner, position, CHASE, Landing::Bound)
                .is_some()
        {
            return None;
        }
        // What the class names outright, nearest first, below the framework's class.
        if let Some(property) = rule.from_property.as_deref() {
            let mut class = self.class_location(class_fqn)?.fqn;
            for _ in 0..crate::resolve::HIERARCHY_DEPTH {
                if class.as_str().eq_ignore_ascii_case(&rule.class) {
                    break;
                }
                let location = self.class_location(class.as_str())?;
                if let Some(named) =
                    self.skeleton
                        .class_default(location.file, class.as_str(), property)
                {
                    return self
                        .class_location(named)
                        .map(|found| crate::resolve::Held::Class(found.fqn));
                }
                let Some(parent) = self
                    .class_header(class.as_str())
                    .and_then(|header| header.extends.first().cloned())
                else {
                    break;
                };
                class = parent;
            }
        }
        let members = self.members(class_fqn);
        let declares_it = rule.unless.iter().any(|name| {
            [
                phrost_sema::MemberKind::Property,
                phrost_sema::MemberKind::Method,
            ]
            .into_iter()
            .filter_map(|kind| members.find(kind, name))
            .any(|member| {
                !matches!(&member.provenance, phrost_sema::MemberProvenance::Inherited(from)
                        if from.as_str().eq_ignore_ascii_case(&rule.class))
            })
        });
        if declares_it {
            return None;
        }
        let class = self.class_location(class_fqn)?.fqn;
        let namespace = self
            .skeleton
            .namespace_of_directory(&rule.app_directory)
            .unwrap_or(&rule.default_namespace);
        (rule.names)(class.as_str(), namespace)
            .iter()
            .find_map(|named| self.class_location(named))
            .map(|found| crate::resolve::Held::Class(found.fqn))
    }

    /// A member's `Collection<T>` with the value's own arguments put into it.
    ///
    /// `@return Collection<T>` on a `@template T` class: `T` is not the member's type, it is one of
    /// the *result's*, so what the receiver was given fills it -- and what comes back is an instance
    /// of the same class carrying that, rather than a name nothing declares. The one substitution
    /// this module's opening note said it did not do, and it is affordable for exactly the reason
    /// that note gave against it: the result is a [`Held::Applied`](crate::resolve::Held::Applied),
    /// which a reader can build, rather than an interned type, which it cannot.
    ///
    /// `None` unless a slot was actually filled. A generic naming no template of the owner already
    /// says everything it has to say, and wrapping it would make two spellings of one value compare
    /// unequal.
    ///
    /// `scope` is what lets the base be one of PHP's relative words. `@return self<T>` and
    /// `@return Collection<T>` are one claim written two ways -- the fluent method of every
    /// collection class writes the first -- and without it the second was substituted and the first
    /// resolved to nothing at all: `$c->map(...)->first()` answered `T`, so the chain died one call
    /// in while `$c->first()` was right.
    pub(crate) fn applied_with(
        &self,
        class_fqn: &str,
        owner: &str,
        ty: TypeId,
        arguments: &[crate::resolve::Held],
        scope: Option<&crate::resolve::RelativeScope>,
    ) -> Option<crate::resolve::Held> {
        let types = self.skeleton.types();
        let phrost_ty::TypeRef::Generic { base, args, .. } = types.get(ty) else {
            return None;
        };

        let mut filled = Vec::new();
        let mut bound = false;
        for &argument in types.operands(*args) {
            // `static` and `$this` as an argument are the receiver itself, arguments and all.
            // Symfony's `children(): NodeBuilder<static>` on an `ArrayNodeDefinition<NodeBuilder>`
            // is a builder whose `end()` hands that very definition back; read as the bare class,
            // the `end()` after it fell to a bound, and every config tree past its first level was
            // a missing method.
            if let Some(scope) = scope
                && let Some(receiver) =
                    self.the_receiver_back(argument, scope, class_fqn, arguments)
            {
                bound = true;
                filled.push(receiver);
                continue;
            }
            if let Some(supplied) = self.supplied_argument_at(class_fqn, owner, argument, arguments)
            {
                bound = true;
                filled.push(supplied);
                continue;
            }
            // A template the value did not supply is still one the declaration answers for: what
            // an `@extends` between the two filled in, or the template's bound. Left as written it
            // was a slot holding a class called `TModel` -- Laravel's `Factory::create()` returns
            // `Collection<int, TModel>|TModel`, and `$users[0]->name` asked for a member of it.
            if let Some(position) = self.template_named(owner, argument) {
                bound = true;
                filled.push(self.template_or_bound(class_fqn, owner, argument, position));
                continue;
            }
            // A union or an intersection in a slot is the question a whole return asks:
            // `Collection<int, TRelatedModel&object{pivot: TPivotModel}>` is what `BelongsToMany`
            // loads. Left as written, the relation's `get()` declined whole, and its `getResults()`
            // -- the property `$vault->users` -- was a collection of a class nothing declares.
            if let Some(scope) = scope
                && matches!(
                    types.get(argument),
                    phrost_ty::TypeRef::Union(_)
                        | phrost_ty::TypeRef::Intersection(_)
                        | phrost_ty::TypeRef::Nullable(_)
                )
                && self.mentions_template(owner, argument)
                && let Some(nested) =
                    self.substituted_union(class_fqn, owner, argument, arguments, scope)
            {
                bound = true;
                filled.push(nested);
                continue;
            }
            // Left as the declaration wrote it, so the positions still line up with the class's
            // templates -- a slot that names a real type is an answer.
            filled.push(crate::resolve::Held::Stored {
                ty: argument,
                scope: None,
            });
        }
        if !bound {
            return None;
        }
        // Through the index, for the reason every other class here goes through it: a `Held` for a
        // class with no declaration would report every member of it as missing.
        let class = self
            .class_location(self.class_of_stored(*base, scope)?.as_str())?
            .fqn;
        Some(crate::resolve::Held::Applied {
            class,
            arguments: filled.into_boxed_slice(),
        })
    }

    /// What `owner`'s template at `position` is for a value of `class_fqn` that supplied nothing.
    ///
    /// What an `@extends` between them filled in, else the template's bound, else anything at all --
    /// the promise the declaration makes about every argument it could be given.
    fn template_or_bound(
        &self,
        class_fqn: &str,
        owner: &str,
        template: TypeId,
        position: usize,
    ) -> crate::resolve::Held {
        let bound = self
            .substituted_template(class_fqn, owner, template)
            .or_else(|| {
                self.class_templates(owner)
                    .and_then(|templates| templates.get(position)?.bound)
            });
        match bound {
            Some(ty) => crate::resolve::Held::Stored { ty, scope: None },
            None => crate::resolve::Held::Builtin(phrost_model::BuiltinTy::Mixed),
        }
    }

    /// A member's `T[]` with the value's own argument put into it.
    ///
    /// The one shape this module's opening note said it did not do, and the note's reason has expired:
    /// `@return list<T>` has a *builtin* base, so there is no class to hang arguments off and
    /// [`Self::applied_with`] cannot build it. [`Held::ListOf`](crate::resolve::Held::ListOf) can --
    /// it is built rather than interned, exactly as `Held::Applied` is, and it is what every other
    /// container of a known element in this crate already uses.
    ///
    /// All three spellings, because they are one claim: `T[]`, `list<T>` and `array<K, T>` all say the
    /// container holds `T`, and the *last* type argument is what a container holds however it was
    /// keyed. What is given up is the key half -- `Held::ListOf` states no key -- which costs a
    /// judgement that declines rather than one that answers wrong.
    ///
    /// Left alone unless the element is a template of `owner`: a `Row[]` already says everything it
    /// has to say, and wrapping it would make two spellings of one value compare unequal.
    pub(crate) fn list_of_supplied(
        &self,
        class_fqn: &str,
        owner: &str,
        ty: TypeId,
        arguments: &[crate::resolve::Held],
    ) -> Option<crate::resolve::Held> {
        let types = self.skeleton.types();
        let phrost_ty::TypeRef::Generic { base, args, .. } = types.get(ty) else {
            return None;
        };
        if !matches!(
            types.get(*base),
            phrost_ty::TypeRef::Builtin(
                phrost_model::BuiltinTy::Array | phrost_model::BuiltinTy::Iterable
            )
        ) {
            return None;
        }
        let element = types.operands(*args).last().copied()?;
        let position = self.template_named(owner, element)?;
        let held = match self.supplied_argument_at(class_fqn, owner, element, arguments) {
            Some(supplied) => supplied,
            // Nothing supplied it, so the element is what the template promises about every
            // argument. Left as written it was a list of a class called `T`, and iterating it bound
            // a variable of that name.
            None => self.template_or_bound(class_fqn, owner, element, position),
        };
        Some(crate::resolve::Held::ListOf(Box::new(held)))
    }

    /// An instance of the class a contract named, with whatever its arguments resolved to.
    ///
    /// `None` in each slot is an argument the call did not settle, and the class's own `@template`
    /// bound stands in for it -- the same rule [`Self::instance_of`] follows, and for the same
    /// reason: the positions have to line up with the templates whatever happened at the call.
    ///
    /// A plain [`Held::Class`](crate::resolve::Held::Class) when nothing was settled, because that is
    /// what the value is: a `Bar` whose argument nobody wrote. Still an answer, and a better one than
    /// the nothing this used to give.
    pub(crate) fn applied_instance(
        &self,
        class: TypeId,
        filled: Vec<Option<crate::resolve::Held>>,
    ) -> Option<crate::resolve::Held> {
        let class = self
            .class_location(self.class_of_stored(class, None)?.as_str())?
            .fqn;
        self.applied_instance_of(class, filled)
    }

    /// The same, for a class already resolved -- one a relative word named, which only a caller
    /// holding the call's scope can say.
    pub(crate) fn applied_instance_of(
        &self,
        class: Name,
        filled: Vec<Option<crate::resolve::Held>>,
    ) -> Option<crate::resolve::Held> {
        if filled.iter().all(Option::is_none) {
            return Some(crate::resolve::Held::Class(class));
        }
        let templates = self.class_templates(class.as_str()).unwrap_or_default();
        let arguments = filled
            .into_iter()
            .enumerate()
            .map(|(position, slot)| match slot {
                Some(held) => held,
                None => match templates.get(position).and_then(|template| template.bound) {
                    Some(ty) => crate::resolve::Held::Stored { ty, scope: None },
                    None => crate::resolve::Held::Builtin(phrost_model::BuiltinTy::Mixed),
                },
            })
            .collect::<Vec<_>>();
        Some(crate::resolve::Held::Applied {
            class,
            arguments: arguments.into_boxed_slice(),
        })
    }

    /// Which of `owner`'s templates `ty` names, when it is a template at all.
    ///
    /// A template name resolves to no class, and that is the cheap way to tell it from a real one --
    /// see [`Self::substituted`] for why the check is worth its cost here.
    pub(crate) fn template_named(&self, owner: &str, ty: TypeId) -> Option<usize> {
        let phrost_ty::TypeRef::Named(named) = self.skeleton.types().get(ty) else {
            return None;
        };
        if self.class_location(named.as_str()).is_some() {
            return None;
        }
        self.template_position(owner, ty)
    }

    /// The docblock return, when it names a template of `owner` and a hint was written beside it.
    ///
    /// A `@template T of X` is bounded by exactly what the hint declares, so a tag naming one is not
    /// a competing claim about the type -- it says *which* of the declared possibilities this call
    /// gets, and can only ever be narrower. `TypeTable::refined` cannot see that, because a template
    /// name is a `Named` and looks like a class to it, so the choice belongs here where the owner's
    /// template list is in hand.
    ///
    /// It is not a corner: **124 of the 145 templated returns** in one real vendor tree write a
    /// native type beside the tag, and ranking the hint first reads none of them.
    ///
    /// The hint is not discarded -- see the caller. Nothing may supply the template, and what was
    /// written beside it is then a real type and the better answer.
    pub(crate) fn template_return(
        &self,
        owner: &str,
        row: &phrost_sema::MemberRow,
    ) -> Option<TypeId> {
        let (Some(_), Some(doc)) = (row.ty, row.doc_ty) else {
            return None;
        };
        self.mentions_template(owner, doc).then_some(doc)
    }

    /// A parameter's expectation with a template the receiver bound put in its place.
    ///
    /// `Some(ty)` unchanged when the expectation names no template, which is nearly every parameter
    /// and costs one match on the interned type. `Some(substituted)` when the receiver said what the
    /// template is. **`None` when it did not**, and that is the load-bearing answer: the bare name is
    /// not a type, and judging against it rejects every scalar for not being a class while accepting
    /// every class because the name resolves to none.
    ///
    /// Substituted to an interned id rather than to a `Held`, because that is what the judgement
    /// takes -- so a receiver typed in the *file being read* (`/** @var Bag<Row> */` on a local)
    /// declines rather than being interned, which a reading view cannot do. A miss, and never a wrong
    /// answer.
    /// [`Self::substituted_expectation`] for either spelling of an expectation.
    ///
    /// A written one is a closure's parameter, read out of the file being looked at. It carries no
    /// class templates for a receiver to bind -- a closure declares none and there is no receiver
    /// -- so it passes through, and the substitution stays the one rule it always was.
    pub(crate) fn bound_expectation(
        &self,
        templates: Option<&crate::hints::CalleeTemplates>,
        expected: crate::compat::Expectation,
    ) -> Option<crate::compat::Expectation> {
        match expected {
            crate::compat::Expectation::Stored(ty) => self
                .substituted_expectation(templates, ty)
                .map(crate::compat::Expectation::Stored),
            written => Some(written),
        }
    }

    pub(crate) fn substituted_expectation(
        &self,
        templates: Option<&crate::hints::CalleeTemplates>,
        ty: TypeId,
    ) -> Option<TypeId> {
        let Some(templates) = templates else {
            // No receiver, so no class template can appear -- and a *method* template on a free
            // function is bound by the contract reader rather than here. What is left may still be
            // a name nothing declares, which is the same unjudgeable thing an unbound template is.
            return self.judgeable(ty);
        };
        let called = templates.called.as_str();
        let owner = templates.owner.as_str();
        if !self.mentions_template(owner, ty) {
            return self.judgeable(ty);
        }
        if let Some(crate::resolve::Held::Stored { ty: supplied, .. }) =
            self.supplied_argument_at(called, owner, ty, &templates.supplied)
        {
            return Some(supplied);
        }
        self.substituted_template(called, owner, ty)
    }

    /// The expectation itself, unless something in it names nothing this project declares.
    ///
    /// Asked only where no template was substituted, so a bound `T` keeps the class the receiver
    /// gave it and is judged against that.
    fn judgeable(&self, ty: TypeId) -> Option<TypeId> {
        (!self.names_nothing_declared(ty)).then_some(ty)
    }

    /// Whether anything inside a type names a class the project does not declare.
    ///
    /// The same judgement the substitution above makes for a class template the receiver left
    /// unbound, for the names it has no owner to recognise them by. A `Named` that resolves to
    /// nothing is not a type to judge an argument against: it rejects every scalar for not being a
    /// class and accepts every class because nothing can be related to a name that resolves to
    /// none -- wrong in both directions, which is what `an_argument_against_a_template` records.
    ///
    /// The carrier is a *free function's own* `@template`, which no receiver can bind and the index
    /// does not hold -- only a class has a template list. WordPress writes `@template T` +
    /// `@param T $value` on `wp_unslash` and `wp_slash`, and a call to either reported
    /// `$value takes \T`, which is a sentence no reader can act on. A class that genuinely does not
    /// exist reads the same way here and is `undefined-type`'s finding rather than this check's.
    ///
    /// Every operand, because a union with one unjudgeable alternative is unjudgeable: the value may
    /// be that alternative. The traversal is [`Self::mentions_template`]'s, for the same reason it
    /// recurses -- a template in an element position is as unusable as one written bare.
    fn names_nothing_declared(&self, ty: TypeId) -> bool {
        let types = self.skeleton.types();
        match types.get(ty) {
            phrost_ty::TypeRef::Named(named) => self.class_location(named.as_str()).is_none(),
            phrost_ty::TypeRef::Nullable(inner) => self.names_nothing_declared(*inner),
            phrost_ty::TypeRef::Generic { base, args, .. } => {
                self.names_nothing_declared(*base)
                    || types
                        .operands(*args)
                        .to_vec()
                        .iter()
                        .any(|arg| self.names_nothing_declared(*arg))
            }
            phrost_ty::TypeRef::Union(operands) => types
                .operands(*operands)
                .to_vec()
                .iter()
                .any(|operand| self.names_nothing_declared(*operand)),
            _ => false,
        }
    }

    /// Whether a union or a `?T` has an operand naming a class nothing declares.
    ///
    /// Narrower than [`Self::names_nothing_declared`], which also refuses `Collection<T>` -- a
    /// class whose members are real whatever its slot says, and worth keeping where the question
    /// is what a value *is* rather than what to judge an argument against.
    pub(crate) fn a_union_names_nothing_declared(&self, ty: TypeId) -> bool {
        let types = self.skeleton.types();
        let operands = match types.get(ty) {
            phrost_ty::TypeRef::Union(operands) => types.operands(*operands).to_vec(),
            phrost_ty::TypeRef::Nullable(inner) => vec![*inner],
            _ => return false,
        };
        operands.into_iter().any(|operand| {
            matches!(types.get(operand), phrost_ty::TypeRef::Named(named)
                if self.class_location(named.as_str()).is_none())
        })
    }

    /// Whether a type is one of `owner`'s templates, or has one inside a `?T` or a union.
    pub(crate) fn mentions_template(&self, owner: &str, ty: TypeId) -> bool {
        if self.template_named(owner, ty).is_some() {
            return true;
        }
        match self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Nullable(inner) => self.mentions_template(owner, *inner),
            // `T[]` and `Collection<T>`. Without this arm a template in an *element* position falls
            // past every substitution and is rendered as written -- a variable called `T` in a
            // tooltip, which is strictly worse than the `array` the hint says.
            phrost_ty::TypeRef::Generic { base, args, .. } => {
                self.mentions_template(owner, *base)
                    || self
                        .skeleton
                        .types()
                        .operands(*args)
                        .to_vec()
                        .iter()
                        .any(|&argument| self.mentions_template(owner, argument))
            }
            // A union and an intersection alike: `MockObject&T` is what every mock factory writes,
            // and a template in one operand is a template the return mentions however the operands
            // are joined.
            phrost_ty::TypeRef::Union(operands) | phrost_ty::TypeRef::Intersection(operands) => {
                self.skeleton
                    .types()
                    .operands(*operands)
                    .to_vec()
                    .iter()
                    .any(|&operand| self.mentions_template(owner, operand))
            }
            // `object{pivot: TPivotModel}`: a template in a property is one the type mentions.
            phrost_ty::TypeRef::ObjectShape { fields } => self
                .skeleton
                .types()
                .operands(*fields)
                .to_vec()
                .iter()
                .any(|&field| self.mentions_template(owner, field)),
            phrost_ty::TypeRef::Field { ty, .. } => self.mentions_template(owner, *ty),
            _ => false,
        }
    }

    /// A union or an intersection with a template in it, substituted operand by operand.
    ///
    /// `@return TDefault|TInput` on Symfony's `InputBag::get` is the shape, and it is 38 of the
    /// vendor tree's templated returns. Read as one type it answers nothing at all: a union is not a
    /// template name, so every substitution above declines and what is left is a union of names that
    /// resolve to no class.
    ///
    /// **Every operand has to answer**, for the reason a list literal's every entry does: a union
    /// with an unsubstituted operand dropped is *narrower than the truth*, and a type narrower than
    /// the truth reports correct code. So one unbound template declines the whole union, and the
    /// caller falls back to the hint written beside the tag.
    pub(crate) fn substituted_union(
        &self,
        class_fqn: &str,
        owner: &str,
        ty: TypeId,
        arguments: &[crate::resolve::Held],
        scope: &crate::resolve::RelativeScope,
    ) -> Option<crate::resolve::Held> {
        // An intersection travels the same road and joins the other way: `MockObject&T` is what
        // every mock factory writes, and the value really is both. What differs is only the fold, so
        // splitting them into two walks would be two constructors of one rule.
        let (operands, all_of) = match self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Union(operands) => (*operands, false),
            phrost_ty::TypeRef::Intersection(operands) => (*operands, true),
            // `(T&object{...})|null` interns as a nullable intersection: the same question inside.
            phrost_ty::TypeRef::Nullable(inner)
                if matches!(
                    self.skeleton.types().get(*inner),
                    phrost_ty::TypeRef::Union(_) | phrost_ty::TypeRef::Intersection(_)
                ) =>
            {
                return self
                    .substituted_union(class_fqn, owner, *inner, arguments, scope)
                    .map(Self::nullable);
            }
            _ => return None,
        };
        let operands = self.skeleton.types().operands(operands).to_vec();
        let mut held = Vec::with_capacity(operands.len());
        for operand in operands {
            held.push(self.substituted_operand(class_fqn, owner, operand, arguments, scope)?);
        }
        Some(match all_of {
            // `object` beside anything else says nothing the rest does not, and it is what
            // `object{pivot: TPivotModel}` lowers to: `Book&object` is a `Book` to every reader.
            true => {
                let object = |held: &crate::resolve::Held| match held {
                    crate::resolve::Held::Stored { ty, .. } => matches!(
                        self.skeleton.types().get(*ty),
                        phrost_ty::TypeRef::Builtin(phrost_model::BuiltinTy::Object)
                    ),
                    crate::resolve::Held::Builtin(phrost_model::BuiltinTy::Object) => true,
                    _ => false,
                };
                if held.iter().any(|one| !object(one)) {
                    held.retain(|one| !object(one));
                }
                Self::all_of(held)
            }
            false => Self::one_of(held),
        })
    }

    /// One operand of a union or an intersection with the value's arguments put into it: `None`
    /// where it names nothing this can place, which ends the whole composite -- see
    /// [`Self::substituted_union`] for why every operand has to answer.
    fn substituted_operand(
        &self,
        class_fqn: &str,
        owner: &str,
        operand: TypeId,
        arguments: &[crate::resolve::Held],
        scope: &crate::resolve::RelativeScope,
    ) -> Option<crate::resolve::Held> {
        if let Some(supplied) = self.supplied_argument_at(class_fqn, owner, operand, arguments) {
            return Some(supplied);
        }
        if let Some(substituted) = self.substituted_template(class_fqn, owner, operand) {
            return Some(crate::resolve::Held::Stored {
                ty: substituted,
                scope: Some(scope.clone()),
            });
        }
        // A container of one, which a union of a collection and its element is:
        // `Collection<int, TModel>|TModel`. Asked of the operand exactly as of a whole return.
        if let Some(container) = self
            .list_of_supplied(class_fqn, owner, operand, arguments)
            .or_else(|| self.applied_with(class_fqn, owner, operand, arguments, Some(scope)))
        {
            return Some(container);
        }
        match self.skeleton.types().get(operand) {
            // A union or an intersection one level in is the same question again:
            // `(TRelatedModel&object{pivot: TPivotModel})|null` is how `BelongsToMany::first()` is
            // written. Kept whole, its template was a name nothing declares and the relation's
            // `first()` typed nothing.
            phrost_ty::TypeRef::Union(_) | phrost_ty::TypeRef::Intersection(_) => {
                return self.substituted_union(class_fqn, owner, operand, arguments, scope);
            }
            phrost_ty::TypeRef::Nullable(_) => {
                if let Some(nested) =
                    self.substituted_union(class_fqn, owner, operand, arguments, scope)
                {
                    return Some(nested);
                }
            }
            // And an object's named properties, each the same question: the `TPivotModel` of
            // `object{pivot: TPivotModel}` is the relation's own argument.
            phrost_ty::TypeRef::ObjectShape { fields }
                if self.mentions_template(owner, operand) =>
            {
                return Some(self.substituted_object(class_fqn, owner, *fields, arguments, scope));
            }
            _ => {}
        }
        // An operand that is not a template at all -- the `null` of `@return null|TNode` -- says
        // what it says. One that names nothing this can place ends the union, and it is *naming
        // nothing* rather than *being a class template* that decides: a `@template` written on
        // the method rather than on the class is bound by the contract reader and not by this
        // one, so testing for a class template here would keep an operand nothing resolved.
        if !self.names_something_real(operand) {
            return None;
        }
        Some(crate::resolve::Held::Stored {
            ty: operand,
            scope: Some(scope.clone()),
        })
    }

    /// An object shape's properties with the value's arguments put into each. A property nothing
    /// can place is left out -- the object still has the rest -- and one left with none is the
    /// `object` it is.
    pub(crate) fn substituted_object(
        &self,
        class_fqn: &str,
        owner: &str,
        fields: phrost_ty::TypeList,
        arguments: &[crate::resolve::Held],
        scope: &crate::resolve::RelativeScope,
    ) -> crate::resolve::Held {
        let types = self.skeleton.types();
        let named = types
            .operands(fields)
            .iter()
            .filter_map(|&field| match types.get(field) {
                phrost_ty::TypeRef::Field { key, ty, .. } => Some((
                    key.clone(),
                    self.substituted_operand(class_fqn, owner, *ty, arguments, scope)?,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        match named.is_empty() {
            true => crate::resolve::Held::Builtin(phrost_model::BuiltinTy::Object),
            false => crate::resolve::Held::Object(named.into_boxed_slice()),
        }
    }

    fn substituted(&self, class_fqn: &str, owner: &str, ty: TypeId, depth: u8) -> Option<TypeId> {
        let phrost_ty::TypeRef::Named(named) = self.skeleton.types().get(ty) else {
            return None;
        };
        // A template name resolves to no class, and that is the cheap way to tell it from a real
        // one: this runs for every member access whose type is a name, and the lookups below are a
        // file scan. A `@template T` beside a class genuinely called `T` would be shadowed by PHP
        // itself, so nothing is lost by stopping here.
        if self.class_location(named.as_str()).is_some() {
            return None;
        }

        // The short name, because a template name is resolved like a class reference at ingest and
        // comes back with whatever namespace was in scope.
        let written = short_name(named.as_str());
        let templates = self.class_templates(owner)?;
        let position = templates
            .iter()
            .position(|template| template.name.as_str() == written)?;

        // The bound when nothing supplied an argument. It is what the declaration promises about
        // every argument it could be given, so its members are a subset of the real ones -- fewer
        // than the type has and never more, which is the only direction worth guessing in.
        match self.type_argument(class_fqn, owner, position, depth, Landing::Bound) {
            Some(found) if self.names_something_real(found) => Some(found),
            // The chase ended on *another template*, which is what an interface extending a
            // generic one writes: `FieldItemListInterface<T of FieldItemInterface> extends
            // ListInterface<T>` passes its own `T` along, so `first(): ?T` resolved to a name
            // nothing declares and the whole chain after it died. The bound to use is then that
            // template's own, which is the narrower of the two -- `FieldItemInterface` rather than
            // `TypedDataInterface` -- and `->get('field')->first()?->getValue()` is the shape.
            Some(found) => self
                .template_named(class_fqn, found)
                .and_then(|position| self.class_templates(class_fqn)?.get(position)?.bound)
                .or(templates[position].bound),
            None => templates[position].bound,
        }
    }

    /// An instance of `class` with whatever its constructor's arguments bound its templates to.
    ///
    /// `@template T` on the class and `@param T $item` on the constructor: the argument at the `new`
    /// is what `T` is for this value and for nothing else of the same class. A class whose templates
    /// nothing bound is a plain class -- there is nothing to carry, and a wrapper around no arguments
    /// would make two spellings of one value compare unequal.
    pub(crate) fn instance_of(
        &self,
        class: Name,
        at: &crate::symbols::CallFacts<'_>,
    ) -> Option<crate::resolve::Held> {
        let templates = self.class_templates(class.as_str())?;
        if templates.is_empty() {
            return Some(crate::resolve::Held::Class(class));
        }
        // No constructor anywhere above it means `new` takes no arguments, so nothing binds a
        // template and the value is the plain class. Answering nothing instead left every
        // `new \SplObjectStorage()` and `new \SplQueue()` without a type.
        let members = self.members(class.as_str());
        let Some(constructor) = members.find(phrost_sema::MemberKind::Method, "__construct") else {
            return Some(crate::resolve::Held::Class(class));
        };
        let params = constructor.row.params.clone();

        let mut arguments = Vec::with_capacity(templates.len());
        let mut bound = false;
        for (position, template) in templates.iter().enumerate() {
            match self.argument_binding(&params, template.name.as_str(), at) {
                Some(argument) => {
                    bound = true;
                    arguments.push(argument);
                }
                // Nothing bound this one, and the positions have to line up with the templates -- so
                // the bound stands in for it, which is what the declaration promises about every
                // argument it could be given.
                None => arguments.push(match template.bound {
                    Some(ty) => crate::resolve::Held::Stored { ty, scope: None },
                    None => crate::resolve::Held::Builtin(phrost_model::BuiltinTy::Mixed),
                }),
            }
            let _ = position;
        }
        Some(match bound {
            true => crate::resolve::Held::Applied {
                class,
                arguments: arguments.into_boxed_slice(),
            },
            false => crate::resolve::Held::Class(class),
        })
    }

    /// What a constructor's arguments bound one template to.
    ///
    /// Through whichever parameter names it: `@param T $item` is bound by that argument, and `@param
    /// T[] $items` by what that argument holds. Both are the same claim about where to look, which is
    /// the only thing a declaration with no generics of its own can say.
    fn argument_binding(
        &self,
        params: &[phrost_sema::ParamRow],
        template: &str,
        at: &crate::symbols::CallFacts<'_>,
    ) -> Option<crate::resolve::Held> {
        for (position, param) in params.iter().enumerate() {
            let position = u16::try_from(position).ok()?;
            // The docblock first: a template is only ever written there, and a hint beside it --
            // `ReflectionClass::__construct(object|string $objectOrClass)` -- is what the effective
            // type prefers when the docblock names something it cannot relate to the hint, which
            // is exactly what a template looks like to it.
            for ty in [param.doc_ty, param.effective_ty(self.skeleton.types())]
                .into_iter()
                .flatten()
            {
                if let Some(answer) = self.bound_through(ty, template, params, position, at) {
                    return answer;
                }
            }
        }
        None
    }

    /// What one parameter's type binds `template` to: `None` when it does not mention it, and
    /// `Some(None)` when it does and the argument says nothing -- which ends the search, since the
    /// parameter that names a template is the one to read.
    fn bound_through(
        &self,
        ty: TypeId,
        template: &str,
        params: &[phrost_sema::ParamRow],
        position: u16,
        at: &crate::symbols::CallFacts<'_>,
    ) -> Option<Option<crate::resolve::Held>> {
        if self.names_template(ty, template) {
            return Some(self.argument_held(at, params, position));
        }
        // `@param class-string<T> $name` -- the argument *names* the class rather than being
        // one, so what fills the template is written at the call and read from the literal. The
        // factory shape, and the commonest generic in modern PHP: a container, a repository, a
        // `createMock`. Before the container check below it, because both are a generic over a
        // builtin and only the base tells them apart -- read as a container this asked for the
        // element of a string and answered nothing.
        if self
            .class_string_of_stored(ty)
            .is_some_and(|argument| self.names_template(argument, template))
        {
            return Some(
                self.class_named_at(at, params, position)
                    .map(crate::resolve::Held::Class),
            );
        }
        // `@param (Closure(Connection): TReturn) $callback` -- what the closure handed over
        // returns. BookStack's `new DatabaseTransaction(fn () => $transformer->toBook($chapter))`
        // is a transaction of a `Book`, and its `run(): mixed` documented `@return TReturn` says so
        // only if the constructor's template is bound by the closure.
        if self
            .stored_callable_return(ty)
            .is_some_and(|returned| self.names_template(returned, template))
        {
            let Some(written) = at.written_argument(params, position) else {
                return Some(None);
            };
            return Some(self.callback_held(at, written));
        }
        // `@param Builder<T> $query` names a class that is no container, so `T` is what the
        // argument holds at that position: in the arguments it was written with, or the ones its
        // class gives this one. Read as a container's element below, it was the last argument --
        // right only while `T` happens to be last, and `MorphTo<*, TDeclaringModel>` handed a bare
        // `Relation` was bound by iterating the relation.
        if let Some((base, slot)) = self.class_argument_naming(ty, template) {
            let Some(passed) = self.argument_held(at, params, position) else {
                return Some(None);
            };
            return Some(self.argument_in_slot(at, &passed, &base, slot));
        }
        // `@param T[] $items` -- the argument is a container of it, so what it holds is what the
        // template is.
        if self
            .element_of_stored(ty)
            .is_some_and(|element| self.names_template(element, template))
        {
            let Some(passed) = self.argument_held(at, params, position) else {
                return Some(None);
            };
            // A literal is a shape, and a shape has no element of its own -- a keyed read that
            // misses must stay a miss -- so what `new Coll([new Item()])` holds is its values.
            // The container a *variable* holds answers as an element, and that is every other
            // spelling of the same call.
            return Some(self.value_contained(at.offset, at.hir, &passed));
        }
        // `@param array<K, V> $array` -- the other half of the same container, and the one
        // `ArrayIterator` binds its `K` from. What a key is follows from how the argument was
        // written rather than from anything a template can name: `[1, 2, 3]` is keyed by `int` and
        // `['a' => 1]` by `string`, the question `foreach ($x as $k => $v)` asks. Unread, `new
        // ArrayIterator([1, 2, 3])` was keyed by the bound, `array-key`, and its `key()` an
        // `int|string` no `int` parameter takes.
        if self
            .iterated_argument(ty, crate::iteration::Bound::Key)
            .is_some_and(|key| self.names_template(key, template))
        {
            let Some(passed) = self.argument_held(at, params, position) else {
                return Some(None);
            };
            return Some(self.iteration_key_of_held(at.offset, at.hir, &passed));
        }
        // A union of those spellings: `class-string<T>|T` on `ReflectionClass`, and
        // `Arrayable<TKey, TValue>|iterable<TKey, TValue>|null` on Laravel's collection. The
        // argument is one operand, and the one that answers is the one it was -- a class named
        // by a string first, since a `Foo::class` read as the bare `T` is a string.
        let types = self.skeleton.types();
        let operands: Vec<TypeId> = match types.get(ty) {
            phrost_ty::TypeRef::Union(parts) => types.operands(*parts).to_vec(),
            phrost_ty::TypeRef::Nullable(inner) => vec![*inner],
            _ => return None,
        };
        let mut spoke = false;
        let (named, rest): (Vec<TypeId>, Vec<TypeId>) = operands
            .into_iter()
            .partition(|&operand| self.class_string_of_stored(operand).is_some());
        for operand in named.into_iter().chain(rest) {
            match self.bound_through(operand, template, params, position, at) {
                // Beside other spellings a bare `T` is the instance case, and what binds it is an
                // object: a string the class-string half could not read is no instance of the class,
                // and neither is the `true` or the callable some other operand was written for.
                Some(Some(found))
                    if self.names_template(operand, template)
                        && self.kind_of_held(at.hir, &found)
                            != Some(phrost_model::BuiltinTy::Object) =>
                {
                    spoke = true;
                }
                Some(Some(found)) => return Some(Some(found)),
                Some(None) => spoke = true,
                None => {}
            }
        }
        spoke.then_some(None)
    }

    /// The class a parameter's type is a generic over, and which of its arguments is `template`,
    /// where that class is one PHP does not traverse -- see `iterates_its_properties`.
    fn class_argument_naming(&self, ty: TypeId, template: &str) -> Option<(Name, usize)> {
        let types = self.skeleton.types();
        let phrost_ty::TypeRef::Generic { base, args, .. } = types.get(ty) else {
            return None;
        };
        let phrost_ty::TypeRef::Named(named) = types.get(*base) else {
            return None;
        };
        let slot = types
            .operands(*args)
            .iter()
            .position(|&argument| self.names_template(argument, template))?;
        let class = self.class_location(named.as_str())?.fqn;
        self.iterates_its_properties(class.as_str())
            .then_some((class, slot))
    }

    /// What `passed` holds at `base`'s argument `slot`: what it was written with when it is a
    /// `base` itself, and otherwise what its class gives `base` there.
    fn argument_in_slot(
        &self,
        at: &crate::symbols::CallFacts<'_>,
        passed: &crate::resolve::Held,
        base: &Name,
        slot: usize,
    ) -> Option<crate::resolve::Held> {
        let class = self.class_of_held(at.offset, at.hir, passed)?;
        if class.as_str().eq_ignore_ascii_case(base.as_str()) {
            return self.arguments_of_held(at.hir, passed).get(slot).cloned();
        }
        self.supplied_argument(class.as_str(), base.as_str(), slot)
            .map(|ty| crate::resolve::Held::Stored { ty, scope: None })
    }

    /// Whether an interned type is exactly the template named `written`.
    fn names_template(&self, ty: TypeId, written: &str) -> bool {
        match self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Named(named) => short_name(named.as_str()) == written,
            _ => false,
        }
    }

    /// The class the argument at `position` names, when it names one.
    ///
    /// Read from the *expression* rather than from a literal list, because a `new` carries none: a
    /// method step keeps its leading literals and `PathRoot::Instantiated` keeps only the call. So
    /// `Foo::class` is resolved the way it is anywhere else -- as a path whose root names a class --
    /// which also means `static::class` and an imported alias answer without a case of their own.
    pub(crate) fn class_named_at(
        &self,
        at: &crate::symbols::CallFacts<'_>,
        params: &[phrost_sema::ParamRow],
        position: u16,
    ) -> Option<Name> {
        let written = at.written_argument(params, position)?;
        let path = phrost_body::value_path_of(at.hir, written)?;
        if !path.steps.is_empty() {
            return None;
        }
        match &path.root {
            phrost_body::PathRoot::ClassName(target) => {
                self.class_of_target(at.offset, at.hir, *target)
            }
            _ => None,
        }
    }

    /// The argument of an interned `class-string<T>`, which is the class the value *names*.
    ///
    /// Told from a container by its base: lowering keeps a `class-string<T>` as a generic over the
    /// `string` builtin, exactly as it keeps `T[]` over `array` -- see
    /// `phrost_lower::docblock::type_lowering::Lowerer::class_string`. Only the base separates the
    /// two, and reading one as the other asks for the element of a string.
    fn class_string_of_stored(&self, ty: TypeId) -> Option<TypeId> {
        let types = self.skeleton.types();
        let phrost_ty::TypeRef::Generic { base, args, .. } = types.get(ty) else {
            return None;
        };
        matches!(
            types.get(*base),
            phrost_ty::TypeRef::Builtin(phrost_model::BuiltinTy::String)
        )
        .then(|| types.operands(*args).first().copied())
        .flatten()
    }

    /// The last type argument of an interned container, which is what iterating it yields.
    pub(crate) fn element_of_stored(&self, ty: TypeId) -> Option<TypeId> {
        match self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Generic { args, .. } => {
                self.skeleton.types().operands(*args).last().copied()
            }
            _ => None,
        }
    }

    /// Which of `class_fqn`'s own `@template` parameters `ty` names, if it names one.
    pub(crate) fn template_position(&self, class_fqn: &str, ty: TypeId) -> Option<usize> {
        let phrost_ty::TypeRef::Named(named) = self.skeleton.types().get(ty) else {
            return None;
        };
        let written = short_name(named.as_str());
        self.class_templates(class_fqn)?
            .iter()
            .position(|template| template.name.as_str() == written)
    }

    /// What `class_fqn` gives `ancestor` for its template at `position`.
    ///
    /// For a caller that knows the position rather than the name: what a container yields is its
    /// last template, whatever it is called.
    pub(crate) fn supplied_argument(
        &self,
        class_fqn: &str,
        ancestor: &str,
        position: usize,
    ) -> Option<TypeId> {
        self.type_argument(class_fqn, ancestor, position, CHASE, Landing::Bound)
            .filter(|found| self.names_something_real(*found))
    }

    /// What `class_fqn` gives `ancestor`'s template at `position`, through the classes PHP has it
    /// inherit from -- and only where they say nothing, through the `@mixin`s it writes as well.
    ///
    /// A written `@mixin` supplies its class's templates as an `@extends` does: `Relation` writes
    /// `@mixin Builder<TRelatedModel>` and the builder `@use BuildsQueries<TModel>`, so what a
    /// relation's `first()` returns -- `BuildsQueries`' `TValue` -- is the relation's own
    /// `TRelatedModel`, two tags apart. Second, and its own walk: a mixin reached among the
    /// inherited ones answers too, and the first walk with mixins in it took the one an interface
    /// above `Relation` writes -- the query builder's, `@use BuildsQueries<\stdClass>` -- and
    /// typed 192 of BookStack's relation reads as `stdClass`. So inheritance first, unchanged, and
    /// in the second walk a class's own mixins before anything it inherits.
    pub(crate) fn type_argument(
        &self,
        class_fqn: &str,
        ancestor: &str,
        position: usize,
        depth: u8,
        landing: Landing,
    ) -> Option<TypeId> {
        self.type_argument_through(class_fqn, ancestor, position, depth, landing, false)
            .or_else(|| {
                self.type_argument_through(class_fqn, ancestor, position, depth, landing, true)
            })
    }

    fn type_argument_through(
        &self,
        class_fqn: &str,
        ancestor: &str,
        position: usize,
        depth: u8,
        landing: Landing,
        mixins: bool,
    ) -> Option<TypeId> {
        let depth = depth.checked_sub(1)?;
        if class_fqn.eq_ignore_ascii_case(ancestor) {
            return None;
        }

        if let Some(written) = self.written_argument(class_fqn, ancestor, position) {
            return Some(written);
        }

        // Written further up, and in terms of a template *this* class supplies: `Foo extends
        // Second<Baz>` where `Second<T> extends First<T>` gives `First` a `Baz` without saying so.
        let header = self.class_header(class_fqn)?;
        let written_mixins = match mixins {
            true => self
                .class_declaration(class_fqn)
                .map(|declaration| {
                    declaration
                        .mixins()
                        .iter()
                        .map(|mixin| mixin.fqn.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            false => Vec::new(),
        };
        for parent in written_mixins
            .iter()
            .chain(&header.extends)
            .chain(&header.implements)
            .chain(&header.uses)
        {
            if let Some(argument) = self.type_argument_through(
                parent.as_str(),
                ancestor,
                position,
                depth,
                landing,
                mixins,
            ) {
                return match landing {
                    Landing::Bound => Some(
                        self.substituted(class_fqn, parent.as_str(), argument, depth)
                            .unwrap_or(argument),
                    ),
                    Landing::Template => match self.template_named(parent.as_str(), argument) {
                        Some(at) => self.type_argument_through(
                            class_fqn,
                            parent.as_str(),
                            at,
                            depth,
                            landing,
                            mixins,
                        ),
                        None => Some(argument),
                    },
                };
            }
        }

        None
    }

    /// The last argument a supertype tag actually wrote for `ancestor`.
    ///
    /// Not the last template the ancestor declares, which is a different thing when the tag writes
    /// fewer: `@implements IteratorAggregate<Command>` gives one argument to a two-parameter
    /// interface, and it means the value. `<Key, Value>` is the only pair anybody writes, so one
    /// argument is the second half of it.
    pub(crate) fn last_written_argument(&self, class_fqn: &str, ancestor: &str) -> Option<TypeId> {
        self.written_arguments_for(class_fqn, ancestor)
            .last()
            .copied()
    }

    /// Every argument a supertype tag wrote for `ancestor`, in the order written.
    ///
    /// The count is part of the answer, which is why this hands back the whole list rather than one
    /// position: a tag may write fewer arguments than the interface declares, and *which* of them
    /// means what then depends on how many there are. Reading the tag is this module's; deciding
    /// what a count implies belongs to whoever knows the interface, which is
    /// [`iteration`](crate::iteration).
    ///
    /// Empty when the declaration named `ancestor` without arguments, and when it did not name it
    /// at all -- the same answer to a caller, because neither said anything about what is held.
    pub(crate) fn written_arguments_for(&self, class_fqn: &str, ancestor: &str) -> &'a [TypeId] {
        let types = self.skeleton.types();
        let Some(declaration) = self.class_declaration(class_fqn) else {
            return &[];
        };
        for &id in self.supertype_arguments_of(class_fqn, declaration) {
            let phrost_ty::TypeRef::Generic { base, args, .. } = types.get(id) else {
                continue;
            };
            let phrost_ty::TypeRef::Named(named) = types.get(*base) else {
                continue;
            };
            if named.as_str().eq_ignore_ascii_case(ancestor) {
                return types.operands(*args);
            }
        }
        &[]
    }

    fn written_argument(&self, class_fqn: &str, ancestor: &str, position: usize) -> Option<TypeId> {
        let types = self.skeleton.types();
        let declaration = self.class_declaration(class_fqn)?;
        for &id in self.supertype_arguments_of(class_fqn, declaration) {
            let phrost_ty::TypeRef::Generic { base, args, .. } = types.get(id) else {
                continue;
            };
            let phrost_ty::TypeRef::Named(named) = types.get(*base) else {
                continue;
            };
            if named.as_str().eq_ignore_ascii_case(ancestor) {
                return types.operands(*args).get(position).copied();
            }
        }
        None
    }

    /// The arguments a class's supertype tags wrote, and those of the `@mixin` tags a framework says
    /// it behaves as though it wrote -- which are read exactly as a written one's are.
    fn supertype_arguments_of(
        &self,
        class_fqn: &str,
        declaration: &'a phrost_index::ClassSkeleton,
    ) -> impl Iterator<Item = &'a TypeId> {
        declaration
            .supertype_arguments()
            .iter()
            .chain(self.skeleton.returns().implied_mixins_of(class_fqn).1)
    }

    /// Whether a substitution's result is worth handing back.
    ///
    /// A chain can run out halfway: `Container<V>` extended by a class that supplies nothing leaves
    /// `V`, which is a name and not a class. Handing it over turns an honest "no type" into a claim
    /// about a class nothing declares -- and on a real project that was twenty-one member accesses
    /// reported against a class called `V`.
    ///
    /// Under a `?` too: `?V` is the same name with a null beside it, and a conditional return's
    /// `TRelated|null` branch interns to exactly that.
    pub(crate) fn names_something_real(&self, ty: TypeId) -> bool {
        match self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Named(named) => self.class_location(named.as_str()).is_some(),
            phrost_ty::TypeRef::Nullable(inner) => self.names_something_real(*inner),
            _ => true,
        }
    }

    /// The `@template` parameters a class declares, in the order written.
    pub(crate) fn class_templates(&self, fqn: &str) -> Option<&'a [phrost_index::TemplateRow]> {
        Some(self.class_declaration(fqn)?.templates())
    }

    /// The stored declaration of a class, for the parts the member tables do not carry.
    ///
    /// Memoized for the life of the view, because finding one is a scan of every class the file
    /// declares and this is asked for every member whose type is a name and for every `new`. Folded,
    /// because PHP looks classes up case-insensitively and two spellings must not scan twice.
    pub(crate) fn class_declaration(&self, fqn: &str) -> Option<&'a phrost_index::ClassSkeleton> {
        let key: Box<str> = fqn.to_ascii_lowercase().into();
        if let Some(hit) = self.declarations.borrow().get(&key) {
            return *hit;
        }
        let location = self.class_location(fqn);
        let found = location.and_then(|location| {
            self.skeleton
                .file(location.file)?
                .classes()
                .iter()
                .find(|class| class.fqn() == &location.fqn)
        });
        self.declarations.borrow_mut().insert(key, found);
        found
    }

    /// The class a `@template` bound names, for a caller that wants the fallback rather than the
    /// argument.
    pub(crate) fn template_bound(&self, fqn: &str) -> Option<Name> {
        self.class_templates(fqn)?
            .iter()
            .rev()
            .find_map(|template| self.class_of_stored(template.bound?, None))
    }
}
