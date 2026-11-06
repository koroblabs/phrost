//! Classes, their members, and free functions, written and read back.
//!
//! Every range here has to survive the round trip: a member's `name_range` is the goto target, and
//! §5's survivable-data rule exists because a range that does not come back turns navigation into a
//! jump to `0:0`. That is what the invariant test checks, field by field, without naming any.

use phrost_model::{Interner, Visibility};
use phrost_sema::{
    ClassKind, ClassRef, ConditionTest, Contract, Hooks, MemberKind, MemberModifiers, MemberRow,
    ParamPromotion, ParamRow, ReturnBranch, ShapeLiteral, TestCase, TraitAdaptation,
    TraitAdaptationKind,
};
use phrost_ty::TypeId;

use super::bytes::{Reader, Writer};
use crate::skeleton::TemplateRow;

pub(super) fn write_class_ref(out: &mut Writer<'_>, reference: &ClassRef) {
    out.str(reference.fqn.as_str());
    out.range(reference.range);
}

pub(super) fn read_class_ref(input: &mut Reader<'_>, interner: &mut Interner) -> Option<ClassRef> {
    Some(ClassRef {
        fqn: interner.name(input.str()?),
        range: input.range()?,
    })
}

pub(super) fn write_adaptation(out: &mut Writer<'_>, adaptation: &TraitAdaptation) {
    out.option(adaptation.trait_fqn.as_ref(), |out, name| {
        out.str(name.as_str());
    });
    out.str(adaptation.method.as_str());
    out.range(adaptation.method_range);
    match &adaptation.kind {
        TraitAdaptationKind::Precedence { excluded } => {
            out.u8(0);
            out.list(excluded, |out, name| out.str(name.as_str()));
        }
        TraitAdaptationKind::Alias { alias, visibility } => {
            out.u8(1);
            out.option(alias.as_ref(), |out, name| out.str(name.as_str()));
            out.option(visibility.as_ref(), |out, value| {
                out.u8(visibility_tag(*value));
            });
        }
    }
}

pub(super) fn read_adaptation(
    input: &mut Reader<'_>,
    interner: &mut Interner,
) -> Option<TraitAdaptation> {
    let trait_fqn = input.option(|input| Some(interner.name(input.str()?)))?;
    let method = interner.name(input.str()?);
    let method_range = input.range()?;
    let kind = match input.u8()? {
        0 => TraitAdaptationKind::Precedence {
            excluded: input.list(|input| Some(interner.name(input.str()?)))?,
        },
        1 => TraitAdaptationKind::Alias {
            alias: input.option(|input| Some(interner.name(input.str()?)))?,
            visibility: input.option(|input| visibility_of(input.u8()?))?,
        },
        _ => return None,
    };

    Some(TraitAdaptation {
        trait_fqn,
        method,
        method_range,
        kind,
    })
}

pub(super) fn write_member(out: &mut Writer<'_>, row: &MemberRow) {
    out.str(row.name.as_str());
    out.u8(member_tag(row.kind));
    out.u8(visibility_tag(row.visibility));
    out.option(row.set_visibility.as_ref(), |out, value| {
        out.u8(visibility_tag(*value));
    });
    write_modifiers(out, row.modifiers);
    write_ty(out, row.ty);
    write_ty(out, row.doc_ty);
    out.list(&row.params, write_param);
    out.range(row.name_range);
    out.range(row.full_range);
    out.option(row.docblock_range.as_ref(), |out, range| out.range(*range));
    out.option(row.contract.as_deref(), write_contract);
    out.bool(row.from_docblock);
    out.bool(row.ty_from_value);
    out.bool(row.doc_ty_from_body);
    out.bool(row.doc_ty_refused);
    // Last, and read last below -- §8's rule about a codec's field order.
    out.bool(row.reads_own_arguments);
}

