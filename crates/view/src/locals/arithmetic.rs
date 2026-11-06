//! What an arithmetic operator produces, read off what each operand can be.
//!
//! PHP settles `+`, `-`, `*`, `/` and `**` by their operands, and an operand is rarely one kind:
//! `strrpos($s, '|') + 1` adds an `int|false`, `$offset + $page` a `?int`. Asked of one kind
//! at a time, every one of those answered nothing, and a computed number in real code is more often
//! a union than not. So each operand is read as the set of kinds it may be, and the result is what
//! every pair of them makes.
//!
//! Two classes take the operators as well, and they are listed rather than inferred: see
//! [`OVERLOADING`].

use phrost_hir::{FileHir, body::ArithmeticOp};
use phrost_model::{BuiltinTy, Name, TextSize};

use crate::{SentinelReach, resolve::Held, view::IndexView};

/// The classes whose objects the arithmetic operators take, each handing back an object of itself.
///
/// Two extensions overload the operators on a class of their own: GMP, and BCMath's `Number` since
/// PHP 8.4. Asked of PHP 8.5: `Number * 2`, `2 * Number`, `Number + 1.5`, `Number - '4'` and
/// `Number * true` are each a `BcMath\Number`, and `GMP + GMP`, `GMP * 3` and `-GMP` a `GMP`.
///
/// Closed, and by name. An object of any other class is a `TypeError` under these operators, so a
/// rule reading "an object operand" would answer for every one of them; a third extension that
/// overloads them is a row here, not a guess.
const OVERLOADING: [&str; 2] = ["BcMath\\Number", "GMP"];

/// The kinds one operand of an arithmetic operator may be, as far as the result cares.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(super) struct Operand {
    /// An int, or a value every operator reads as one: `null` and `false` are `0`, `true` is `1`.
    /// Asked of PHP 8.5: `null + 1` is `int(1)`, `true * 1.5` is `float(1.5)`.
    int: bool,
    float: bool,
    /// A string, which is whichever number it spells -- `"5" + 1` is an int and `"5.5" + 1` a float
    /// -- or a `TypeError` that hands back no value at all.
    string: bool,
    array: bool,
    /// Null, which [`Self::int`] already counts for the scalar rules. Apart as well because a class
    /// that overloads the operators does not read it as `0`: `Number + null` and `null * GMP` are a
    /// `TypeError`.
    null: bool,
    /// Which of [`OVERLOADING`] it may be an object of, by position.
    objects: [bool; OVERLOADING.len()],
}

impl Operand {
    const ARRAY: Self = Self {
        int: false,
        float: false,
        string: false,
        array: true,
        null: false,
        objects: [false; OVERLOADING.len()],
    };

    /// One kind, or `None` for a kind arithmetic has no answer for: an object of a class outside
    /// [`OVERLOADING`] is a `TypeError`, and anything else is either unknown or one too.
    fn of_kind(kind: BuiltinTy) -> Option<Self> {
        let mut operand = Self::default();
        match kind {
            BuiltinTy::Null => {
                operand.int = true;
                operand.null = true;
            }
            BuiltinTy::Int | BuiltinTy::Bool | BuiltinTy::True | BuiltinTy::False => {
                operand.int = true;
            }
            BuiltinTy::Float => operand.float = true,
            BuiltinTy::String | BuiltinTy::NonEmptyString | BuiltinTy::NonFalsyString => {
                operand.string = true;
            }
            BuiltinTy::Array => operand.array = true,
            BuiltinTy::Object
            | BuiltinTy::Callable
            | BuiltinTy::Iterable
            | BuiltinTy::Mixed
            | BuiltinTy::Void
            | BuiltinTy::Never
            | BuiltinTy::Resource => return None,
        }
        Some(operand)
    }

    /// An object of a class, which only one of [`OVERLOADING`] can be.
    fn of_class(class: &str) -> Option<Self> {
        let class = class.trim_start_matches('\\');
        let at = OVERLOADING
            .iter()
            .position(|overloading| overloading.eq_ignore_ascii_case(class))?;
        let mut operand = Self::default();
        operand.objects[at] = true;
        Some(operand)
    }

