//! A declaration PHP refuses on its own, with nothing else in the project needed to see it.
//!
//! The rest of this file's neighbours are about a pair -- a child and its parent, two traits, two
//! files -- and need an index to see. These need nothing: the declaration is written out and PHP
//! will not compile it. They are worth having anyway, because the alternative to a red squiggle is
//! a white screen, and the alternative to *this* file answering is that each of them is written
//! once and never thought about again.
//!
//! Every rule here was run against PHP 8.5.10 rather than read off a manual, and several of the
//! answers are not what the wording suggests -- a variable is case-sensitive, so `$This` is an
//! ordinary parameter and only `$this` is refused; a **by-reference** promoted property is legal
//! and a variadic one is not; and `abstract private` is legal inside a trait.
//!
//! About a declaration:
//!
//! * `public readonly $x` -- `Readonly property C::$x must have type`.
//! * `public readonly int $x = 1` -- `Readonly property C::$x cannot have default value`.
//! * `public static readonly int $x` -- `Static property C::$x cannot be readonly`.
//! * `abstract public function m(): void {}` -- `Abstract function C::m() cannot contain body`.
//! * `abstract private function m(): void;` in a **class** -- `Abstract function C::m() cannot be
//!   declared private`. Legal in a trait, where the using class fulfils it with a private method of
//!   its own; Symfony writes that three times.
//! * `interface I { public function m(): void {} }` -- `Interface function I::m() cannot contain
//!   body`, which is the same rule: an interface's method is abstract whether the word is written
//!   or not.
//! * a property on an enum -- `Enum E cannot include properties`.
//! * `abstract final function m()`, and `final` on an interface's method -- `Cannot use the final
//!   modifier on an abstract method` and `Interface method I::m() must not be final`. One rule: an
//!   interface's method is abstract whether the word is written or not.
//! * a **backed** enum's case with no value, and a plain enum's case with one -- `Case A of backed
//!   enum E must have a value` and `Case A of non-backed enum E must not have a value`.
//! * a class constant called `class` -- `A class constant must not be called 'class'; it is
//!   reserved for class name fetching`. In any case, which is the one place a constant's name is
//!   compared case-insensitively: `const CLASS` is the same fatal.
//!
//! About a parameter list, which is checked wherever one is written -- a function, a method, a
//! closure, an arrow function -- because PHP refuses all four the same way:
//!
//! * two parameters of one name -- `Redefinition of parameter $a`. Exactly, because variables are
//!   case-sensitive and `f($a, $A)` compiles.
//! * `$this` as a parameter -- `Cannot use $this as parameter`.
//! * a variadic that is not the last -- `Only the last parameter can be variadic`.
//! * a variadic with a default -- `Variadic parameter cannot have a default value`.
//! * a **variadic** promoted property -- `Cannot declare variadic promoted property`. A
//!   by-reference one is legal and was run to check.
//! * promotion anywhere but a constructor, and in a constructor with no body -- `Cannot declare
//!   promoted property outside a constructor` and `Cannot declare promoted property`. The second is
//!   an abstract constructor or an interface's, which have nothing to assign in.
//!
//! And about a **native** type, wherever one is written -- never a docblock's, which PHP does not
//! read and which may say things the language would refuse:
//!
//! * `void`, `never` or `mixed` as an operand of a union or an intersection, or under a `?` --
//!   `Void can only be used as a standalone type`.
//! * `void` or `never` on a parameter or a property, where only a return may carry them --
//!   `void cannot be used as a parameter type`, `Property C::$x cannot have type void`.
//! * anything that is not a class in an intersection -- `Type int cannot be part of an intersection
//!   type`. `callable` and `null` are in it, and `self`, `static` and `parent` are not: they name a
//!   class.
//! * one type written twice in a union, case-insensitively, because type names are --
//!   `Duplicate type int is redundant`, and `int|INT` says the same. `iterable|array` is the same
//!   fatal and is **not** reported here: `iterable` means `array|Traversable`, and expanding it is
//!   a rule about what a name unfolds to rather than about what was written.
//! * `true` and `false` in one union -- `Type contains both true and false, bool must be used
//!   instead`.
//!
//! A member an `@method` or `@property` tag declares is skipped whole, for the reason the native
//! types above are: PHP reads no comments, so nothing a docblock writes is a fatal.
//!
//! Off the HIR rather than the skeleton, because every one is a fact about the *written*
//! declaration and this only ever runs on a file somebody has open. A body is the one that could
//! not come from anywhere else: the index keeps no statements for a closed file.