pub(super) fn read_member(input: &mut Reader<'_>, interner: &mut Interner) -> Option<MemberRow> {
    Some(MemberRow {
        name: interner.name(input.str()?),
        kind: member_of(input.u8()?)?,
        visibility: visibility_of(input.u8()?)?,
        set_visibility: input.option(|input| visibility_of(input.u8()?))?,
        modifiers: read_modifiers(input)?,
        ty: read_ty(input)?,
        doc_ty: read_ty(input)?,
        params: input
            .list(|input| read_param(input, interner))?
            .into_boxed_slice(),
        name_range: input.range()?,
        full_range: input.range()?,
        docblock_range: input.option(Reader::range)?,
        contract: input
            .option(|input| read_contract(input, interner))?
            .map(Box::new),
        from_docblock: input.bool()?,
        ty_from_value: input.bool()?,
        doc_ty_from_body: input.bool()?,
        doc_ty_refused: input.bool()?,
        reads_own_arguments: input.bool()?,
    })
}

pub(super) fn write_param(out: &mut Writer<'_>, param: &ParamRow) {
    out.str(param.name.as_str());
    write_ty(out, param.ty);
    write_ty(out, param.doc_ty);
    out.bool(param.has_default);
    // The value as written, which is what the repair for a required method writes back. Field order
    // is read order, so this sits between the two bools in both halves.
    out.option(param.default.as_ref(), |out, written| out.str(written));
    out.bool(param.is_variadic);
    out.bool(param.by_reference);
    out.option(param.promotion.as_ref(), |out, promotion| {
        out.u8(visibility_tag(promotion.visibility));
        out.option(promotion.set_visibility.as_ref(), |out, value| {
            out.u8(visibility_tag(*value));
        });
        out.bool(promotion.is_readonly);
    });
    // Last, and read last below. Kept adjacent in the diff for the reason §8 records: a codec's
    // field order is its read order, and a struct literal does not say so.
    write_ty(out, param.out_ty);
    write_ty(out, param.callback_params_ty);
    write_ty(out, param.closure_this_ty);
    out.bool(param.documented);
}

pub(super) fn read_param(input: &mut Reader<'_>, interner: &mut Interner) -> Option<ParamRow> {
    Some(ParamRow {
        name: interner.name(input.str()?),
        ty: read_ty(input)?,
        doc_ty: read_ty(input)?,
        has_default: input.bool()?,
        default: input.option(|input| input.str().map(Into::into))?,
        is_variadic: input.bool()?,
        by_reference: input.bool()?,
        promotion: input.option(|input| {
            Some(ParamPromotion {
                visibility: visibility_of(input.u8()?)?,
                set_visibility: input.option(|input| visibility_of(input.u8()?))?,
                is_readonly: input.bool()?,
            })
        })?,
        out_ty: read_ty(input)?,
        callback_params_ty: read_ty(input)?,
        closure_this_ty: read_ty(input)?,
        documented: input.bool()?,
    })
}

fn write_modifiers(out: &mut Writer<'_>, modifiers: MemberModifiers) {
    // One byte for the words, one for the hook shape. Through `bits`/`from_bits` rather than a
    // bool each, so the stream stays the shape of the struct and an unknown bit is masked off
    // rather than read back as a word nothing declared.
    out.u8(modifiers.bits());
    out.u8(hooks_tag(modifiers.hooks));
}

fn read_modifiers(input: &mut Reader<'_>) -> Option<MemberModifiers> {
    let mut modifiers = MemberModifiers::from_bits(input.u8()?);
    modifiers.hooks = hooks_of(input.u8()?)?;
    Some(modifiers)
}

pub(super) fn write_template(out: &mut Writer<'_>, row: &TemplateRow) {
    out.str(row.name.as_str());
    write_ty(out, row.bound);
}

pub(super) fn read_template(
    input: &mut Reader<'_>,
    interner: &mut Interner,
) -> Option<TemplateRow> {
    Some(TemplateRow {
        name: interner.name(input.str()?),
        bound: read_ty(input)?,
    })
}

pub(super) fn write_ty(out: &mut Writer<'_>, ty: Option<TypeId>) {
    out.option(ty.as_ref(), |out, id| out.u32(id.as_raw()));
}

pub(super) fn read_ty(input: &mut Reader<'_>) -> Option<Option<TypeId>> {
    input.option(|input| Some(TypeId::from_raw(input.u32()?)))
}

