//! What iterating a class yields.
//!
//! An array says what it holds in its own type — `array<Item>` — and a class cannot. PHP has no
//! generics, so a container states it in a docblock: `@template T of FieldItemInterface`. Until this
//! existed, `foreach ($entity->get($field) as $item)` gave `$item` no type at all, which was the
//! single largest gap on the corpus at 149 positions.
//!
//! # Which argument is the value
//!
//! The shapes do not agree, so there is a table of them — [`CONTRACTS`] — and it is the only place
//! that knows. `Iterator<TKey, TValue>` puts the value second of two and `Generator<TKey, TValue,
//! TSend, TReturn>` puts it second of *four*, so "the last argument" reads a finished generator's
//! return type instead of what a `foreach` over it binds. Naming the interfaces is what makes that
//! difference expressible; a positional guess cannot express it at all.
//!
//! Everything not in the table still gets the guess — the last template — and the guess is kept
//! safe by refusing anything that is not a class. A class declaring one `@template TKey` and handing
//! it to `Iterator` as the *key* has an argument at that position, and it is the key: stopping on it
//! would be a confident wrong answer, and failing to name it as a class is what keeps the walk going
//! up to the `Iterator` that does say. That refusal is load-bearing, not conservatism.
//!
//! # The bound, and why that is honest
//!
//! A `@template` is a parameter, and the argument it is given is written on a subclass —
//! `@extends ListInterface<FileItem>` — often several levels up a chain. Substituting through that
//! chain is a much larger job and is not done here. What is used instead is the *bound*: the
//! constraint the declaration puts on every argument it could ever be given.
//!
//! That direction is the safe one. The bound is a supertype of the real element, so its member table
//! is a subset of the real one: every member offered exists, and some that exist are missing. A miss
//! rather than a wrong answer — and a wrong answer here would be a member a reader is told to call
//! on a value that does not have it.

use phrost_model::{BuiltinTy, Name};
use phrost_ty::TypeId;

use crate::{MemberKind, resolve::Held, view::IndexView};

/// How far up a hierarchy to look for a declaration that says what is held.
///
/// A bound, not a guess about depth: PHP hierarchies are shallow and a cycle in a broken file must
/// not be walked forever.
const DEPTH: usize = 16;

/// A contract PHP itself declares, whose type arguments state what iterating a value of it yields.
///
/// Named rather than positional because the arities disagree and the value is not always last. Only
/// classes go in here: `array<K, V>` and `iterable<V>` are builtins, never appear in the catalog
/// this walks, and are read straight off the written type — where the last argument *is* the value,
/// because neither has a shape that puts it anywhere else.
struct IterationContract {
    /// As PHP spells it. Every one of them is global, so there is no namespace to match against.
    fqn: &'static str,
    /// How many `@template` parameters the declaration carries.
    arity: usize,
    /// Which of them a `foreach` binds to the value.
    value: usize,
    /// Which of them it binds to the key.
    ///
    /// First in all four, and stated rather than assumed for the reason the value is: the arities
    /// disagree, and a rule that reads a position has to say which position it read.
    key: usize,
    /// The method the contract answers through, for a class that stated nothing in a type argument.
    ///
    /// PHP's own semantics rather than a convention. `IteratorAggregate` says a `foreach` over the
    /// object iterates whatever `getIterator()` gives back, and `Iterator` says the value is
    /// `current()` and the key is `key()` -- so a class that writes those signatures has said what
    /// it yields as surely as an `@implements` tag would, and far more often: all three of one real
    /// project's own collections write `@return \\Traversable<string, X>` on `getIterator()` and none
    /// writes the tag.
    ///
    /// `None` for a contract that declares no method to ask. `Traversable` declares none at all, and
    /// a `Generator` is a class no project implements -- reaching it means the value being iterated
    /// *is* one, which its own type arguments answer.
    through: Option<Forwarding>,
}

/// Where a class's declarations put what a `foreach` binds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum IteratedSlot {
    /// One of the value's own type arguments, by position.
    Argument(usize),
    /// A type the tags wrote outright, whatever the value was given: `SplObjectStorage`'s `int` key.
    Fixed(TypeId),
}