    fn or(self, other: Self) -> Self {
        let mut objects = self.objects;
        for (mine, theirs) in objects.iter_mut().zip(other.objects) {
            *mine |= theirs;
        }
        Self {
            int: self.int || other.int,
            float: self.float || other.float,
            string: self.string || other.string,
            array: self.array || other.array,
            null: self.null || other.null,
            objects,
        }
    }

    fn may_be_an_object(self) -> bool {
        self.objects.contains(&true)
    }

    fn may_be_a_scalar(self) -> bool {
        self.int || self.float || self.string || self.array
    }

    /// Whether every value the other side may be is one this side's objects take.
    ///
    /// A number, a numeric string, a bool, or an object of the same class. Not a null and not an
    /// array, which neither class reads as a number, and not the *other* class: `Number + GMP` is a
    /// `TypeError` both ways round.
    fn objects_take(self, other: Self) -> bool {
        if !self.may_be_an_object() {
            return true;
        }
        let another_class = self.objects.iter().enumerate().any(|(mine, &is)| {
            is && other
                .objects
                .iter()
                .enumerate()
                .any(|(theirs, &also)| also && theirs != mine)
        });
        !other.null && !other.array && !another_class
    }

    /// What `self <operator> right` hands back, over every pair of kinds the two may be.
    ///
    /// A float as soon as either side is one. Two ints make an int except under `/`, where PHP
    /// hands back an int only when the division comes out even -- `int|float` is the whole answer,
    /// and picking either half would be a confident wrong one. A string is whichever number it
    /// spells, so it makes either.
    ///
    /// An array only for `+` given two arrays and nothing else -- the one spelling where the
    /// operator does not mean arithmetic at all. An array beside any other kind is refused rather
    /// than answered by its other half: `array|int + 1` is a `TypeError` on one path, and saying
    /// `int` about it would read as the code being fine.
    fn produces(self, operator: ArithmeticOp, right: Self) -> Option<Held> {
        if self.array || right.array {
            return (operator == ArithmeticOp::Add && self == Self::ARRAY && right == Self::ARRAY)
                .then_some(Held::Builtin(BuiltinTy::Array));
        }
        let float = self.float || right.float;
        let int = (self.int || self.string) && (right.int || right.string);
        let int_or_float = self.string || right.string || (int && operator == ArithmeticOp::Divide);
        let made = match (int, float || int_or_float) {
            (true, true) => {
                return Some(IndexView::one_of(vec![
                    Held::Builtin(BuiltinTy::Int),
                    Held::Builtin(BuiltinTy::Float),
                ]));
            }
            (true, false) => BuiltinTy::Int,
            (false, true) => BuiltinTy::Float,
            (false, false) => return None,
        };
        Some(Held::Builtin(made))
    }

    /// The same operand with its objects taken off, for the pairs that are scalars on both sides.
    fn scalars(self) -> Self {
        Self {
            objects: [false; OVERLOADING.len()],
            ..self
        }
    }
}