/// Written out rather than derived from the enum's order, so adding a state cannot silently
/// change what an older cache means.
fn hooks_tag(hooks: Hooks) -> u8 {
    match hooks {
        Hooks::None => 0,
        Hooks::Settable => 1,
        Hooks::BackedGet => 2,
        Hooks::DeclaredGet => 3,
        Hooks::VirtualGet => 4,
    }
}

fn hooks_of(tag: u8) -> Option<Hooks> {
    Some(match tag {
        0 => Hooks::None,
        1 => Hooks::Settable,
        2 => Hooks::BackedGet,
        3 => Hooks::DeclaredGet,
        4 => Hooks::VirtualGet,
        _ => return None,
    })
}

pub(super) fn class_kind_tag(kind: ClassKind) -> u8 {
    match kind {
        ClassKind::Class => 0,
        ClassKind::Interface => 1,
        ClassKind::Trait => 2,
        ClassKind::Enum => 3,
    }
}

pub(super) fn class_kind_of(tag: u8) -> Option<ClassKind> {
    Some(match tag {
        0 => ClassKind::Class,
        1 => ClassKind::Interface,
        2 => ClassKind::Trait,
        3 => ClassKind::Enum,
        _ => return None,
    })
}

fn member_tag(kind: MemberKind) -> u8 {
    match kind {
        MemberKind::Method => 0,
        MemberKind::Property => 1,
        MemberKind::ClassConstant => 2,
        MemberKind::EnumCase => 3,
    }
}

fn member_of(tag: u8) -> Option<MemberKind> {
    Some(match tag {
        0 => MemberKind::Method,
        1 => MemberKind::Property,
        2 => MemberKind::ClassConstant,
        3 => MemberKind::EnumCase,
        _ => return None,
    })
}

fn visibility_tag(visibility: Visibility) -> u8 {
    match visibility {
        Visibility::Public => 0,
        Visibility::Protected => 1,
        Visibility::Private => 2,
    }
}

fn visibility_of(tag: u8) -> Option<Visibility> {
    Some(match tag {
        0 => Visibility::Public,
        1 => Visibility::Protected,
        2 => Visibility::Private,
        _ => return None,
    })
}