impl IteratedSlot {
    /// Which of the `written` arguments a slot at `position` reads.
    ///
    /// That one -- except where one argument was written to a class whose value is not the first of
    /// its templates. `Collection<Page>` gives a `<TKey, TValue>` class one argument, and it is the
    /// value: a collection of pages, whose keys it does not say. Read by position, the page was the
    /// key and the value nothing, so `foreach ($chapter->getVisiblePages() as $page)` typed nothing --
    /// and PHPStan, asked the same question on BookStack, answers `Page` for the value and
    /// `int|string` for the key.
    pub(crate) fn written_position(position: usize, bound: Bound, written: usize) -> Option<usize> {
        match (written, bound) {
            (1, Bound::Value) => Some(0),
            (1, Bound::Key) => None,
            _ => (position < written).then_some(position),
        }
    }
}

/// Which of the two things a `foreach` binds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Bound {
    /// The `$v` of `foreach ($c as $k => $v)`.
    Value,
    /// The `$k`.
    Key,
}

/// How a contract's own method answers what iterating yields.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Forwarding {
    /// `IteratorAggregate::getIterator()` -- iterate whatever it gives back, which is a second
    /// question about a second value rather than the answer.
    Iterating(&'static str),
    /// `Iterator::current()` and `key()` -- the return *is* the answer, one method per bound.
    Returning {
        value: &'static str,
        key: &'static str,
    },
}

/// Every interface whose value slot is known by name. Four, because PHP has four.
const CONTRACTS: &[IterationContract] = &[
    // The root of everything PHP will iterate. `Traversable<TKey, TValue>` — the pair the other
    // three restate, and the only one a class can reach without naming any of them.
    IterationContract {
        fqn: "Traversable",
        arity: 2,
        value: 1,
        key: 0,
        // The interface declares no method: `Traversable` exists to be type-hinted and cannot be
        // implemented directly.
        through: None,
    },
    // `Iterator<TKey, TValue>`: `key()` gives the first and `current()` the second.
    IterationContract {
        fqn: "Iterator",
        arity: 2,
        value: 1,
        key: 0,
        through: Some(Forwarding::Returning {
            value: "current",
            key: "key",
        }),
    },
    // `IteratorAggregate<TKey, TValue>`: the same pair, forwarded to whatever `getIterator()`
    // returns.
    IterationContract {
        fqn: "IteratorAggregate",
        arity: 2,
        value: 1,
        key: 0,
        through: Some(Forwarding::Iterating("getIterator")),
    },
    // `Generator<TKey, TValue, TSend, TReturn>`. Two more parameters than the rest and neither is
    // about iterating — `TSend` is what `send()` takes, `TReturn` what `getReturn()` gives once the
    // generator is done — so the last argument is the one answer a `foreach` never binds.
    IterationContract {
        fqn: "Generator",
        arity: 4,
        value: 1,
        key: 0,
        through: None,
    },
];

impl IterationContract {
    /// The contract a class name is, when it is one of them.
    fn named(fqn: &str) -> Option<&'static Self> {
        let written = fqn.trim_start_matches('\\');
        CONTRACTS
            .iter()
            .find(|contract| contract.fqn.eq_ignore_ascii_case(written))
    }

    /// Which slot a bound reads, when the tag wrote the whole pair.
    fn slot(&self, bound: Bound) -> usize {
        match bound {
            Bound::Value => self.value,
            Bound::Key => self.key,
        }
    }

    /// Which of the arguments a tag wrote answers `bound`, given how many it wrote.
    ///
    /// The declared slot whenever the tag stayed inside the declaration, and the *first* when it
    /// wrote exactly one — `@implements IteratorAggregate<Command>` is the commonest spelling in
    /// real code and it means the value, not the key. More arguments than the interface declares is
    /// a docblock describing something else, and no slot in it is worth guessing at.
    ///
    /// Which makes one argument say nothing at all about the key, and that is the half worth
    /// stating: reading position zero for the key without counting first would name every pass of
    /// the loop after `Command`.
    fn written_slot(&self, bound: Bound, written: usize) -> Option<usize> {
        match (bound, written) {
            (_, 0) => None,
            (Bound::Value, 1) => Some(0),
            (Bound::Key, 1) => None,
            (_, written) if written <= self.arity => Some(self.slot(bound)),
            _ => None,
        }
    }

    /// The method this contract answers `bound` through, and whether what it gives back is itself
    /// something to iterate.
    fn forwarded(&self, bound: Bound) -> Option<Forwarded> {
        match self.through? {
            Forwarding::Iterating(method) => Some(Forwarded {
                method,
                iterate: true,
            }),
            Forwarding::Returning { value, key } => Some(Forwarded {
                method: match bound {
                    Bound::Value => value,
                    Bound::Key => key,
                },
                iterate: false,
            }),
        }
    }
}