use phrost_hir::MemberKind;
use phrost_model::{Name, TextRange, Visibility};

use crate::view::IndexView;

/// One declaration PHP will not compile.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RefusedDeclaration {
    /// The member's own name, which is what a report underlines.
    pub range: TextRange,
    pub member: Name,
    pub because: Refused,
}

/// What PHP objects to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refused {
    /// `readonly` with no native type.
    ReadonlyWithoutType,
    /// `readonly` with a default value.
    ReadonlyWithDefault,
    /// `static readonly`, which PHP refuses outright.
    ReadonlyAndStatic,
    /// `abstract` with a body.
    AbstractWithBody,
    /// `abstract private`, which nothing below could implement.
    AbstractAndPrivate,
    /// A property on an enum, which holds cases and nothing else.
    PropertyOnEnum,
    /// `abstract final`, or `final` on an interface's method: two words that cancel out.
    AbstractAndFinal,
    /// A case of a backed enum with no value after it.
    CaseWithoutValue,
    /// A case of a plain enum with a value after it.
    CaseWithValue,
    /// Two parameters of one name.
    DuplicateParameter,
    /// `$this` written as a parameter.
    ThisAsParameter,
    /// A variadic parameter with something after it.
    VariadicNotLast,
    /// A variadic parameter with a default value.
    VariadicWithDefault,
    /// A variadic parameter that also promotes a property.
    VariadicPromotion,
    /// A promoted property outside a constructor.
    PromotedOutsideConstructor,
    /// A promoted property in a constructor with no body to assign in.
    PromotedWithoutABody,
    /// `void`, `never` or `mixed` written as an operand rather than on its own.
    StandaloneTypeOnly(phrost_model::BuiltinTy),
    /// `void` or `never` where only a return type may carry them.
    ReturnTypeOnly(phrost_model::BuiltinTy),
    /// A type that is not a class, written as an operand of an intersection.
    NotAClassInIntersection(phrost_model::BuiltinTy),
    /// One type written twice in a union.
    DuplicateInUnion,
    /// `true` and `false` in one union, which is what `bool` is for.
    BothTrueAndFalse,
    /// A class constant called `class`, which is the name-fetching syntax.
    ConstantCalledClass,
}

/// Where a native type was written, which decides two of the rules about it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TypeOf {
    /// A return type, the only place `void` and `never` may appear at all.
    Return,
    /// A parameter or a property, which may carry neither.
    Value,
}

/// Where a parameter list was written, which is what two of the rules about one turn on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ParamsOf {
    /// A `__construct` with a body, the only place a promoted property may be declared.
    Constructor,
    /// A `__construct` with nothing to assign in: abstract, or an interface's.
    BodilessConstructor,
    /// A function, a closure, or any other method.
    Anything,
}

