//! The vocabulary the rest of this module speaks.
//!
//! One enum for "a type this view is holding", and the constructors that keep its degenerate forms
//! out. Separate from every sibling because none of them own it: the class questions, the guards,
//! the container reads and the predicates all consume a [`Held`] and none of them decides what one
//! is. Nothing here asks the index anything -- if a function needs the catalog, it belongs next
//! door.

use phrost_hir::{ExprId, TypeRefId};
use phrost_model::{BuiltinTy, Name};
use phrost_ty::TypeId;

use crate::view::IndexView;

/// A type this view is holding while it walks a value path.
///
/// Three arms because a path crosses two boundaries. A `new Foo` names a class and says nothing
/// about what it contains; a declaration in the *current* file is a written reference into that
/// file's arena; anything the index stored is an interned id. The element of either type is still a
/// type, so both have to be expressible — and a bare class has no element, which is the honest
/// answer for `new Foo` iterated.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Held {
    Class(Name),
    /// A type the source stated outright, from a cast. Names no class, so it has no members -- which
    /// is the right answer for `(int) $x`, and a different answer from "not known".
    Builtin(BuiltinTy),
    Written(TypeRefId),
    /// An interned type, plus what `self` and `static` mean where it was declared.
    ///
    /// The scope travels with the type because a stored one has left the file that wrote it: `:
    /// static` on a method in another file means the class the call was made on, and there is
    /// nothing at the cursor to say which that is. Fluent interfaces are built entirely out of this
    /// -- `DataDefinition::create()->setLabel()->setRequired()` is three relative return types in a
    /// row -- so without it a chain dies at the first link.
    /// An array of those. What a variadic parameter holds, and the one shape whose *element* is the
    /// interesting half: it names no class itself.
    ListOf(Box<Held>),
    /// An array whose keys are one thing and whose elements are another, both built here.
    ///
    /// What `array_combine($names, $lengths)` gives back: keyed by what `$names` holds and holding
    /// what `$lengths` does. [`Self::ListOf`] states no key -- and answers `int` when asked for one,
    /// which a list is and this is not -- so the key half needed an arm of its own.
    MapOf {
        key: Box<Held>,
        value: Box<Held>,
    },
    /// An array whose keys are known, each with what it holds.
    ///
    /// What a record literal is: `['id' => 1, 'when' => new DateTimeImmutable()]` says as much
    /// about itself as an `array{id: int, when: \DateTimeImmutable}` docblock would, and until this
    /// existed it arrived here as the bare `array` a literal of *unknown* contents deserves.
    ///
    /// Built rather than pointed at, for the reason [`Self::OneOf`] and [`Self::Applied`] are: the
    /// interned table would have to be open for writing, and one entry of the literal may be a
    /// class written in the file being read, which is a reference into that file's own arena. The
    /// difference from those two is only that the operands are named.
    ///
    /// Never empty: `[]` says nothing a bare `array` does not, and a wrapper around nothing would
    /// make two spellings of one value compare unequal.
    Record(Box<[(Name, Held)]>),
    /// An object whose named properties are known, each with what it holds.
    ///
    /// What an `object{pivot: TPivotModel}` shape becomes once its templates are filled for one
    /// value: Laravel's `BelongsToMany` loads `TRelatedModel&object{pivot: TPivotModel}`, and the
    /// `Pivot` that `->pivot` reads is only known where the relation's arguments are. Built rather
    /// than pointed at for [`Self::Record`]'s reason, and kept apart from it because a record is an
    /// array -- subscripted, iterated, written as a literal -- and this is none of those.
    ///
    /// A shape whose properties need no filling stays the interned type it is.
    Object(Box<[(Name, Held)]>),
    /// One of several, and nothing here knows which.
    ///
    /// What is left of a union once a guard has ruled some of it out: `Foo|Bar|string` past
    /// `if (is_string($x)) { return; }` is a `Foo` or a `Bar`. Built rather than pointed at, which is
    /// the whole reason this arm exists -- a written type lives in another file's arena and an
    /// interned one needs the table open for writing, so a union with one operand removed can be
    /// expressed no other way.
    ///
    /// Never one operand: a single survivor is that operand, and a degenerate wrapper would make two
    /// spellings of one type compare unequal.
    OneOf(Box<[Held]>),
    /// All of several at once -- an intersection, and the dual of [`Self::OneOf`].
    ///
    /// What a guard leaves when the declaration named something the guard's class has no relation to:
    /// `assert($mock instanceof Payment)` on a `MockInterface` does not replace the interface, because
    /// the value answers to both. That is what a test double *is*, and it is how every one of them in
    /// a real project is written.
    ///
    /// Built rather than pointed at, for the reason [`Self::OneOf`] is: an intersection assembled from
    /// a declaration and a guard exists in no type table. Never one operand, for the same reason too.
    AllOf(Box<[Held]>),
    /// An instance of a class, with what it was given for that class's `@template` parameters.
    ///
    /// What `new Collection(new Item())` produces: the class says `@template T` and its constructor
    /// says `@param T $item`, so the argument at the `new` is what `T` is for *this* value and for
    /// nothing else of the same class.
    ///
    /// Its own arm because the type table cannot hold it. An interned `Collection<Item>` would need
    /// the table open for writing, and the argument may not even be internable -- a class written in
    /// the file being read is a reference into that file's arena.
    ///
    /// Never with no arguments: a class with nothing supplied is a [`Self::Class`], and a wrapper
    /// around one would make two spellings of one value compare unequal.
    Applied {
        class: Name,
        arguments: Box<[Held]>,
    },
    /// A callable, and what invoking it gives back.
    ///
    /// Behind the box it already had, so every site that matches this arm without reading it is
    /// untouched: PHP reaches nothing through a callable but its result, and every question but
    /// [`phrost_body::PathStep::Invoked`] answers here exactly as it does for a bare `callable` --
    /// no class, no element, never null.
    Callable(Box<Callable>),
    /// A value that is one of those, or null. What a `$c ? f() : null` holds.
    ///
    /// A wrapper rather than a flag, because the null is the only thing it adds: everything asked of
    /// the inner type is asked of this one, and only the question "can null arrive here" gets a
    /// different answer. There is nowhere to put a rebuilt `?T` otherwise -- a written type lives in
    /// another file's arena and an interned one needs the table open for writing, and neither is
    /// available while reading.
    Nullable(Box<Held>),
    Stored {
        ty: TypeId,
        /// `None` for a value that came from somewhere with no enclosing class -- a free function's
        /// return. A relative type cannot be written there, so there is nothing to resolve it
        /// against, and saying so beats inventing a class for it.
        scope: Option<RelativeScope>,
    },
}