/// The codec for what a declaration promises beyond its signature.
///
/// Shared by members and free functions, because both carry one and a second copy of these tags
/// would be a second chance to write them differently.
pub(super) fn write_contract(out: &mut Writer<'_>, contract: &Contract) {
    match contract {
        Contract::Receiver { ty, when_true } => {
            out.u8(0);
            write_ty(out, Some(*ty));
            out.bool(*when_true);
        }
        Contract::Argument {
            parameter,
            ty,
            negated,
            when_true,
            from_class_string,
        } => {
            out.u8(1);
            out.str(parameter.as_str());
            write_ty(out, Some(*ty));
            out.bool(*negated);
            out.option(when_true.as_ref(), |out, when_true| out.bool(*when_true));
            out.option(from_class_string.as_ref(), |out, position| {
                out.u32(u32::from(*position))
            });
        }
        Contract::ReturnsArgumentClass {
            parameter,
            absent,
            many,
            beside,
        } => {
            out.u8(2);
            out.u32(u32::from(*parameter));
            // Written even though it is barely a byte: a warm start that lost the sentinel would
            // answer non-null where a cold one answers `?T`, which is narrower than the truth and
            // reports correct code -- the one direction these carry.
            out.option(absent.as_ref(), |out, absence| {
                out.u8(super::types::absence_tag(*absence))
            });
            out.bool(*many);
            // Last, and last in the reader too: a field put anywhere else is read out of its
            // neighbour's bytes.
            out.bool(*beside);
        }
        Contract::ConditionalReturn {
            argument,
            test,
            then,
            otherwise,
            by_default,
        } => {
            out.u8(3);
            out.u32(u32::from(*argument));
            write_test(out, test);
            write_branch(out, then.clone());
            write_branch(out, otherwise.clone());
            out.option(by_default.as_ref(), |out, holds| out.bool(*holds));
        }
        Contract::AgreedReturn(branch) => {
            out.u8(4);
            write_branch(out, branch.clone());
        }
        Contract::ReturnsApplied {
            class,
            arguments,
            many,
            alternatives,
        } => {
            out.u8(5);
            write_ty(out, Some(*class));
            out.u32(u32::try_from(arguments.len()).unwrap_or(u32::MAX));
            for argument in arguments {
                write_branch(out, argument.clone());
            }
            out.bool(*many);
            out.u32(u32::try_from(alternatives.len()).unwrap_or(u32::MAX));
            for alternative in alternatives {
                write_ty(out, Some(*alternative));
            }
        }
        Contract::ReturnsOneOf(operands) => {
            out.u8(6);
            out.u32(u32::try_from(operands.len()).unwrap_or(u32::MAX));
            for operand in operands {
                write_branch(out, operand.clone());
            }
        }
        Contract::LiteralCases {
            argument,
            cases,
            otherwise,
        } => {
            out.u8(7);
            out.u32(u32::from(*argument));
            out.u32(u32::try_from(cases.len()).unwrap_or(u32::MAX));
            for (literal, branch) in cases {
                out.str(literal.as_str());
                write_branch(out, branch.clone());
            }
            write_branch(out, otherwise.clone());
        }
        Contract::TestChain { cases, otherwise } => {
            out.u8(8);
            out.u32(u32::try_from(cases.len()).unwrap_or(u32::MAX));
            for case in cases {
                out.u32(u32::from(case.argument));
                write_test(out, &case.test);
                out.bool(case.negated);
                write_branch(out, case.then.clone());
                out.option(case.by_default.as_ref(), |out, holds| out.bool(*holds));
            }
            write_branch(out, otherwise.clone());
        }
        Contract::ReceiverOut { class, arguments } => {
            out.u8(9);
            write_ty(out, Some(*class));
            out.u32(u32::try_from(arguments.len()).unwrap_or(u32::MAX));
            for argument in arguments {
                write_branch(out, argument.clone());
            }
        }
        Contract::ConstantEntry { table, parameter } => {
            out.u8(10);
            out.u32(u32::from(*parameter));
            match table {
                phrost_sema::ConstantTable::Global(names) => {
                    out.u8(0);
                    out.list(names, |out, name| out.str(name.as_str()));
                }
                phrost_sema::ConstantTable::Member { class, name } => {
                    out.u8(1);
                    write_ty(out, Some(*class));
                    out.str(name.as_str());
                }
            }
        }
    }
}

/// One condition test. Shared by a conditional and by every link of a chain, so the two cannot
/// come to spell a test differently.
fn write_test(out: &mut Writer<'_>, test: &ConditionTest) {
    match test {
        ConditionTest::Null => out.u8(0),
        ConditionTest::True => out.u8(1),
        ConditionTest::False => out.u8(2),
        ConditionTest::ClassString(bound) => {
            out.u8(3);
            write_ty(out, *bound);
        }
        ConditionTest::Literal(written) => {
            out.u8(4);
            out.str(written.as_str());
        }
        ConditionTest::OfKind(kind) => {
            out.u8(5);
            out.u8(super::types::builtin_tag(*kind));
        }
        ConditionTest::NonEmpty => out.u8(6),
        ConditionTest::Empty => out.u8(9),
        ConditionTest::NonEmptyString => out.u8(11),
        ConditionTest::NonFalsyString => out.u8(12),
        ConditionTest::Number(written) => {
            out.u8(7);
            out.u64(written.cast_unsigned());
        }
        ConditionTest::OfKindOrInstance { kind, class } => {
            out.u8(8);
            out.u8(super::types::builtin_tag(*kind));
            write_ty(out, Some(*class));
        }
        ConditionTest::Instance(class) => {
            out.u8(13);
            write_ty(out, Some(*class));
        }
        ConditionTest::Scalar => out.u8(14),
        ConditionTest::Shape {
            key,
            accepts,
            sealed,
        } => {
            out.u8(10);
            out.str(key.as_str());
            out.u32(u32::try_from(accepts.len()).unwrap_or(u32::MAX));
            for literal in accepts {
                match literal {
                    ShapeLiteral::String(text) => {
                        out.u8(0);
                        out.str(text.as_str());
                    }
                    ShapeLiteral::Int(value) => {
                        out.u8(1);
                        out.u64(u64::from_ne_bytes(value.to_ne_bytes()));
                    }
                    ShapeLiteral::True => out.u8(2),
                    ShapeLiteral::False => out.u8(3),
                    ShapeLiteral::Null => out.u8(4),
                }
            }
            out.bool(*sealed);
        }
    }
}

