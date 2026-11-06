use phrost_defmap::NamespaceScope;
use phrost_hir::{FileHir, ImportKind, ParamId, TypeRefData, TypeRefId};
use phrost_model::Interner;
use phrost_sema::{ParamPromotion, ParamRow};
use phrost_ty::{TypeId, TypeTable};

/// Lowers a written type hint into the project-wide type table, resolving class names as it goes.
///
/// Resolution happens at ingest because it depends only on the declaring file's own namespace and
/// imports — never on what else the project contains — so a resolved type cannot go stale when
/// another file changes. `self`, `static`, and `parent` are the deliberate exception: they mean
/// different classes depending on the receiver a call is made through, so they stay unresolved.
pub(crate) struct TypeLowerer<'a> {
    pub(crate) hir: &'a FileHir,
    pub(crate) namespace: &'a NamespaceScope,
    pub(crate) interner: &'a mut Interner,
    pub(crate) types: &'a mut TypeTable,
}

impl TypeLowerer<'_> {
    pub(crate) fn lower(&mut self, id: TypeRefId) -> TypeId {
        // Copied out so the borrow of the tree is independent of the mutable borrow of `self`.
        let hir = self.hir;
        match hir.type_ref(id) {
            TypeRefData::Builtin(builtin) => self.types.builtin(*builtin),
            TypeRefData::Relative(relative) => self.types.relative(*relative),
            TypeRefData::Named(name_ref) => {
                let name_ref = hir.name_ref(*name_ref);
                let resolution = self.namespace.resolve(
                    name_ref.path.as_str(),
                    name_ref.anchor,
                    ImportKind::ClassLike,
                );
                let fqn = self.interner.name(resolution.primary().fqn());
                self.types.named(fqn)
            }
            TypeRefData::Nullable(inner) => {
                let inner = self.lower(*inner);
                self.types.nullable(inner)
            }
            TypeRefData::Union(parts) => {
                let operands = self.lower_all(hir.type_ref_list(*parts));
                self.types.union(&operands)
            }
            TypeRefData::Intersection(parts) => {
                let operands = self.lower_all(hir.type_ref_list(*parts));
                self.types.intersection(&operands)
            }
            TypeRefData::Generic {
                base,
                args,
                non_empty,
            } => {
                let base = self.lower(*base);
                let args = self.lower_all(hir.type_ref_list(*args));
                self.types.generic_of(base, &args, *non_empty)
            }
            TypeRefData::Shape { fields, open, list } => {
                let fields = self.lower_all(hir.type_ref_list(*fields));
                self.types.shape_of(&fields, *open, *list)
            }
            TypeRefData::ObjectShape { fields } => {
                let fields = self.lower_all(hir.type_ref_list(*fields));
                self.types.object_shape(&fields)
            }
            TypeRefData::ShapeField { key, optional, ty } => {
                let ty = self.lower(*ty);
                self.types.field(key.clone(), *optional, ty)
            }
        }
    }

    pub(crate) fn lower_optional(&mut self, id: Option<TypeRefId>) -> Option<TypeId> {
        id.map(|id| self.lower(id))
    }

    /// What a declaration promises beyond its signature, with its types interned.
    ///
    /// Shared by members and free functions: both carry one, and mapping it in two places would be
    /// two chances to map it differently.
    pub(crate) fn contract(
        &mut self,
        contract: &phrost_hir::ContractData,
    ) -> Option<phrost_sema::Contract> {
        Some(match contract {
            phrost_hir::ContractData::Receiver { ty, when_true } => {
                phrost_sema::Contract::Receiver {
                    ty: self.lower(*ty),
                    when_true: *when_true,
                }
            }
            phrost_hir::ContractData::Argument {
                parameter,
                ty,
                negated,
                when_true,
                from_class_string,
            } => phrost_sema::Contract::Argument {
                parameter: self.interner.name(parameter.as_str()),
                ty: self.lower(*ty),
                negated: *negated,
                when_true: *when_true,
                from_class_string: *from_class_string,
            },
            phrost_hir::ContractData::ReturnsArgumentClass {
                parameter,
                absent,
                many,
                beside,
            } => phrost_sema::Contract::ReturnsArgumentClass {
                parameter: *parameter,
                absent: *absent,
                many: *many,
                beside: *beside,
            },
            phrost_hir::ContractData::ReturnsApplied {
                class,
                arguments,
                many,
                alternatives,
            } => phrost_sema::Contract::ReturnsApplied {
                class: self.lower(*class),
                arguments: arguments
                    .iter()
                    .map(|argument| self.branch(argument))
                    .collect(),
                many: *many,
                alternatives: alternatives
                    .iter()
                    .map(|alternative| self.lower(*alternative))
                    .collect(),
            },
            phrost_hir::ContractData::ConstantEntry { table, parameter } => {
                let table = match table {
                    // Against the declaration's namespace and imports, the way a name in its
                    // docblock is resolved -- and every name PHP would try, since which of them is
                    // declared is a question about the whole index.
                    phrost_hir::ConstantTableData::Global { path, anchor } => {
                        phrost_sema::ConstantTable::Global(
                            self.namespace
                                .resolve(path.as_str(), *anchor, ImportKind::Constant)
                                .candidates()
                                .map(|candidate| self.interner.name(candidate.fqn()))
                                .collect(),
                        )
                    }
                    phrost_hir::ConstantTableData::Member { class, name } => {
                        phrost_sema::ConstantTable::Member {
                            class: self.lower(*class),
                            name: self.interner.name(name.as_str()),
                        }
                    }
                };
                phrost_sema::Contract::ConstantEntry {
                    table,
                    parameter: *parameter,
                }
            }
            phrost_hir::ContractData::ReceiverOut { class, arguments } => {
                phrost_sema::Contract::ReceiverOut {
                    class: self.lower(*class),
                    arguments: arguments
                        .iter()
                        .map(|argument| self.branch(argument))
                        .collect(),
                }
            }
            phrost_hir::ContractData::ReturnsOneOf(operands) => {
                phrost_sema::Contract::ReturnsOneOf(
                    operands
                        .iter()
                        .map(|operand| self.branch(operand))
                        .collect(),
                )
            }
            phrost_hir::ContractData::ConditionalReturn {
                argument,
                test,
                then,
                otherwise,
                by_default,
            } => phrost_sema::Contract::ConditionalReturn {
                argument: *argument,
                test: self.condition_test(test),
                then: self.branch(then),
                otherwise: self.branch(otherwise),
                by_default: *by_default,
            },
            phrost_hir::ContractData::LiteralCases {
                argument,
                cases,
                otherwise,
            } => phrost_sema::Contract::LiteralCases {
                argument: *argument,
                cases: cases
                    .iter()
                    .map(|(literal, branch)| (literal.clone(), self.branch(branch)))
                    .collect(),
                otherwise: self.branch(otherwise),
            },
            phrost_hir::ContractData::TestChain { cases, otherwise } => {
                phrost_sema::Contract::TestChain {
                    cases: cases
                        .iter()
                        .map(|case| phrost_sema::TestCase {
                            argument: case.argument,
                            test: self.condition_test(&case.test),
                            negated: case.negated,
                            then: self.branch(&case.then),
                            by_default: case.by_default,
                        })
                        .collect(),
                    otherwise: self.branch(otherwise),
                }
            }
            phrost_hir::ContractData::AgreedReturn(branch) => {
                phrost_sema::Contract::AgreedReturn(self.branch(branch))
            }
        })
    }

    /// One condition test, with the class an instance test names interned.
    fn condition_test(
        &mut self,
        test: &phrost_hir::ConditionTestData,
    ) -> phrost_sema::ConditionTest {
        match test {
            phrost_hir::ConditionTestData::Null => phrost_sema::ConditionTest::Null,
            phrost_hir::ConditionTestData::True => phrost_sema::ConditionTest::True,
            phrost_hir::ConditionTestData::False => phrost_sema::ConditionTest::False,
            phrost_hir::ConditionTestData::Literal(written) => {
                phrost_sema::ConditionTest::Literal(written.clone())
            }
            phrost_hir::ConditionTestData::ClassString(bound) => {
                phrost_sema::ConditionTest::ClassString(bound.map(|bound| self.lower(bound)))
            }
            phrost_hir::ConditionTestData::OfKind(kind) => {
                phrost_sema::ConditionTest::OfKind(*kind)
            }
            phrost_hir::ConditionTestData::Number(written) => {
                phrost_sema::ConditionTest::Number(*written)
            }
            phrost_hir::ConditionTestData::NonEmpty => phrost_sema::ConditionTest::NonEmpty,
            phrost_hir::ConditionTestData::Empty => phrost_sema::ConditionTest::Empty,
            phrost_hir::ConditionTestData::NonEmptyString => {
                phrost_sema::ConditionTest::NonEmptyString
            }
            phrost_hir::ConditionTestData::NonFalsyString => {
                phrost_sema::ConditionTest::NonFalsyString
            }
            phrost_hir::ConditionTestData::Shape {
                key,
                accepts,
                sealed,
            } => phrost_sema::ConditionTest::Shape {
                key: key.clone(),
                accepts: accepts
                    .iter()
                    .map(|literal| match literal {
                        phrost_hir::ShapeLiteralData::String(text) => {
                            phrost_sema::ShapeLiteral::String(text.clone())
                        }
                        phrost_hir::ShapeLiteralData::Int(value) => {
                            phrost_sema::ShapeLiteral::Int(*value)
                        }
                        phrost_hir::ShapeLiteralData::True => phrost_sema::ShapeLiteral::True,
                        phrost_hir::ShapeLiteralData::False => phrost_sema::ShapeLiteral::False,
                        phrost_hir::ShapeLiteralData::Null => phrost_sema::ShapeLiteral::Null,
                    })
                    .collect(),
                sealed: *sealed,
            },
            phrost_hir::ConditionTestData::OfKindOrInstance { kind, class } => {
                phrost_sema::ConditionTest::OfKindOrInstance {
                    kind: *kind,
                    class: self.lower(*class),
                }
            }
            phrost_hir::ConditionTestData::Instance(class) => {
                phrost_sema::ConditionTest::Instance(self.lower(*class))
            }
            phrost_hir::ConditionTestData::Scalar => phrost_sema::ConditionTest::Scalar,
        }
    }

    /// One branch of a conditional return type, with its type interned.
    fn branch(&mut self, branch: &phrost_hir::ReturnBranchData) -> phrost_sema::ReturnBranch {
        match branch {
            phrost_hir::ReturnBranchData::Ty(id) => phrost_sema::ReturnBranch::Ty(self.lower(*id)),
            phrost_hir::ReturnBranchData::ArgumentClass(position) => {
                phrost_sema::ReturnBranch::ArgumentClass(*position)
            }
            phrost_hir::ReturnBranchData::ArgumentType {
                parameter,
                absent,
                evaluated,
            } => phrost_sema::ReturnBranch::ArgumentType {
                absent: *absent,
                parameter: *parameter,
                evaluated: *evaluated,
            },
            phrost_hir::ReturnBranchData::ArgumentElement { parameter, absent } => {
                phrost_sema::ReturnBranch::ArgumentElement {
                    parameter: *parameter,
                    absent: *absent,
                }
            }
            phrost_hir::ReturnBranchData::ArgumentKey { parameter, absent } => {
                phrost_sema::ReturnBranch::ArgumentKey {
                    parameter: *parameter,
                    absent: *absent,
                }
            }
            phrost_hir::ReturnBranchData::ArgumentElements { parameter } => {
                phrost_sema::ReturnBranch::ArgumentElements {
                    parameter: *parameter,
                }
            }
            phrost_hir::ReturnBranchData::ArgumentTypes { parameter } => {
                phrost_sema::ReturnBranch::ArgumentTypes {
                    parameter: *parameter,
                }
            }
            phrost_hir::ReturnBranchData::CombinedElements { keys, values } => {
                phrost_sema::ReturnBranch::CombinedElements {
                    keys: *keys,
                    values: *values,
                }
            }
            phrost_hir::ReturnBranchData::ArgumentKeys { parameter } => {
                phrost_sema::ReturnBranch::ArgumentKeys {
                    parameter: *parameter,
                }
            }
            phrost_hir::ReturnBranchData::ArgumentColumn { parameter, column } => {
                phrost_sema::ReturnBranch::ArgumentColumn {
                    parameter: *parameter,
                    column: *column,
                }
            }
            phrost_hir::ReturnBranchData::CallbackResults { parameter } => {
                phrost_sema::ReturnBranch::CallbackResults {
                    parameter: *parameter,
                }
            }
            phrost_hir::ReturnBranchData::CallbackResult { parameter } => {
                phrost_sema::ReturnBranch::CallbackResult {
                    parameter: *parameter,
                }
            }
            phrost_hir::ReturnBranchData::Conditional(conditional) => {
                phrost_sema::ReturnBranch::Conditional(Box::new(phrost_sema::ConditionalBranch {
                    argument: conditional.argument,
                    test: self.condition_test(&conditional.test),
                    then: self.branch(&conditional.then),
                    otherwise: self.branch(&conditional.otherwise),
                    by_default: conditional.by_default,
                }))
            }
            phrost_hir::ReturnBranchData::Unfilled => phrost_sema::ReturnBranch::Unfilled,
            phrost_hir::ReturnBranchData::Applied { class, arguments } => {
                phrost_sema::ReturnBranch::Applied {
                    class: self.lower(*class),
                    arguments: arguments
                        .iter()
                        .map(|argument| self.branch(argument))
                        .collect(),
                }
            }
        }
    }

    /// One parameter, with its type resolved. Shared by methods and free functions so the two
    /// cannot end up describing the same parameter differently.
    pub(crate) fn param(&mut self, id: ParamId) -> ParamRow {
        let param = self.hir.param(id);
        ParamRow {
            // Interned, not cloned: the HIR may come from another interner -- see `ingest_lowered`
            // -- and a parameter name cloned from it is a second allocation of `$value`.
            name: self.interner.name(param.name.as_str()),
            ty: self.lower_optional(param.type_ref),
            doc_ty: self.lower_optional(param.doc_type_ref),
            has_default: param.has_default,
            default: param.default.clone(),
            is_variadic: param.is_variadic,
            by_reference: param.by_reference,
            promotion: param.promotion.as_ref().map(|promotion| ParamPromotion {
                visibility: promotion.visibility,
                set_visibility: promotion.set_visibility,
                is_readonly: promotion.modifiers.is_readonly(),
            }),
            out_ty: self.lower_optional(param.out_type_ref),
            callback_params_ty: self.lower_optional(param.callback_params_type_ref),
            closure_this_ty: self.lower_optional(param.closure_this_type_ref),
            // Whether the declaration said *anything* about this parameter, which is not what
            // `doc_ty` says: a `@param` naming a template is refused on the way in and leaves the
            // slot empty. See `ParamRow::documented`.
            documented: param.doc_type_ref.is_some() || param.doc_type_refused,
        }
    }

    fn lower_all(&mut self, ids: &[TypeRefId]) -> Vec<TypeId> {
        ids.iter().map(|&id| self.lower(id)).collect()
    }
}