impl Held {
    /// Whether this is exactly PHP's null, which is the one operand a union folds out of itself.
    fn is_null(&self) -> bool {
        matches!(self, Self::Builtin(BuiltinTy::Null))
    }
}

/// A callable value: what invoking it gives back, and the signature it was written with.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Callable {
    /// What invoking it gives back.
    ///
    /// A resolved [`Held`] rather than a written or interned type, because the three sources
    /// disagree about which of those they have: `fn (): Row` writes one into this file's arena, an
    /// arrow function with no declaration says it only through the value its body holds, and a
    /// docblock signature would say it in a third place again. What they agree on is the answer.
    pub(crate) returns: Held,
    /// The closure literal this value is, when it is one -- the only source that carries a parameter
    /// list to show. `None` for a callable reached any other way, which renders the same as a
    /// closure declaring no parameters, because the two say a reader the same thing.
    ///
    /// An id into the file the value was read in, exactly like [`Held::Written`], and read for one
    /// thing: rendering the signature. Nothing resolves through it, so a chain that crosses into
    /// another file loses the parameters rather than reading the wrong ones.
    pub(crate) literal: Option<ExprId>,
}

/// What `self`, `static`, and `parent` mean for one stored type.
///
/// Two classes, not one, because PHP's two are genuinely different: `static` is the class the call
/// was made on, and `self` is the class that wrote the declaration. `Base::create(): self` called as
/// `Child::create()` returns a `Base`, and returning `Child` there would be a confident wrong
/// answer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct RelativeScope {
    /// The class the member was reached through — late static binding's `static`.
    pub(crate) called: Name,
    /// The class whose declaration wrote the type — `self`.
    pub(crate) declaring: Name,
}