fn read_test(input: &mut Reader<'_>, interner: &mut Interner) -> Option<ConditionTest> {
    Some(match input.u8()? {
        0 => ConditionTest::Null,
        1 => ConditionTest::True,
        2 => ConditionTest::False,
        3 => ConditionTest::ClassString(read_ty(input)?),
        4 => ConditionTest::Literal(interner.name(input.str()?)),
        5 => ConditionTest::OfKind(super::types::builtin_of(input.u8()?)?),
        6 => ConditionTest::NonEmpty,
        9 => ConditionTest::Empty,
        11 => ConditionTest::NonEmptyString,
        12 => ConditionTest::NonFalsyString,
        7 => ConditionTest::Number(input.u64()?.cast_signed()),
        8 => ConditionTest::OfKindOrInstance {
            kind: super::types::builtin_of(input.u8()?)?,
            class: read_ty(input)??,
        },
        13 => ConditionTest::Instance(read_ty(input)??),
        14 => ConditionTest::Scalar,
        10 => {
            let key = interner.name(input.str()?);
            let count = input.u32()?;
            let mut accepts = Vec::with_capacity(count.min(16) as usize);
            for _ in 0..count {
                accepts.push(match input.u8()? {
                    0 => ShapeLiteral::String(interner.name(input.str()?)),
                    1 => ShapeLiteral::Int(i64::from_ne_bytes(input.u64()?.to_ne_bytes())),
                    2 => ShapeLiteral::True,
                    3 => ShapeLiteral::False,
                    4 => ShapeLiteral::Null,
                    _ => return None,
                });
            }
            ConditionTest::Shape {
                key,
                accepts: accepts.into_boxed_slice(),
                sealed: input.bool()?,
            }
        }
        _ => return None,
    })
}

fn write_branch(out: &mut Writer<'_>, branch: ReturnBranch) {
    match branch {
        ReturnBranch::Ty(ty) => {
            out.u8(0);
            write_ty(out, Some(ty));
        }
        ReturnBranch::ArgumentClass(position) => {
            out.u8(1);
            out.u32(u32::from(position));
        }
        ReturnBranch::ArgumentType {
            parameter,
            absent,
            evaluated,
        } => {
            out.u8(3);
            out.u32(u32::from(parameter));
            // What the declaration's own default states. A warm start that lost it would answer the
            // whole declared union where a cold one answers `?string`, which is the larger half of
            // the calls: 62 of 67 on one real project leave the argument out.
            out.option(absent.as_ref(), |out, builtin| {
                out.u8(super::types::builtin_tag(*builtin))
            });
            out.u8(u8::from(evaluated));
        }
        ReturnBranch::ArgumentElement { parameter, absent } => {
            out.u8(2);
            out.u32(u32::from(parameter));
            out.option(absent.as_ref(), |out, absence| {
                out.u8(super::types::absence_tag(*absence))
            });
        }
        ReturnBranch::ArgumentKey { parameter, absent } => {
            out.u8(9);
            out.u32(u32::from(parameter));
            out.option(absent.as_ref(), |out, absence| {
                out.u8(super::types::absence_tag(*absence))
            });
        }
        ReturnBranch::ArgumentElements { parameter } => {
            out.u8(4);
            out.u32(u32::from(parameter));
        }
        ReturnBranch::ArgumentKeys { parameter } => {
            out.u8(7);
            out.u32(u32::from(parameter));
        }
        ReturnBranch::ArgumentTypes { parameter } => {
            out.u8(14);
            out.u32(u32::from(parameter));
        }
        ReturnBranch::CombinedElements { keys, values } => {
            out.u8(10);
            out.u32(u32::from(keys));
            out.u32(u32::from(values));
        }
        ReturnBranch::ArgumentColumn { parameter, column } => {
            out.u8(8);
            out.u32(u32::from(parameter));
            out.u32(u32::from(column));
        }
        ReturnBranch::CallbackResults { parameter } => {
            out.u8(5);
            out.u32(u32::from(parameter));
        }
        ReturnBranch::CallbackResult { parameter } => {
            out.u8(6);
            out.u32(u32::from(parameter));
        }
        ReturnBranch::Conditional(conditional) => {
            out.u8(11);
            out.u32(u32::from(conditional.argument));
            write_test(out, &conditional.test);
            write_branch(out, conditional.then);
            write_branch(out, conditional.otherwise);
            out.option(conditional.by_default.as_ref(), |out, holds| {
                out.bool(*holds)
            });
        }
        ReturnBranch::Unfilled => out.u8(13),
        ReturnBranch::Applied { class, arguments } => {
            out.u8(12);
            write_ty(out, Some(class));
            out.u32(u32::try_from(arguments.len()).unwrap_or(u32::MAX));
            for argument in arguments {
                write_branch(out, argument);
            }
        }
    }
}