impl IndexView<'_> {
    /// Every declaration in the current file PHP refuses on its own.
    pub fn refused_declarations(&self) -> Vec<RefusedDeclaration> {
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        let mut out = Vec::new();

        for (_, function) in hir.functions() {
            Self::refuse_params(
                hir,
                hir.param_list(function.params),
                ParamsOf::Anything,
                &mut out,
            );
            Self::refuse_type(
                hir,
                function.return_type,
                TypeOf::Return,
                &function.name,
                function.name_range,
                &mut out,
            );
        }
        // Every closure and arrow function in the file, which PHP refuses exactly as it refuses a
        // named one. Only where bodies were lowered: a skeleton-mode file holds no expressions, and
        // the walk would be over nothing.
        if hir.has_bodies() {
            phrost_body::visit_every_expression(hir, &mut |hir, expr| {
                if let phrost_hir::body::ExprData::Closure { params, .. } = hir.expr(expr) {
                    Self::refuse_params(hir, hir.param_list(*params), ParamsOf::Anything, &mut out);
                }
            });
        }

        // Collected rather than checked in place: the member walk holds a closure that borrows
        // `out`, and a parameter list is checked by a function that appends to it too.
        let mut params_of = Vec::new();
        let mut types_of = Vec::new();
        for (id, class) in hir.classes() {
            let is_enum = class.kind == phrost_hir::ClassKind::Enum;
            // An interface's method is abstract whether the word is written or not, which is what
            // makes `interface I { public function m(): void {} }` the fatal
            // `Interface function I::m() cannot contain body`.
            let is_interface = class.kind == phrost_hir::ClassKind::Interface;
            let is_trait = class.kind == phrost_hir::ClassKind::Trait;
            for &member_id in hir.member_list(class.members) {
                let member = hir.member(member_id);
                if member.owner != id {
                    continue;
                }
                // A member an `@method` or `@property` tag declares is a claim in a comment, and
                // PHP reads no comments: `@method static object|mixed make(...)` is Laravel's own
                // and is not the fatal `Type mixed can only be used as a standalone type`. Thirty
                // three of them on Laravel alone were the first thing these rules reported.
                if member.from_docblock {
                    continue;
                }
                let mut refuse = |because| {
                    out.push(RefusedDeclaration {
                        range: member.name_range,
                        member: member.name.clone(),
                        because,
                    });
                };

                match member.kind {
                    MemberKind::Property => {
                        // An enum holds cases, constants and methods. A property is the one thing
                        // it cannot have, because there is no instance to put it on.
                        if is_enum {
                            refuse(Refused::PropertyOnEnum);
                            continue;
                        }
                        types_of.push((
                            member.type_ref,
                            TypeOf::Value,
                            member.name.clone(),
                            member.name_range,
                        ));
                        if member.modifiers.is_readonly() {
                            if member.modifiers.is_static() {
                                refuse(Refused::ReadonlyAndStatic);
                            } else if member.type_ref.is_none() {
                                // The *native* hint, not a docblock: PHP reads one and not the
                                // other, and `@var int` above an untyped readonly is still the
                                // fatal.
                                refuse(Refused::ReadonlyWithoutType);
                            } else if member.has_default {
                                refuse(Refused::ReadonlyWithDefault);
                            }
                        }
                    }
                    MemberKind::Method => {
                        // Every method's parameters, abstract or not. A constructor with a body is
                        // the only place promotion is legal, and an interface's or an abstract one
                        // is the case PHP refuses with a different sentence.
                        params_of.push((
                            member.params,
                            match member.name.as_str().eq_ignore_ascii_case("__construct") {
                                false => ParamsOf::Anything,
                                true => match is_interface || member.modifiers.is_abstract() {
                                    true => ParamsOf::BodilessConstructor,
                                    false => ParamsOf::Constructor,
                                },
                            },
                        ));
                        types_of.push((
                            member.type_ref,
                            TypeOf::Return,
                            member.name.clone(),
                            member.name_range,
                        ));

                        if !member.modifiers.is_abstract() && !is_interface {
                            continue;
                        }
                        // `final` and `abstract` cancel out, and an interface's method is abstract
                        // without the word -- so `interface I { final function m(); }` is the same
                        // fatal wearing different words.
                        if member.modifiers.is_final() {
                            refuse(Refused::AbstractAndFinal);
                        }
                        // An interface's method is abstract without the word, and writing the word
                        // is legal there -- so only a body and a private one are refused, and the
                        // body half is what `interface I { function m() {} }` is.
                        // In a *class*. A trait's `abstract private` is legal and Symfony writes
                        // it three times: the using class fulfils it with a private method of its
                        // own, which is exactly what a trait's requirement is for. Run both ways
                        // before this line was narrowed.
                        if member.visibility == Visibility::Private && !is_trait {
                            refuse(Refused::AbstractAndPrivate);
                        }
                        // Only where bodies were lowered at all: a skeleton-mode file has `None`
                        // for every one of them, and reading that as "abstract" would report every
                        // method in the project.
                        if hir.has_bodies() && member.body.is_some() {
                            refuse(Refused::AbstractWithBody);
                        }
                    }
                    // A case is backed or it is not, and the enum's own header decides which:
                    // `enum E: string` requires every case to carry a value and a plain `enum E`
                    // refuses one. Both are fatals at compile time.
                    MemberKind::EnumCase => {
                        match (class.enum_backing.is_some(), member.has_default) {
                            (true, false) => refuse(Refused::CaseWithoutValue),
                            (false, true) => refuse(Refused::CaseWithValue),
                            _ => {}
                        }
                    }
                    // `Foo::class` is how PHP fetches a class's name, so the word is taken and a
                    // constant may not have it -- in any case, which is the one place a constant
                    // name is compared case-insensitively.
                    MemberKind::ClassConstant => {
                        if member.name.as_str().eq_ignore_ascii_case("class") {
                            refuse(Refused::ConstantCalledClass);
                        }
                    }
                }
            }
        }

        for (params, written_in) in params_of {
            Self::refuse_params(hir, hir.param_list(params), written_in, &mut out);
        }
        for (ty, written_as, name, range) in types_of {
            Self::refuse_type(hir, ty, written_as, &name, range, &mut out);
        }

        out.sort_by_key(|found| found.range.start());
        out.dedup();
        out
    }

    /// What PHP refuses about one parameter list, wherever it was written.
    ///
    /// Static because nothing here needs the index: every rule is about the parameters beside each
    /// other, which is why a closure's list can be checked by the same code as a function's.
    fn refuse_params(
        hir: &phrost_hir::FileHir,
        ids: &[phrost_hir::ParamId],
        written_in: ParamsOf,
        out: &mut Vec<RefusedDeclaration>,
    ) {
        let mut seen: Vec<&str> = Vec::with_capacity(ids.len());

        for (at, &id) in ids.iter().enumerate() {
            let param = hir.param(id);
            let mut refuse = |because| {
                out.push(RefusedDeclaration {
                    range: param.name_range,
                    member: param.name.clone(),
                    because,
                });
            };

            // Exactly, and not folded: PHP's variables are case-sensitive, so `f($a, $A)` compiles
            // and `f($a, $a)` does not.
            match seen.contains(&param.name.as_str()) {
                true => refuse(Refused::DuplicateParameter),
                false => seen.push(param.name.as_str()),
            }
            if param.name.as_str() == "this" {
                refuse(Refused::ThisAsParameter);
            }
            if param.is_variadic {
                if at + 1 != ids.len() {
                    refuse(Refused::VariadicNotLast);
                }
                if param.has_default {
                    refuse(Refused::VariadicWithDefault);
                }
                if param.promotion.is_some() {
                    refuse(Refused::VariadicPromotion);
                }
            }
            if param.promotion.is_some() {
                match written_in {
                    ParamsOf::Constructor => {}
                    ParamsOf::BodilessConstructor => refuse(Refused::PromotedWithoutABody),
                    ParamsOf::Anything => refuse(Refused::PromotedOutsideConstructor),
                }
            }
            Self::refuse_type(
                hir,
                param.type_ref,
                TypeOf::Value,
                &param.name,
                param.name_range,
                out,
            );
        }
    }

    /// What PHP refuses about one **native** type, and about every operand inside it.
    ///
    /// Native and never a docblock's, which is the whole safety of this: PHP does not read a
    /// docblock, so `@return int|int` and `@param void $x` are nothing to it, and reporting them
    /// would be reporting a language the runtime does not have. `type_ref` is the written hint and
    /// `doc_type_ref` is the other one -- only the first is passed in.
    ///
    /// The report lands on the declaration's own name, because a type reference carries no range:
    /// lowering keeps the shape of one and not where each operand was written.
    fn refuse_type(
        hir: &phrost_hir::FileHir,
        id: Option<phrost_hir::TypeRefId>,
        written_as: TypeOf,
        name: &Name,
        range: TextRange,
        out: &mut Vec<RefusedDeclaration>,
    ) {
        use phrost_hir::TypeRefData;
        use phrost_model::BuiltinTy;

        let Some(id) = id else {
            return;
        };
        let mut refuse = |because| {
            out.push(RefusedDeclaration {
                range,
                member: name.clone(),
                because,
            });
        };

        // A `void` or `never` standing alone is still refused on a parameter or a property, where
        // only a return may carry either. `mixed` is legal there, which is why this arm names two
        // and the operand rule below names three.
        if let TypeRefData::Builtin(builtin) = hir.type_ref(id)
            && written_as == TypeOf::Value
            && matches!(builtin, BuiltinTy::Void | BuiltinTy::Never)
        {
            refuse(Refused::ReturnTypeOnly(*builtin));
            return;
        }

        let (operands, intersecting) = match hir.type_ref(id) {
            TypeRefData::Union(parts) => (hir.type_ref_list(*parts).to_vec(), false),
            TypeRefData::Intersection(parts) => (hir.type_ref_list(*parts).to_vec(), true),
            // `?T` is a union with `null` written shorter, and PHP applies the standalone rule to
            // it the same way: `?void` and `int|void` are one message.
            TypeRefData::Nullable(inner) => (vec![*inner], false),
            _ => return,
        };

        let mut seen: Vec<String> = Vec::with_capacity(operands.len());
        let (mut has_true, mut has_false) = (false, false);
        for operand in operands {
            match hir.type_ref(operand) {
                TypeRefData::Builtin(builtin) => {
                    if matches!(
                        builtin,
                        BuiltinTy::Void | BuiltinTy::Never | BuiltinTy::Mixed
                    ) {
                        refuse(Refused::StandaloneTypeOnly(*builtin));
                        continue;
                    }
                    // An intersection is every operand at once, and only an object can be several
                    // things at once -- so `callable`, `null`, `array` and every scalar are out.
                    if intersecting {
                        refuse(Refused::NotAClassInIntersection(*builtin));
                        continue;
                    }
                    has_true |= *builtin == BuiltinTy::True;
                    has_false |= *builtin == BuiltinTy::False;
                }
                // `self`, `static` and `parent` name a class, so an intersection may hold them.
                TypeRefData::Relative(_) | TypeRefData::Named(_) => {}
                // Anything else in a native position is a spelling this walk did not expect, and
                // saying nothing about it is the direction that cannot be wrong.
                _ => continue,
            }
            // Case-insensitively, because PHP's type names are: `int|INT` is the same fatal.
            let written = match hir.type_ref(operand) {
                TypeRefData::Builtin(builtin) => builtin.as_str().to_owned(),
                TypeRefData::Named(name_ref) => {
                    hir.name_ref(*name_ref).path.as_str().to_ascii_lowercase()
                }
                TypeRefData::Relative(relative) => format!("{relative:?}"),
                _ => continue,
            };
            match seen.contains(&written) {
                true if !intersecting => refuse(Refused::DuplicateInUnion),
                _ => seen.push(written),
            }
        }
        if has_true && has_false {
            refuse(Refused::BothTrueAndFalse);
        }
    }
}