impl IndexView<'_> {
    /// What `left <operator> right` produces, where both have been resolved.
    pub(super) fn arithmetic_result(
        &self,
        offset: TextSize,
        hir: &FileHir,
        operator: ArithmeticOp,
        left: &Held,
        right: &Held,
    ) -> Option<Held> {
        let left = self.arithmetic_operand(offset, hir, left)?;
        let right = self.arithmetic_operand(offset, hir, right)?;
        if !left.may_be_an_object() && !right.may_be_an_object() {
            return left.produces(operator, right);
        }
        self.overloaded_result(operator, left, right)
    }

    /// What an operator hands back where either side may be an object of a class that overloads it.
    ///
    /// The union over every pair, as for the scalars: an object of each class either side may be,
    /// since a pair with one in it makes one, and what the scalars make between themselves where
    /// both sides may be one. So `?Number * 2` is a `Number` or the `0` a null makes, and
    /// `Number|int + 1` a `Number` or an `int`.
    ///
    /// Refused whole when any pair is a `TypeError`, for the reason an array beside a number is:
    /// the other pairs' answer would read as the code being fine.
    fn overloaded_result(
        &self,
        operator: ArithmeticOp,
        left: Operand,
        right: Operand,
    ) -> Option<Held> {
        if !left.objects_take(right) || !right.objects_take(left) {
            return None;
        }
        let mut made = Vec::new();
        for (at, class) in OVERLOADING.iter().enumerate() {
            if left.objects[at] || right.objects[at] {
                made.push(Held::Class(self.class_location(class)?.fqn));
            }
        }
        if left.may_be_a_scalar() && right.may_be_a_scalar() {
            made.push(left.scalars().produces(operator, right.scalars())?);
        }
        Some(IndexView::one_of(made))
    }

    /// What `++$n` or `--$n` leaves in a value that is an object of a class that overloads the
    /// operators: another of its class, as `$n + 1` would be. Asked of PHP 8.5 for both classes.
    ///
    /// Only a value that is nothing but such objects. A null steps by rules of its own -- `null`
    /// goes up to `1` and stays `null` going down -- so a `?Number` is left unanswered rather than
    /// answered for half its paths.
    pub(super) fn stepped_object(
        &self,
        offset: TextSize,
        hir: &FileHir,
        held: &Held,
    ) -> Option<Held> {
        let operand = self.arithmetic_operand(offset, hir, held)?;
        if operand.may_be_a_scalar() || operand.null {
            return None;
        }
        self.overloaded_result(
            ArithmeticOp::Add,
            operand,
            Operand::of_kind(BuiltinTy::Int)?,
        )
    }

    /// What `left & right`, `|` or `^` produces, where both have been resolved.
    ///
    /// PHP's own rule, which is simpler than the arithmetic one: two strings make a string, worked
    /// byte by byte, and any other pair is read as two ints. Asked of PHP 8.5: `'a' | 'b'` is
    /// `string(1) "c"`, `'5' | 1` is `int(5)`, `true | 2` is `int(3)`. An array is refused, as it is
    /// for arithmetic, and so is an object: GMP overloads these and `Number` does not, and a
    /// constant -- the only thing asking -- holds neither.
    pub(super) fn bitwise_result(
        &self,
        offset: TextSize,
        hir: &FileHir,
        left: &Held,
        right: &Held,
    ) -> Option<Held> {
        let left = self.arithmetic_operand(offset, hir, left)?;
        let right = self.arithmetic_operand(offset, hir, right)?;
        if left.array || right.array || left.may_be_an_object() || right.may_be_an_object() {
            return None;
        }
        let only_strings = |operand: Operand| !operand.int && !operand.float;
        let string = left.string && right.string;
        let int = !(only_strings(left) && only_strings(right));
        Some(match (int, string) {
            (true, true) => IndexView::one_of(vec![
                Held::Builtin(BuiltinTy::Int),
                Held::Builtin(BuiltinTy::String),
            ]),
            (true, false) => Held::Builtin(BuiltinTy::Int),
            (false, _) => Held::Builtin(BuiltinTy::String),
        })
    }

    /// The kinds a held value may be, every operand of a union read in turn.
    ///
    /// Through [`Self::union_operands`], which knows all three spellings of a union, and with the
    /// null asked for separately -- a `?int` lifts its null into a wrapper that the operands no
    /// longer show.
    fn arithmetic_operand(&self, offset: TextSize, hir: &FileHir, held: &Held) -> Option<Operand> {
        let mut operand = match self.union_operands(offset, hir, held) {
            Some(operands) => operands.iter().try_fold(Operand::default(), |sum, one| {
                Some(sum.or(Self::operand_of(one.kind?, || one.class.clone())?))
            })?,
            None => Self::operand_of(self.kind_of_held(hir, held)?, || {
                self.class_of_held(offset, hir, held)
            })?,
        };
        if self.null_reach_of(hir, held) != SentinelReach::Impossible {
            operand.int = true;
            operand.null = true;
        }
        Some(operand)
    }

    /// One kind, and for an object the class it is -- asked only then, since nothing else needs it.
    fn operand_of(kind: BuiltinTy, class: impl FnOnce() -> Option<Name>) -> Option<Operand> {
        match kind {
            BuiltinTy::Object => Operand::of_class(class()?.as_str()),
            kind => Operand::of_kind(kind),
        }
    }
}