fn read_branch(input: &mut Reader<'_>, interner: &mut Interner) -> Option<ReturnBranch> {
    match input.u8()? {
        0 => Some(ReturnBranch::Ty(read_ty(input)??)),
        1 => Some(ReturnBranch::ArgumentClass(
            u16::try_from(input.u32()?).unwrap_or(u16::MAX),
        )),
        2 => Some(ReturnBranch::ArgumentElement {
            // In the order the writer wrote them, for the reason the branch below states.
            parameter: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
            absent: input.option(|input| super::types::absence_of(input.u8()?))?,
        }),
        3 => Some(ReturnBranch::ArgumentType {
            // In the order the writer wrote them: a struct literal's fields are evaluated in source
            // order, so reading `absent` first silently reads the parameter's bytes as a flag.
            parameter: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
            absent: input.option(|input| super::types::builtin_of(input.u8()?))?,
            evaluated: input.u8()? != 0,
        }),
        4 => Some(ReturnBranch::ArgumentElements {
            parameter: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
        }),
        5 => Some(ReturnBranch::CallbackResults {
            parameter: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
        }),
        6 => Some(ReturnBranch::CallbackResult {
            parameter: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
        }),
        7 => Some(ReturnBranch::ArgumentKeys {
            parameter: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
        }),
        10 => Some(ReturnBranch::CombinedElements {
            keys: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
            values: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
        }),
        8 => Some(ReturnBranch::ArgumentColumn {
            parameter: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
            column: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
        }),
        9 => Some(ReturnBranch::ArgumentKey {
            parameter: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
            absent: input.option(|input| super::types::absence_of(input.u8()?))?,
        }),
        11 => {
            let argument = u16::try_from(input.u32()?).unwrap_or(u16::MAX);
            let test = read_test(input, interner)?;
            let then = read_branch(input, interner)?;
            let otherwise = read_branch(input, interner)?;
            let by_default = input.option(Reader::bool)?;
            Some(ReturnBranch::Conditional(Box::new(
                phrost_sema::ConditionalBranch {
                    argument,
                    test,
                    then,
                    otherwise,
                    by_default,
                },
            )))
        }
        13 => Some(ReturnBranch::Unfilled),
        12 => {
            let class = read_ty(input)??;
            let count = input.u32()?;
            let mut arguments = Vec::with_capacity(count.min(16) as usize);
            for _ in 0..count {
                arguments.push(read_branch(input, interner)?);
            }
            Some(ReturnBranch::Applied {
                class,
                arguments: arguments.into_boxed_slice(),
            })
        }
        14 => Some(ReturnBranch::ArgumentTypes {
            parameter: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
        }),
        _ => None,
    }
}

