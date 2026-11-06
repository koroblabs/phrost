//! A value used in a way no value of its kind can be.
//!
//! Two of PHP's `Error`s, both certain once the value's kind is known: `Value of type int is not
//! callable` and `clone(): Argument #1 ($object) must be of type object`. Each stops the line, so
//! like [`non_object_receiver`](crate::MemberAccessOutcome) this is proof rather than an absence of
//! knowledge.
//!
//! # Which kinds, which was run and not reasoned
//!
//! Asked of PHP 8.5.10, and two of the answers are why this cannot be "not an object":
//!
//! * a **string** is callable -- `$f = 'strlen'; $f('ab')` is how PHP has always written a callback;
//! * an **array** is callable too -- `[$object, 'method']` is the other spelling.
//!
//! So only `int`, `float`, `bool` and `null` are certainly not callable. `clone` is stricter and
//! takes an object alone, so every scalar, `array` and `null` is refused there.
//!
//! # An object of a class nothing can extend
//!
//! Three more `Error`s, and each is about what a class declares rather than what kind a value is:
//! `Object of type Foo is not callable` (no `__invoke`), `Cannot use object of type Foo as array`
//! (no `ArrayAccess` -- for a read, a write, an append, `isset` and `??` alike, all run against
//! 8.5.10), and `Object of class Foo could not be converted to string` (no `__toString`, in a
//! concatenation or an interpolated string). A class lacking one today is one a subclass may still
//! supply, so only a `final` class or an enum written in PHP is settled -- see
//! `resolve::object_kinds`, whose answer the compatibility judgement shares.
//!
//! A third shape joins them for the same reason: `$x::CONST`, `$x::method()` and `new $x` each need
//! a class, and PHP takes either an object *or the class's name as a string* -- `Class name must be
//! a valid object or a string` is the error, and it names the two that work.
//!
//! And a computed member name, which is two rules and not one. A property's name is *converted*, so
//! `$x->{$name}` fails only where `$name` is an object with no string form -- the `__toString` rule
//! again. A method's name must *be* a string: `$x->{$name}()` raises `Method name must be a string`
//! for an `int`, and for a `Stringable` object too.

use phrost_hir::{FileHir, body::ExprData};
use phrost_model::{BuiltinTy, TextRange};

use crate::view::{FOLLOW_LIMIT, IndexView};

/// One value used in a way its kind does not allow.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImpossibleUse {
    /// The value itself, which is what a report underlines.
    pub range: TextRange,
    pub kind: BuiltinTy,
    pub used: UsedAs,
    /// The class, when the value is an object refused for what its class does not declare. `kind`
    /// is `object` then.
    pub class: Option<phrost_model::Name>,
}

/// What was done with it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UsedAs {
    /// `$x()`, which needs a callable.
    Invoked,
    /// `clone $x`, which needs an object.
    Cloned,
    /// `$x::CONST`, `$x::method()`, `new $x` -- each needs a class, which a value gives by being an
    /// object or by being the class's *name*.
    AsAClassName,
    /// `$x[...]`, which on an object needs an `ArrayAccess`. Scalars are `non-array-subscript`'s.
    Subscripted,
    /// An operand of `.`, a hole in an interpolated string, or a computed property name -- each of
    /// which on an object needs a `__toString`. Only objects: a scalar converts, and an array is a
    /// warning rather than an error.
    AsAString,
    /// A computed method name, `$x->{$name}()`, which has to be a string and is converted from
    /// nothing -- not even from an object that declares `__toString`.
    AsAMethodName,
}