impl IndexView<'_> {
    /// The closure literal a value *is*, when it is one written in the file being read.
    ///
    /// The one thing a call through a value can be given a parameter list from: a callable reached
    /// any other way -- a `callable` parameter, a string name, a first-class callable across files
    /// -- carries none, and answering nothing there is what keeps a wrong list out of a report.
    pub(crate) fn closure_literal_of(&self, held: &Held) -> Option<ExprId> {
        match held {
            Held::Callable(callable) => callable.literal,
            Held::Nullable(inner) => self.closure_literal_of(inner),
            _ => None,
        }
    }

    /// A union of several, or the one operand when there is only one.
    ///
    /// Never a one-operand wrapper: a single survivor *is* that operand, and wrapping it would make
    /// two spellings of one type compare unequal.
    ///
    /// Null is folded out to a [`Held::Nullable`] for the same reason. `A|null` is what that shape
    /// means, and everything that reads a null -- the guards, the null diagnostics,
    /// [`without_null`](IndexView::without_null) -- reads it there; leaving a null operand inside a
    /// union would be a second spelling of one type that all of them would have to learn.
    ///
    /// A `mixed` operand absorbs the union, which is the fifth: `Row|mixed` says both more and less
    /// than `mixed` does, and a reader taking it apart has to decide which half to believe.
    ///
    /// Every fold here is by *shape*, and a caller that needs the three spellings of a builtin
    /// resolves them before calling: nothing here has the file or the index to ask with. So this
    /// catches the `mixed` a cast produced and not the `mixed` a parameter declared -- see
    /// `list_literal_element`, which asks `kind_of_held` first for exactly that reason.
    ///
    /// A repeat is dropped, which is the fourth. A union built from a *list* -- the arguments filling
    /// one variadic parameter, the arms of a join -- produces one naturally: `max(1, 2, 3)` bound the
    /// same template three times and rendered `int|int`. The interned constructor has dropped repeats
    /// since it existed; this one could not compare its operands until [`Held`] derived equality.
    ///
    /// A nested union is flattened for the third time on the same grounds, and it is the one of the
    /// three that stops an answer being reached rather than merely spelling it oddly. Every guard
    /// works by taking a union apart and keeping the operands that survive, and an operand that is
    /// itself a union answers nothing about its own class or kind -- so a positive test drops it
    /// without looking inside and a negative one keeps it whole. One level suffices: this is the
    /// only place a [`Held::OneOf`] is built, so its operands are already flat.
    ///
    /// And `never` drops out of a union with anything else in it, because it is the type of a
    /// value that does not arrive: `$row ?? throw new NotFound()` is the row, not `Row|never`.
    pub(crate) fn one_of(operands: Vec<Held>) -> Held {
        let mut operands = operands
            .into_iter()
            .flat_map(|held| match held {
                Held::OneOf(nested) => nested.into_vec(),
                // A nullable operand is its null and the rest, and the null belongs to the union:
                // `string|?int` is the degenerate form no guard can narrow, because a test takes the
                // union apart and an operand that is itself nullable answers nothing about its kind.
                Held::Nullable(inner) => match *inner {
                    Held::OneOf(nested) => nested
                        .into_vec()
                        .into_iter()
                        .chain([Held::Builtin(BuiltinTy::Null)])
                        .collect(),
                    inner => vec![inner, Held::Builtin(BuiltinTy::Null)],
                },
                held => vec![held],
            })
            .collect::<Vec<_>>();
        let never = |held: &Held| matches!(held, Held::Builtin(BuiltinTy::Never));
        if operands.iter().any(|held| !never(held)) {
            operands.retain(|held| !never(held));
        }

        if !operands.iter().any(Held::is_null) {
            return Self::union(operands);
        }
        let rest = operands
            .into_iter()
            .filter(|held| !held.is_null())
            .collect::<Vec<_>>();
        match rest.is_empty() {
            // Nothing but null, which is a type of its own rather than a nullable anything.
            true => Held::Builtin(BuiltinTy::Null),
            false => Held::Nullable(Box::new(Self::union(rest))),
        }
    }

    /// A nullable, with the forms that mean nothing folded out.
    ///
    /// The constructor [`Self::one_of`] and [`Self::all_of`] both have and this concept did not: nine
    /// places built a `Held::Nullable` by hand, so there was nowhere for a rule about the degenerate
    /// forms to live and both of them showed up in a hover card. `??array<string, int>` is one --
    /// a nullable wrapped in a nullable, which no reader can act on and which is not a type PHP can
    /// spell. This is the same lesson §8 records for the two union constructors, arriving at a
    /// concept that had *no* constructor at all.
    ///
    /// Only what a [`Held`] can answer about itself. The *other* spelling of an inner nullable is a
    /// [`Held::Stored`] holding an interned `?array`, which needs the type table and so cannot be
    /// seen from here -- and it does not need to be: every reader strips a nullable by recursing, so
    /// a doubled one behaves exactly like a single one and the only place it shows is a rendered
    /// type. `IndexView::render_held` handles it there, where the question is about the text.
    ///
    /// Four forms fold. A nullable nullable is one nullable. A nullable `null` is `null`, which is a
    /// type of its own rather than a nullable anything -- the same answer [`Self::one_of`] gives when
    /// every operand was null. A nullable `mixed` is `mixed`, because `mixed` already admits null and
    /// saying it twice makes a reader choose which half to believe. And a nullable union is built
    /// through [`Self::one_of`] with a null operand added, so the union's own folding runs and the
    /// result is one nullable around a flat union rather than two levels.
    pub(crate) fn nullable(inner: Held) -> Held {
        match inner {
            Held::Nullable(_) => inner,
            Held::Builtin(BuiltinTy::Null) => Held::Builtin(BuiltinTy::Null),
            Held::Builtin(BuiltinTy::Mixed) => Held::Builtin(BuiltinTy::Mixed),
            // Through the union constructor, so a union that already carried a null does not gain a
            // second one and a nested union is flattened on the way.
            Held::OneOf(_) => Self::one_of(vec![inner, Held::Builtin(BuiltinTy::Null)]),
            inner => Held::Nullable(Box::new(inner)),
        }
    }

    fn union(mut operands: Vec<Held>) -> Held {
        // `T|mixed` is `mixed`, which is the fifth degenerate form and the one that reads worst:
        // `Row|mixed` says both more and less than `mixed`, and every reader of the union has to
        // decide which half to believe. First, because absorbing makes the rest of the folding moot.
        if operands
            .iter()
            .any(|held| matches!(held, Held::Builtin(BuiltinTy::Mixed)))
        {
            return Held::Builtin(BuiltinTy::Mixed);
        }
        // Before the boolean fold, so `true|true` reduces to one `true` and does not read as the two
        // literals together.
        Self::drop_repeats(&mut operands);
        Self::fold_bool(&mut operands);
        Self::fold_strings(&mut operands);
        match operands.len() {
            1 => operands.remove(0),
            _ => Held::OneOf(operands.into_boxed_slice()),
        }
    }

    /// The same operand twice is the same operand.
    ///
    /// Kept as its own step rather than folded into the flattening above, because the flattening is
    /// about *shape* and this is about identity: a nested union has to be spliced before its operands
    /// can be compared against the ones beside it.
    fn drop_repeats(operands: &mut Vec<Held>) {
        let mut kept: Vec<Held> = Vec::with_capacity(operands.len());
        for held in operands.drain(..) {
            if !kept.contains(&held) {
                kept.push(held);
            }
        }
        *operands = kept;
    }

    /// `true|false` is `bool`, and so is `bool|true`.
    ///
    /// The fourth degenerate form, and the only one PHP cannot spell at all -- which is the tell,
    /// the same one `string|?int` gave. `$flag ? true : false` is the ordinary way to write a
    /// boolean and it reached hover as `true|false`.
    ///
    /// Unlike the other three this one is *only* the spelling: measured before changing it, the
    /// argument checks accept either literal against a `bool` and `if ($x)` narrows both, so nothing
    /// downstream was answering wrongly. It is still worth folding, because a type nobody can write
    /// is a type nobody can compare against what they wrote.
    ///
    /// The first literal becomes the `bool` rather than one being appended, so an operand's position
    /// in a union is still the position the source wrote it in.
    fn fold_bool(operands: &mut Vec<Held>) {
        let mut has_true = false;
        let mut has_false = false;
        let mut has_bool = false;
        for held in operands.iter() {
            match held {
                Held::Builtin(BuiltinTy::True) => has_true = true,
                Held::Builtin(BuiltinTy::False) => has_false = true,
                Held::Builtin(BuiltinTy::Bool) => has_bool = true,
                _ => {}
            }
        }
        // Either the two literals are together, or a `bool` is already there and absorbs whichever
        // literal came with it.
        if !((has_true && has_false) || (has_bool && (has_true || has_false))) {
            return;
        }

        let mut stood_in = has_bool;
        operands.retain_mut(|held| match held {
            Held::Builtin(BuiltinTy::True | BuiltinTy::False) if stood_in => false,
            Held::Builtin(BuiltinTy::True | BuiltinTy::False) => {
                stood_in = true;
                *held = Held::Builtin(BuiltinTy::Bool);
                true
            }
            _ => true,
        });
    }

    /// `non-empty-string|string` is `string`: two strings are one, holding what both are known to.
    ///
    /// The other refinement fold, beside the boolean one and for the same reason; the interned
    /// constructor folds the same pair. A join of a `''` written in one branch with a `'name'`
    /// written in the other is the shape -- what comes out may be empty, and that is all it says.
    /// In place of the first string, as [`Self::fold_bool`] keeps an operand where it was written.
    fn fold_strings(operands: &mut Vec<Held>) {
        let mut joined: Option<BuiltinTy> = None;
        let mut strings = 0;
        for held in operands.iter() {
            if let Held::Builtin(kind) = held
                && kind.is_string()
            {
                strings += 1;
                joined = Some(joined.map_or(*kind, |seen| seen.string_join(*kind).unwrap_or(seen)));
            }
        }
        let Some(joined) = joined.filter(|_| strings > 1) else {
            return;
        };
        let mut stood_in = false;
        operands.retain_mut(|held| match held {
            Held::Builtin(kind) if kind.is_string() && stood_in => false,
            Held::Builtin(kind) if kind.is_string() => {
                stood_in = true;
                *held = Held::Builtin(joined);
                true
            }
            _ => true,
        });
    }

    /// All of several at once, or the one operand when there is only one. The dual of
    /// [`Self::one_of`], and degenerate for the same reason.
    pub(crate) fn all_of(mut operands: Vec<Held>) -> Held {
        match operands.len() {
            1 => operands.remove(0),
            _ => Held::AllOf(operands.into_boxed_slice()),
        }
    }
}