pub(super) fn read_contract(input: &mut Reader<'_>, interner: &mut Interner) -> Option<Contract> {
    match input.u8()? {
        0 => Some(Contract::Receiver {
            ty: read_ty(input)??,
            when_true: input.bool()?,
        }),
        1 => Some(Contract::Argument {
            parameter: interner.name(input.str()?),
            ty: read_ty(input)??,
            negated: input.bool()?,
            when_true: input.option(Reader::bool)?,
            from_class_string: input
                .option(Reader::u32)?
                .map(|position| u16::try_from(position).unwrap_or(u16::MAX)),
        }),
        2 => Some(Contract::ReturnsArgumentClass {
            // In the writer's order: a struct literal's fields are evaluated in source order, so
            // reading the sentinel first would read the parameter's bytes as a tag.
            parameter: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
            absent: input.option(|input| super::types::absence_of(input.u8()?))?,
            many: input.bool()?,
            // Last in both halves, which is where a new field goes.
            beside: input.bool()?,
        }),
        4 => Some(Contract::AgreedReturn(read_branch(input, interner)?)),
        8 => {
            let count = input.u32()?;
            let mut cases = Vec::with_capacity(count.min(64) as usize);
            for _ in 0..count {
                cases.push(TestCase {
                    argument: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
                    test: read_test(input, interner)?,
                    negated: input.bool()?,
                    then: read_branch(input, interner)?,
                    by_default: input.option(Reader::bool)?,
                });
            }
            Some(Contract::TestChain {
                cases: cases.into_boxed_slice(),
                otherwise: read_branch(input, interner)?,
            })
        }
        7 => {
            let argument = u16::try_from(input.u32()?).unwrap_or(u16::MAX);
            let count = input.u32()?;
            let mut cases = Vec::with_capacity(count.min(64) as usize);
            for _ in 0..count {
                let literal = interner.name(input.str()?);
                cases.push((literal, read_branch(input, interner)?));
            }
            Some(Contract::LiteralCases {
                argument,
                cases: cases.into_boxed_slice(),
                otherwise: read_branch(input, interner)?,
            })
        }
        5 => {
            let class = read_ty(input)??;
            let count = input.u32()?;
            let mut arguments = Vec::with_capacity(count.min(64) as usize);
            for _ in 0..count {
                arguments.push(read_branch(input, interner)?);
            }
            let many = input.bool()?;
            let count = input.u32()?;
            let mut alternatives = Vec::with_capacity(count.min(8) as usize);
            for _ in 0..count {
                alternatives.push(read_ty(input)??);
            }
            Some(Contract::ReturnsApplied {
                class,
                arguments: arguments.into_boxed_slice(),
                many,
                alternatives: alternatives.into_boxed_slice(),
            })
        }
        6 => {
            let count = input.u32()?;
            let mut operands = Vec::with_capacity(count.min(64) as usize);
            for _ in 0..count {
                operands.push(read_branch(input, interner)?);
            }
            Some(Contract::ReturnsOneOf(operands.into_boxed_slice()))
        }
        9 => {
            let class = read_ty(input)??;
            let count = input.u32()?;
            let mut arguments = Vec::with_capacity(count.min(64) as usize);
            for _ in 0..count {
                arguments.push(read_branch(input, interner)?);
            }
            Some(Contract::ReceiverOut {
                class,
                arguments: arguments.into_boxed_slice(),
            })
        }
        3 => Some(Contract::ConditionalReturn {
            argument: u16::try_from(input.u32()?).unwrap_or(u16::MAX),
            test: read_test(input, interner)?,
            then: read_branch(input, interner)?,
            otherwise: read_branch(input, interner)?,
            by_default: input.option(Reader::bool)?,
        }),
        10 => {
            // In the writer's order, for the reason the arms above give.
            let parameter = u16::try_from(input.u32()?).unwrap_or(u16::MAX);
            let table = match input.u8()? {
                0 => phrost_sema::ConstantTable::Global(
                    input
                        .list(|input| Some(interner.name(input.str()?)))?
                        .into_boxed_slice(),
                ),
                1 => phrost_sema::ConstantTable::Member {
                    class: read_ty(input)??,
                    name: interner.name(input.str()?),
                },
                _ => return None,
            };
            Some(Contract::ConstantEntry { table, parameter })
        }
        _ => None,
    }
}