impl IndexView<'_> {
    /// Every invocation and `clone` in the current file whose value cannot take it.
    pub fn impossible_uses(&self) -> Vec<ImpossibleUse> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut out = Vec::new();

        phrost_body::visit_every_expression(hir, &mut |hir, expr| {
            // Beside whatever else the expression asks of a value: `$class::{$name}()` needs a
            // class of the one and a string of the other.
            if let ExprData::MethodCall {
                selector: Some(selector),
                ..
            }
            | ExprData::StaticCall {
                selector: Some(selector),
                ..
            }
            | ExprData::CallableMember {
                selector: Some(selector),
                ..
            } = hir.expr(expr)
            {
                self.refused(hir, *selector, UsedAs::AsAMethodName, &mut out);
            }
            let (subject, used) = match hir.expr(expr) {
                ExprData::IndirectCall { callee, .. } => (*callee, UsedAs::Invoked),
                ExprData::Cloned { value, .. } => (*value, UsedAs::Cloned),
                ExprData::StaticCall { class, .. }
                | ExprData::StaticPropertyAccess { class, .. }
                | ExprData::ClassConstAccess { class, .. }
                | ExprData::New { class, .. } => match class {
                    phrost_hir::body::ClassTarget::Expr(id) => (*id, UsedAs::AsAClassName),
                    _ => return,
                },
                ExprData::Index { base, .. } => {
                    self.refused_object(hir, *base, UsedAs::Subscripted, &mut out);
                    return;
                }
                ExprData::PropertyAccess {
                    selector: Some(selector),
                    ..
                } => {
                    self.refused_object(hir, *selector, UsedAs::AsAString, &mut out);
                    return;
                }
                ExprData::Operator {
                    produces, inner, ..
                } if produces.is_string() => {
                    for &operand in hir.expr_list(*inner) {
                        self.refused_object(hir, operand, UsedAs::AsAString, &mut out);
                    }
                    return;
                }
                _ => return,
            };
            self.refused(hir, subject, used, &mut out);
        });

        out.sort_by_key(|one| one.range.start());
        out
    }

    /// A value its kind refuses for `used`, or -- for the uses an object can fail by being one --
    /// whose class does.
    fn refused(
        &self,
        hir: &FileHir,
        subject: phrost_hir::ExprId,
        used: UsedAs,
        out: &mut Vec<ImpossibleUse>,
    ) {
        if matches!(used, UsedAs::Invoked | UsedAs::AsAMethodName) {
            self.refused_object(hir, subject, used, out);
        }
        let Some(kind) = self.kind_of(hir, subject) else {
            return;
        };
        if refuses(used, kind) {
            out.push(ImpossibleUse {
                range: hir.expr(subject).range(),
                kind,
                used,
                class: None,
            });
        }
    }

    /// A local holding an object of one class, refused for what that class cannot declare.
    ///
    /// A plain local only, for the reason [`Self::kind_of`] gives, and a value that cannot be null:
    /// a `?Foo` invoked is the null check's first, and one finding per mistake.
    fn refused_object(
        &self,
        hir: &FileHir,
        subject: phrost_hir::ExprId,
        used: UsedAs,
        out: &mut Vec<ImpossibleUse>,
    ) {
        if !matches!(hir.expr(subject), ExprData::Variable { .. }) {
            return;
        }
        let at = hir.expr(subject).range().start();
        let Some(path) = phrost_body::value_path_of(hir, subject) else {
            return;
        };
        let Some(held) = self.resolve_path(at, hir, &path, FOLLOW_LIMIT) else {
            return;
        };
        if self.null_reach_of(hir, &held) != crate::references::SentinelReach::Impossible {
            return;
        }
        let Some(class) = self.class_of_held(at, hir, &held) else {
            return;
        };
        let answers = match used {
            UsedAs::Invoked => self.class_answers_kind(class.as_str(), BuiltinTy::Callable),
            UsedAs::AsAString => self.class_answers_kind(class.as_str(), BuiltinTy::String),
            UsedAs::Subscripted => self.class_takes_a_subscript(class.as_str()),
            // No object names a method, whatever it declares.
            UsedAs::AsAMethodName => crate::resolve::Answers::No,
            UsedAs::Cloned | UsedAs::AsAClassName => return,
        };
        if answers == crate::resolve::Answers::No {
            out.push(ImpossibleUse {
                range: hir.expr(subject).range(),
                kind: BuiltinTy::Object,
                used,
                class: Some(class),
            });
        }
    }

    /// The builtin kind an expression certainly holds, if it certainly holds one.
    ///
    /// Only a plain local variable, which is the same line `SubscriptSite::base_is_local` draws and
    /// for the same reason: a kind read off an **untyped property** is a starting value rather than
    /// a claim. Drupal core writes `new $this->containerClass(...)` beside `$this->machineFormat =
    /// TRUE;` and a table missing the parent class answers `bool` for the first -- eight reports,
    /// none of them real. A parameter and a local are what the declaration actually states.
    fn kind_of(&self, hir: &FileHir, expr: phrost_hir::ExprId) -> Option<BuiltinTy> {
        if !matches!(hir.expr(expr), ExprData::Variable { .. }) {
            return None;
        }
        let path = phrost_body::value_path_of(hir, expr)?;
        let held = self.resolve_path(hir.expr(expr).range().start(), hir, &path, FOLLOW_LIMIT)?;
        self.scalar_named(hir, &held)
    }
}

/// Whether a value of this kind certainly cannot be used this way.
///
/// Each list names its kinds rather than negating a short one, for the reason `holds_no_elements`
/// gives about the same vocabulary: `BuiltinTy` is closed by convention and not by the type system,
/// so a variant added later must not fall into a *refused* list by default and turn every use of it
/// into a report.
///
/// What is deliberately in none of them:
///
/// * `mixed`, which is every type;
/// * `callable`, which is a string, an object or an array depending on the value -- Symfony reached
///   `$callable::class` three times and each one runs;
/// * `iterable`, which is an array or a `Traversable` object;
/// * `void` and `never`, whose appearance means something already went wrong further up, and a
///   second report would be about the first one's cause.
fn refuses(used: UsedAs, kind: BuiltinTy) -> bool {
    match used {
        // Objects only, asked of the class in `refused_object`.
        UsedAs::Subscripted | UsedAs::AsAString => false,
        // A string is callable -- `$f = 'strlen'; $f('ab')` -- and so is an array, because
        // `[$object, 'method']` is the other spelling PHP has always taken.
        UsedAs::Invoked => matches!(
            kind,
            BuiltinTy::Int
                | BuiltinTy::Float
                | BuiltinTy::Bool
                | BuiltinTy::True
                | BuiltinTy::False
                | BuiltinTy::Null
        ),
        UsedAs::Cloned => matches!(
            kind,
            BuiltinTy::Int
                | BuiltinTy::Float
                | BuiltinTy::String
                | BuiltinTy::Bool
                | BuiltinTy::True
                | BuiltinTy::False
                | BuiltinTy::Array
                | BuiltinTy::Null
        ),
        // Objects are asked in `refused_object`, which refuses every one.
        UsedAs::AsAMethodName => matches!(
            kind,
            BuiltinTy::Int
                | BuiltinTy::Float
                | BuiltinTy::Bool
                | BuiltinTy::True
                | BuiltinTy::False
                | BuiltinTy::Array
                | BuiltinTy::Null
        ),
        // A *string* names a class -- `$c = 'DateTime'; $c::createFromFormat(...)` runs -- and so
        // does an object. What is left is `Class name must be a valid object or a string`.
        UsedAs::AsAClassName => matches!(
            kind,
            BuiltinTy::Int
                | BuiltinTy::Float
                | BuiltinTy::Bool
                | BuiltinTy::True
                | BuiltinTy::False
                | BuiltinTy::Array
                | BuiltinTy::Null
        ),
    }
}