/// A method to ask, and what its return means.
struct Forwarded {
    method: &'static str,
    /// Whether the return is the container to iterate rather than the answer itself.
    iterate: bool,
}

impl IndexView<'_> {
    /// What an instance of `fqn` yields when iterated, from the nearest declaration above it that
    /// says.
    ///
    /// A held type rather than a class, because a container is allowed to hold something no class
    /// name can express: `Iterator<TKey, string>` yields a `string` and
    /// `IteratorAggregate<array<string, Foo>>` yields an array, and both used to come back as
    /// nothing at all.
    ///
    /// Nothing when no declaration in the hierarchy carries one, which is most classes — and
    /// nothing is the right answer there, because iterating them yields something this layer cannot
    /// name.
    pub(crate) fn iterated_element(&self, fqn: &str) -> Option<Held> {
        self.iterated(fqn, Bound::Value, &mut Vec::new())
    }

    /// What iterating an instance of `fqn` names each pass by: the `$k` of
    /// `foreach ($bag as $k => $row)`.
    ///
    /// The key half of the same walk, and it has to be a walk for the same reason: a class states
    /// its key where it states its element, which is a tag on some declaration above it or the
    /// signature of its own `key()`. Nothing is guessed here that is not guessed for the value —
    /// and rather less, because the *convention* half does not apply. A container that is not one of
    /// PHP's own puts the value in its last template by convention and says nothing anybody agrees
    /// on about a key, so a class outside the four contracts answers nothing at all.
    pub(crate) fn iterated_key(&self, fqn: &str) -> Option<Held> {
        self.iterated(fqn, Bound::Key, &mut Vec::new())
    }

    /// What a `foreach` over a value of `class` binds to `bound`, as the class's declarations say it:
    /// one of the value's own type arguments, or a type the tags fixed.
    ///
    /// The question a value's arguments need answered before any of them can be read:
    /// `SplObjectStorage<Item, Meta>` yields `Item` and is keyed by `int`, because the stub writes
    /// `@template-implements SeekableIterator<int, O>` and `O` is its first template -- and the
    /// convention of taking the last argument answered `Meta`, which is what the storage maps each
    /// object *to*, and named every key after the first. Followed up the tags, through classes that
    /// restate a contract in their own templates, until a contract names the slot.
    ///
    /// `None` when nothing between the class and a contract says, which leaves the convention.
    pub(crate) fn iterated_slot(&self, class: &str, bound: Bound) -> Option<IteratedSlot> {
        self.iterated_slot_within(class, bound, DEPTH)
    }

    fn iterated_slot_within(
        &self,
        class: &str,
        bound: Bound,
        depth: usize,
    ) -> Option<IteratedSlot> {
        let depth = depth.checked_sub(1)?;
        let types = self.skeleton.types();
        self.class_declaration(class)?
            .supertype_arguments()
            .iter()
            .find_map(|&tag| {
                let phrost_ty::TypeRef::Generic { base, args, .. } = types.get(tag) else {
                    return None;
                };
                let phrost_ty::TypeRef::Named(named) = types.get(*base) else {
                    return None;
                };
                let written = types.operands(*args);
                let slot = match IterationContract::named(named.as_str()) {
                    Some(contract) => contract.written_slot(bound, written.len())?,
                    None => match self.iterated_slot_within(named.as_str(), bound, depth)? {
                        IteratedSlot::Argument(position) => position,
                        fixed @ IteratedSlot::Fixed(_) => return Some(fixed),
                    },
                };
                let ty = *written.get(slot)?;
                match self.template_position(class, ty) {
                    Some(position) => Some(IteratedSlot::Argument(position)),
                    // Somebody else's template -- a parent's `TValue` the tag did not restate --
                    // is a name for nothing, and handing it back would report members of `TValue`.
                    None => self
                        .names_something_real(ty)
                        .then_some(IteratedSlot::Fixed(ty)),
                }
            })
    }

    /// The walk both halves share.
    ///
    /// `forwarded` is the classes whose own `getIterator()` has already been followed on this
    /// question. A collection whose iterator is itself — which is legal, and is what
    /// `IteratorAggregate` over `$this` looks like — would otherwise ask the same class forever.
    fn iterated(&self, fqn: &str, bound: Bound, forwarded: &mut Vec<Name>) -> Option<Held> {
        if self.iterates_its_properties(fqn) {
            return None;
        }
        // Seeded from the catalog rather than from the string, because a `Name` is interned and
        // this layer has no interner -- and the catalog's spelling is the canonical one anyway.
        let mut queue = vec![self.class_location(fqn)?.fqn];
        let mut seen = Vec::new();

        for _ in 0..DEPTH {
            let mut next = Vec::new();
            for current in queue.drain(..) {
                if seen.contains(&current) {
                    continue;
                }
                seen.push(current.clone());

                if let Some(element) =
                    self.declared_element(fqn, current.as_str(), bound, forwarded)
                {
                    return Some(element);
                }

                // Interfaces before the parent class: the container contract is nearly always the
                // interface, and a concrete class inherits the parameter rather than restating it.
                if let Some(header) = self.class_header(current.as_str()) {
                    next.extend(header.implements);
                    next.extend(header.extends);
                    next.extend(header.uses);
                }
            }
            if next.is_empty() {
                return None;
            }
            queue = next;
        }

        None
    }

    /// What one class in `fqn`'s hierarchy says a value of `fqn` yields.
    ///
    /// Two rules, and which applies is decided by whether the class is one PHP declares. A contract
    /// names its value slot, so whatever fills it is the answer whatever kind of type it is. Anything
    /// else states the position by convention only, and a convention is safe to follow only while a
    /// wrong answer cannot be named -- see the note at the top of this module.
    fn declared_element(
        &self,
        fqn: &str,
        container: &str,
        bound: Bound,
        forwarded: &mut Vec<Name>,
    ) -> Option<Held> {
        if let Some(contract) = IterationContract::named(container) {
            return self
                .contract_element(fqn, contract, bound)
                .or_else(|| self.through_a_method(fqn, contract, bound, forwarded));
        }

        // The convention states a value and nothing else. `<Key, Value>` is the pair people write,
        // so the last template is the value — and there is no second position to read, because a
        // container writing one template has written the value.
        if bound == Bound::Key {
            return None;
        }

        // The argument first, and the bound only when nothing supplied one. `Foo implements
        // ListInterface<Item>` says what iterating a `Foo` yields; the bound says only what every
        // argument the container could take has in common.
        //
        // Guarded like everything else here, and this is where a bare `\ArrayIterator` used to
        // answer a class called `V`: the stub writes `@template-implements SeekableIterator<K, V>`,
        // so the convention finds an argument and the argument is the stub's own template.
        self.supplied_element(fqn, container)
            .or_else(|| self.template_bound(container))
            .map(Held::Class)
            .and_then(|held| self.stated(held))
    }

    /// What the class's own signature for the contract's method says.
    ///
    /// PHP's semantics, read where a tag was not written: a `foreach` over an `IteratorAggregate`
    /// iterates what `getIterator()` returns, and over an `Iterator` binds `current()` and `key()`.
    /// So a class stating either in a return type or a `@return` has said what it yields, and that
    /// is how collections are actually written — `@implements` is a tag for static analysers and
    /// most code does not carry it.
    ///
    /// Asked only after the type arguments, which stay the more specific claim: `@implements
    /// IteratorAggregate<int, Special>` over a `getIterator(): \Traversable` means `Special`.
    fn through_a_method(
        &self,
        fqn: &str,
        contract: &IterationContract,
        bound: Bound,
        forwarded: &mut Vec<Name>,
    ) -> Option<Held> {
        let asked = contract.forwarded(bound)?;
        let held = self.member_held_on(fqn, MemberKind::Method, asked.method, None, &[])?;
        match asked.iterate {
            true => self.iterating(&held, bound, forwarded),
            false => self.stated(self.while_valid(held)),
        }
    }

    /// `current()` and `key()` without the null they return past the end of the sequence.
    ///
    /// The declaration describes calling the method, and a `foreach` is not that: PHP calls
    /// `rewind()`, then `valid()`, and reads `current()` only while it answered true — so the null
    /// an `Iterator` returns when there is nothing left is unreachable at the one site this type is
    /// being read for. WordPress writes `@return WP_Block|null` on `WP_Block_List::current()` for
    /// exactly that reason, and believing it reported 29 `possibly-null-receiver` on loop bodies
    /// that cannot see a null.
    ///
    /// Only for the method route. A container that says `@implements Iterator<int, ?Row>` is
    /// describing what it *holds*, and that null is the loop's business.
    fn while_valid(&self, held: Held) -> Held {
        match held {
            Held::Nullable(inner) => *inner,
            Held::Stored { ty, scope } => match self.skeleton.types().get(ty) {
                phrost_ty::TypeRef::Nullable(inner) => Held::Stored { ty: *inner, scope },
                _ => Held::Stored { ty, scope },
            },
            other => other,
        }
    }

    /// What iterating the value a `getIterator()` gave back yields.
    ///
    /// Its own read rather than a call back into `element_of_held`, and the reason is the second
    /// half of what that function does: it resolves a written type against a position in a file,
    /// which is a question about the buffer and not about this class. What arrives here is a
    /// member's declared type, which is stored — so the two shapes worth handling are a generic
    /// (`\Generator<int, Row>`) and a plain class (`ArrayIterator`, asked again).
    fn iterating(&self, held: &Held, bound: Bound, forwarded: &mut Vec<Name>) -> Option<Held> {
        match held {
            Held::Nullable(inner) => self.iterating(inner, bound, forwarded),
            Held::Class(fqn) => self.forwarding_to(fqn.clone(), bound, forwarded),
            Held::Stored { ty, scope } => {
                if let Some(argument) = self.iterated_argument(*ty, bound) {
                    return self.stated(Held::Stored {
                        ty: argument,
                        scope: scope.clone(),
                    });
                }
                let class = self.class_of_stored(*ty, scope.as_ref())?;
                self.forwarding_to(class, bound, forwarded)
            }
            _ => None,
        }
    }

    /// Asks `fqn` the same question, unless this walk has asked it already.
    fn forwarding_to(&self, fqn: Name, bound: Bound, forwarded: &mut Vec<Name>) -> Option<Held> {
        if forwarded.contains(&fqn) || forwarded.len() >= DEPTH {
            return None;
        }
        forwarded.push(fqn.clone());
        self.iterated(fqn.as_str(), bound, forwarded)
    }

    /// The argument a stored generic wrote for `bound`.
    ///
    /// The one reader of a container's type arguments, and it is one because the two rules it holds
    /// disagree. A contract names its slots, so `Generator<TKey, TValue, TSend, TReturn>` written in
    /// full is read by name — where taking the last argument answers `getReturn()`'s type, which is
    /// the one thing a `foreach` never binds. Anything else states its element last by convention,
    /// which is what `<Key, Value>` means and all anyone writes.
    ///
    /// Together in one function because they used to be apart: the table read the contract and
    /// `TypeTable::element_of` took the last, so `\Generator<int, Row, mixed, void>` iterated as
    /// `void` while `@implements Generator<int, Row>` on a class iterated as `Row`.
    pub(crate) fn iterated_argument(&self, ty: TypeId, bound: Bound) -> Option<TypeId> {
        let types = self.skeleton.types();
        match types.get(ty) {
            phrost_ty::TypeRef::Nullable(inner) => self.iterated_argument(*inner, bound),
            phrost_ty::TypeRef::Generic { base, args, .. } => {
                let operands = types.operands(*args);
                if let phrost_ty::TypeRef::Named(named) = types.get(*base)
                    && let Some(contract) = IterationContract::named(named.as_str())
                {
                    return operands
                        .get(contract.written_slot(bound, operands.len())?)
                        .copied();
                }
                if let phrost_ty::TypeRef::Named(named) = types.get(*base)
                    && let Some(slot) = self.iterated_slot(named.as_str(), bound)
                {
                    return match slot {
                        IteratedSlot::Argument(position) => operands
                            .get(IteratedSlot::written_position(
                                position,
                                bound,
                                operands.len(),
                            )?)
                            .copied(),
                        IteratedSlot::Fixed(ty) => Some(ty),
                    };
                }
                if let phrost_ty::TypeRef::Named(named) = types.get(*base)
                    && self.iterates_its_properties(named.as_str())
                {
                    return None;
                }
                match bound {
                    Bound::Value => operands.last().copied(),
                    // One argument is the element, so only a written pair states a key.
                    Bound::Key => (operands.len() > 1).then(|| operands[0]),
                }
            }
            _ => None,
        }
    }

    /// Whether a `foreach` over a `fqn` binds its visible properties rather than anything a
    /// declaration states.
    ///
    /// PHP's rule: only a `Traversable` hands a loop what it yields, and any other object hands it
    /// its properties, one per pass. So the last-template convention means nothing on a class that
    /// is not one -- and an Eloquent model iterated as `Factory`, because the walk reached the
    /// `HasFactory<TFactory of Factory>` it uses and took the template's bound. Mixins and traits are
    /// not walked: neither makes a class iterable. A class the index does not hold, and a trait,
    /// whose composing class decides, are not refused, since nothing here can say they are not.
    pub(crate) fn iterates_its_properties(&self, fqn: &str) -> bool {
        if self.is_trait(fqn) {
            return false;
        }
        let Some(location) = self.class_location(fqn) else {
            return false;
        };
        let mut queue = vec![location.fqn];
        let mut seen: Vec<Name> = Vec::new();
        for _ in 0..DEPTH {
            let mut next = Vec::new();
            for current in queue.drain(..) {
                if IterationContract::named(current.as_str()).is_some() {
                    return false;
                }
                if seen.contains(&current) {
                    continue;
                }
                seen.push(current.clone());
                match self.class_header(current.as_str()) {
                    Some(header) => {
                        next.extend(header.implements);
                        next.extend(header.extends);
                    }
                    // Somewhere above is a class nobody indexed, which may be the iterable one.
                    None => return false,
                }
            }
            if next.is_empty() {
                return true;
            }
            queue = next;
        }
        false
    }

    /// A held type only when it says something a reader can use.
    ///
    /// Two things it might not. A `@return V|null` on a stub's `current()` names the container's own
    /// template, which resolves to no class and would report members against something called `V`;
    /// the same guard the tag reader beside this one already applies. And `mixed` is what an
    /// interface writes where it has nothing to say, so handing it back turns a silence into a claim
    /// — which for a `foreach` variable is the difference between no hover and a wrong one.
    fn stated(&self, held: Held) -> Option<Held> {
        match &held {
            Held::Builtin(BuiltinTy::Mixed) => None,
            Held::Stored { ty, .. } if !self.stored_names_something(*ty) => None,
            Held::Class(fqn) if self.class_location(fqn.as_str()).is_none() => None,
            Held::Nullable(inner) => self.stated((**inner).clone()).map(|_| held.clone()),
            _ => Some(held),
        }
    }

    /// Whether a stored type names something the index holds, through however many wrappers.
    ///
    /// [`names_something_real`](IndexView::names_something_real) asks the node it is handed, and a
    /// stub writes `@return V|null` — so the template hides one layer down inside a `Nullable` and
    /// the guard passes it. That is how `ArrayIterator::current()` came back as a class called `V`.
    fn stored_names_something(&self, ty: TypeId) -> bool {
        match self.skeleton.types().get(ty) {
            phrost_ty::TypeRef::Nullable(inner) => self.stored_names_something(*inner),
            phrost_ty::TypeRef::Generic { base, .. } => self.stored_names_something(*base),
            _ => self.names_something_real(ty),
        }
    }

    /// The argument `fqn` gives one of PHP's iteration contracts for that contract's value.
    ///
    /// Two reads, and the order is what a chain needs. The declared position first, resolved through
    /// however many classes lie between `fqn` and the contract -- `Test2 extends Test1<int>` over
    /// `Test1 implements Iterator<TKey, string>` is where the value is written two levels up and in
    /// terms of nothing `Test2` said. Then the tag on `fqn`'s own declaration, which is what answers
    /// when it wrote fewer arguments than the interface declares and no position lines up.
    fn contract_element(
        &self,
        fqn: &str,
        contract: &IterationContract,
        bound: Bound,
    ) -> Option<Held> {
        let ty = self
            .supplied_argument(fqn, contract.fqn, contract.slot(bound))
            .filter(|_| bound == Bound::Value || self.states_the_pair(fqn, contract))
            .or_else(|| self.written_argument_for(fqn, contract, bound))?;
        Some(Held::Stored { ty, scope: None })
    }

    /// Whether whatever supplied the key supplied the value as well.
    ///
    /// The gate on reading a key positionally, and the thing it rules out is one tag: `@implements
    /// IteratorAggregate<Command>` writes a single argument meaning the *value*, and the chase fills
    /// the interface's templates left to right — so the argument lands in `TKey`, travels down the
    /// stub's own `@template-extends Traversable<TKey, TValue>`, and comes back as the name of every
    /// pass of the loop. Counting the arguments does not catch it, because by the time `Traversable`
    /// is asked the count belongs to the stub and is two.
    ///
    /// What does catch it is that a tag stating a pair states both halves. A short tag resolves the
    /// key slot and leaves the value slot holding the interface's own `TValue`, which names no class
    /// and is dropped — so asking for the value is asking whether the key was really a key.
    fn states_the_pair(&self, fqn: &str, contract: &IterationContract) -> bool {
        self.supplied_argument(fqn, contract.fqn, contract.value)
            .is_some()
    }

    /// The argument the declaration of `fqn` itself wrote for `contract`'s `bound`.
    ///
    /// Where [`supplied_argument`](IndexView::supplied_argument) reads a fixed position, this reads
    /// the count first: `@implements IteratorAggregate<Command>` fills a two-parameter interface with
    /// one argument, and there is no position that finds it.
    fn written_argument_for(
        &self,
        fqn: &str,
        contract: &IterationContract,
        bound: Bound,
    ) -> Option<TypeId> {
        let written = self.written_arguments_for(fqn, contract.fqn);
        let ty = *written.get(contract.written_slot(bound, written.len())?)?;
        // A tag can name one of the writer's own templates, which is a name resolving to no class.
        // Handing it back would report members against a class called `TValue`.
        self.names_something_real(ty).then_some(ty)
    }

    /// The class `fqn` supplied for the last template of `container`.
    ///
    /// The last, for the reason [`template_bound`](IndexView::template_bound) takes it: a container
    /// that is not one of PHP's own puts the value there, and `<Key, Value>` is the only pair anyone
    /// writes. A class only, because the position is a convention rather than a declared fact --
    /// see the note at the top of this module for what a builtin at that position would cost.
    fn supplied_element(&self, fqn: &str, container: &str) -> Option<Name> {
        let last = self.class_templates(container)?.len().checked_sub(1)?;
        // The position first, which is right whenever the whole pair was written -- and then the last
        // argument the tag actually wrote, for the far commoner shape where only one was.
        let ty = self
            .supplied_argument(fqn, container, last)
            .or_else(|| self.last_written_argument(fqn, container))?;
        self.class_of_stored(ty, None)
    }
}
