//! One file's contribution to the skeleton, written and read back.

use std::{path::Path, sync::Arc};

use phrost_model::{FileId, Interner, SourceOrigin};
use phrost_vfs::{FileFingerprint, FileStamp};

use crate::{
    ClassSkeleton, Dispatch, DispatchReceiver, DispatchRow, FileSkeleton, FoundTable, FunctionRow,
};

use super::{
    bytes::{Reader, Writer},
    members, scope,
};

/// One file's row and everything it said, read off the skeleton holding it -- the lists the
/// skeleton keeps per file beside the row, which only a lowered body could state.
pub(super) fn write(
    out: &mut Writer<'_>,
    skeleton: &crate::ProjectSkeleton,
    id: FileId,
    file: &FileSkeleton,
) {
    let dispatches = skeleton.dispatches_of(id);
    let tables = skeleton.declared_tables_of(id);
    let stated = skeleton.stated_of(id);
    let renders = skeleton.renders_of(id);
    let configured_this = skeleton
        .configured_this
        .get(&id)
        .map_or(&[][..], |calls| &calls[..]);
    let schema = skeleton.schema_of(id);
    let class_defaults = skeleton.class_defaults_of(id);
    let constructions = skeleton.constructions_of(id);
    let macros = skeleton.macros_of(id);
    out.str(&file.path().to_string_lossy());
    out.u8(origin_tag(file.origin()));
    let (hash, len) = file.stamp().parts();
    out.bytes(&hash);
    out.u64(len);
    out.bool(file.strict_types());

    scope::write(out, file.scope());
    out.list(file.classes(), write_class);
    out.list(file.functions(), |out, function| {
        out.u32(function.symbol());
        out.list(function.params(), members::write_param);
        out.option(function.ty().as_ref(), |out, id| {
            out.u32(id.as_raw());
        });
        out.option(function.doc_ty().as_ref(), |out, id| {
            out.u32(id.as_raw());
        });
        out.bool(function.returns_by_reference());
        out.option(function.contract(), members::write_contract);
        // Last, and read last below -- §8's rule that a codec's field order is its read order and
        // a positional constructor does not say so.
        out.bool(function.reads_own_arguments());
        out.bool(function.is_no_discard());
        out.bool(function.is_deprecated());
    });
    // Written rather than re-derived, for the reason the scope's attribute literals are: a warm
    // start never lowers the file, so there is no body left to read a dispatch out of. A file that
    // forgot to write one declares an extension point on a cold start and not on a warm one, which
    // is the shape of "it stopped working after a restart".
    out.list(dispatches, |out, row| {
        out.str(&row.catalog);
        out.str(row.call.name.as_str());
        out.range(row.call.range);
        out.range(row.call.name_range);
        out.str(&row.call.method);
        out.str(&row.call.required);
        out.option(row.call.receiver.as_ref(), |out, receiver| {
            out.str(&receiver.owner);
            out.str(&receiver.member);
        });
    });
    // Written for the same reason the dispatches above are: a warm start never lowers the file, so
    // the body that stated the table is gone and a kind the project declares itself would answer on
    // the first server of the day and on none after it.
    out.list(tables, |out, table| {
        out.str(&table.catalog);
        out.str(&table.kind);
        out.str(&table.declarer);
        out.list(&table.properties, |out, (name, holds)| {
            out.str(name);
            out.option(holds.as_ref(), |out, holds| {
                out.u8(super::types::builtin_tag(*holds))
            });
        });
        out.list(&table.written_at, |out, at| out.range(*at));
        out.bool(table.complete);
    });
    out.option(file.fingerprint().as_ref(), |out, fingerprint| {
        let (modified, changed, len) = fingerprint.parts();
        out.u64(modified);
        out.u64(changed);
        out.u64(len);
    });
    // The classes this file binds a closure to, read out of a body and so unreachable on a warm
    // start -- the same reason the word set below is written.
    out.list(file.bound_scopes(), |out, name| out.str(name.as_str()));
    // Last, and last in `read` too: a codec's field order is its read order, and a struct literal
    // does not say so. Written because a warm start never reads the file -- a `FileFingerprint`
    // match is the whole point of it -- so a word set derived on demand would be derived by
    // reading every file, which is the cost this exists to remove.
    out.option(file.words(), |out, words| {
        out.list(words.as_sorted(), |out, word| out.u64(*word));
    });
    // After the word set, so the note above about field order still names the last field. Written
    // for the reason all of these are: a `@throws` is read out of a docblock, the index keeps
    // ranges rather than text, and a warm start never opens the file to slice one out.
    out.list(file.throws(), |out, row| {
        out.option(row.owner.as_ref(), |out, owner| out.str(owner.as_str()));
        out.str(row.name.as_str());
        out.list(&row.classes, |out, class| out.str(class.as_str()));
    });
    // And after that, for the same reason again: a chain read out of a body, where the body is gone
    // by the time anybody asks and a warm start never had one.
    out.list(file.returned_paths(), |out, row| {
        out.option(row.owner.as_ref(), |out, owner| out.str(owner.as_str()));
        out.str(row.name.as_str());
        out.option(row.root.as_ref(), |out, root| out.str(root.as_str()));
        out.list(&row.steps, |out, step| out.str(step.as_str()));
        // Last, and read last below: a codec's field order is its read order.
        out.option(row.key.as_ref(), |out, key| match key {
            crate::ReturnedKey::Text(text) => {
                out.u8(0);
                out.str(text.as_str());
            }
            crate::ReturnedKey::Class(class) => {
                out.u8(1);
                out.str(class.as_str());
            }
        });
    });
    // After those: what a constant's value stated, which a warm start never reads.
    out.list(file.constants(), |out, row| {
        out.u32(row.symbol());
        write_constant_value(out, row.value());
    });
    // And last: the properties a class states in an array it writes -- a kept default or a body,
    // neither of which a warm start has.
    out.list(stated, |out, table| {
        out.str(&table.declarer);
        out.list(&table.rows, |out, row| {
            out.str(row.name.as_str());
            out.range(row.at);
            match &row.value {
                crate::StatedValue::Listed => out.u8(0),
                crate::StatedValue::Text(text) => {
                    out.u8(1);
                    out.str(text);
                }
                crate::StatedValue::Class(class) => {
                    out.u8(2);
                    out.str(class);
                }
                crate::StatedValue::Other => out.u8(3),
            }
            out.str(&row.source);
        });
        out.list(&table.getters, |out, getter| {
            out.str(getter.method.as_str());
            members::write_ty(out, Some(getter.ty));
        });
    });
    // And what renders a template, which only a body says.
    out.list(renders, |out, site| {
        out.str(&site.template);
        out.u32(u32::from(site.data));
        out.list(&site.keys, |out, key| out.str(key));
        out.u8(match site.kind {
            crate::RenderKind::Array => 0,
            crate::RenderKind::Compact => 1,
            crate::RenderKind::Shared => 2,
            crate::RenderKind::Named => 3,
            crate::RenderKind::Writes => 4,
        });
        out.option(site.class.as_ref(), |out, class| out.str(class.as_str()));
        out.bool(site.inherits);
    });
    // And what names the class a callee's closures run as, which only a body says.
    out.list(configured_this, |out, call| {
        out.list(&call.classes, |out, class| out.str(class.as_str()));
        out.option(call.within.as_ref(), |out, within| {
            out.list(within, |out, directory| out.str(directory));
        });
    });
    // And what a migration does to a table and which table a model names, which only a body and
    // a kept default say.
    let schema = schema.cloned().unwrap_or_default();
    out.list(&schema.changes, |out, change| {
        out.str(&change.table);
        match &change.kind {
            crate::schema::TableChangeKind::Created => out.u8(0),
            crate::schema::TableChangeKind::Altered => out.u8(1),
            crate::schema::TableChangeKind::Dropped => out.u8(2),
            crate::schema::TableChangeKind::RenamedTo(to) => {
                out.u8(3);
                out.str(to);
            }
        }
        out.list(&change.columns, |out, written| {
            use phrost_model::schema::ColumnChange;
            match &written.change {
                ColumnChange::Defined {
                    name,
                    holds,
                    nullable,
                    constrained,
                } => {
                    out.u8(0);
                    out.str(name);
                    out.u8(super::types::builtin_tag(*holds));
                    out.bool(*nullable);
                    out.bool(*constrained);
                }
                ColumnChange::Constrained(name) => {
                    out.u8(3);
                    out.str(name);
                }
                ColumnChange::Dropped(name) => {
                    out.u8(1);
                    out.str(name);
                }
                ColumnChange::Renamed { from, to } => {
                    out.u8(2);
                    out.str(from);
                    out.str(to);
                }
            }
            out.range(written.at);
        });
    });
    out.list(&schema.named, |out, named| {
        out.str(&named.class);
        out.str(&named.table);
    });
    // And the class a property's default names, which only a declaration's default says.
    out.list(class_defaults, |out, kept| {
        out.str(&kept.declarer);
        out.str(&kept.property);
        out.str(&kept.class);
    });
    // And the entries an object's construction declares, which a dependency's body says and a
    // warm start has no body to read.
    out.list(constructions, |out, site| {
        out.str(&site.catalog);
        out.str(&site.key);
        out.range(site.key_range);
        out.str(&site.declarer);
        match &site.callable {
            crate::constructions::Callable::Function(name) => {
                out.u32(0);
                out.str(name);
            }
            crate::constructions::Callable::Method { class, method } => {
                out.u32(1);
                out.str(class);
                out.str(method);
            }
            crate::constructions::Callable::Unnamed => out.u32(2),
        }
    });
    // And the methods a body gives classes at runtime, which a warm start has no body to read.
    out.list(macros, |out, site| {
        out.str(site.class.as_str());
        out.str(site.name.as_str());
        out.range(site.at);
        out.option(site.returns.as_ref(), |out, ty| out.u32(ty.as_raw()));
        out.option(site.chain.as_ref(), |out, chain| {
            out.option(chain.root.as_ref(), |out, root| out.str(root.as_str()));
            out.list(&chain.steps, |out, step| out.str(step.as_str()));
        });
    });
}

/// How deep a stored constant's value may nest before reading it is refused.
const CONSTANT_VALUE_DEPTH: u8 = 64;

/// A constant's value, as a tree: a tag, then what that kind carries.
fn write_constant_value(out: &mut Writer<'_>, value: &crate::constant::ConstantValue) {
    use crate::constant::ConstantValue;
    match value {
        ConstantValue::Ty(ty) => {
            out.u8(0);
            out.u32(ty.as_raw());
        }
        ConstantValue::Constant(names) => {
            out.u8(1);
            out.list(names, |out, name| out.str(name.as_str()));
        }
        ConstantValue::Arithmetic(operator, operands) => {
            out.u8(2);
            out.u8(match operator {
                phrost_hir::body::ArithmeticOp::Add => 0,
                phrost_hir::body::ArithmeticOp::Subtract => 1,
                phrost_hir::body::ArithmeticOp::Multiply => 2,
                phrost_hir::body::ArithmeticOp::Power => 3,
                phrost_hir::body::ArithmeticOp::Divide => 4,
            });
            for operand in operands.iter() {
                write_constant_value(out, operand);
            }
        }
        ConstantValue::Bitwise(operands) => {
            out.u8(3);
            for operand in operands.iter() {
                write_constant_value(out, operand);
            }
        }
    }
}

/// The other half, with the nesting bounded: the writer only ever writes what a source file nested,
/// and a corrupt length that claims more must become a rebuild rather than a blown stack.
fn read_constant_value(
    input: &mut Reader<'_>,
    interner: &mut Interner,
    depth: u8,
) -> Option<crate::constant::ConstantValue> {
    use crate::constant::ConstantValue;
    let depth = depth.checked_sub(1)?;
    let pair = |input: &mut Reader<'_>, interner: &mut Interner| {
        Some(Box::new([
            read_constant_value(input, interner, depth)?,
            read_constant_value(input, interner, depth)?,
        ]))
    };
    Some(match input.u8()? {
        0 => ConstantValue::Ty(phrost_ty::TypeId::from_raw(input.u32()?)),
        1 => ConstantValue::Constant(
            input
                .list(|input| Some(interner.name(input.str()?)))?
                .into_boxed_slice(),
        ),
        2 => {
            let operator = match input.u8()? {
                0 => phrost_hir::body::ArithmeticOp::Add,
                1 => phrost_hir::body::ArithmeticOp::Subtract,
                2 => phrost_hir::body::ArithmeticOp::Multiply,
                3 => phrost_hir::body::ArithmeticOp::Power,
                4 => phrost_hir::body::ArithmeticOp::Divide,
                _ => return None,
            };
            ConstantValue::Arithmetic(operator, pair(input, interner)?)
        }
        3 => ConstantValue::Bitwise(pair(input, interner)?),
        _ => return None,
    })
}

/// One file as the cache holds it: its rows, and the two things it *said* that the codec has to
/// carry because a warm start never lowers it again.
type CachedFile = (FileId, FileSkeleton, crate::skeleton::Said);

/// Reads one back, keyed by the id the caller assigned to its path.
pub(super) fn read(
    input: &mut Reader<'_>,
    interner: &mut Interner,
    assign: &mut impl FnMut(&Path) -> FileId,
) -> Option<CachedFile> {
    let path = Arc::from(Path::new(input.str()?));
    let origin = origin_of(input.u8()?)?;
    let stamp = FileStamp::from_parts(input.array::<32>()?, input.u64()?);
    let strict_types = input.bool()?;
    // The id comes from the caller, not the file: it is the VFS that owns file identity, and a
    // stored id would mean nothing in another session.
    let file = assign(&path);

    let scope = scope::read(input, interner, file)?;
    let classes = input.list(|input| read_class(input, interner))?;
    let functions = input.list(|input| {
        // Field by field and named, in the order `write_function` wrote them: a positional
        // constructor of eight says nothing about which bool is which.
        Some(FunctionRow {
            symbol: input.u32()?,
            params: input
                .list(|input| members::read_param(input, interner))?
                .into_boxed_slice(),
            ty: input.option(|input| Some(phrost_ty::TypeId::from_raw(input.u32()?)))?,
            doc_ty: input.option(|input| Some(phrost_ty::TypeId::from_raw(input.u32()?)))?,
            returns_by_reference: input.bool()?,
            contract: input
                .option(|input| members::read_contract(input, interner))?
                .map(Box::new),
            reads_own_arguments: input.bool()?,
            is_no_discard: input.bool()?,
            is_deprecated: input.bool()?,
        })
    })?;
    let dispatches = input.list(|input| {
        Some(DispatchRow {
            catalog: input.str()?.into(),
            call: Dispatch {
                file,
                name: interner.name(input.str()?),
                range: input.range()?,
                name_range: input.range()?,
                method: input.str()?.into(),
                required: input.str()?.into(),
                receiver: input.option(|input| {
                    Some(DispatchReceiver {
                        owner: input.str()?.into(),
                        member: input.str()?.into(),
                    })
                })?,
            },
        })
    })?;
    let tables = input.list(|input| {
        Some(FoundTable {
            catalog: input.str()?.into(),
            kind: input.str()?.into(),
            declarer: input.str()?.into(),
            properties: input.list(|input| {
                Some((
                    input.str()?.into(),
                    input.option(|input| super::types::builtin_of(input.u8()?))?,
                ))
            })?,
            written_at: input.list(|input| input.range())?,
            complete: input.bool()?,
        })
    })?;
    let fingerprint = input
        .option(|input| FileFingerprint::from_parts(input.u64()?, input.u64()?, input.u64()?))?;
    let bound_scopes = input
        .list(|input| Some(interner.name(input.str()?)))?
        .into_boxed_slice();
    let words = input.option(|input| {
        Some(crate::WordSet::from_sorted(
            input.list(|input| input.u64())?.into_boxed_slice(),
        ))
    })?;
    let throws = input
        .list(|input| {
            Some(crate::ThrownRow {
                owner: input.option(|input| Some(interner.name(input.str()?)))?,
                name: interner.name(input.str()?),
                classes: input
                    .list(|input| Some(interner.name(input.str()?)))?
                    .into_boxed_slice(),
            })
        })?
        .into_boxed_slice();
    // Matching `write`, in order.
    let returned_paths = input
        .list(|input| {
            Some(crate::ReturnedRow {
                owner: input.option(|input| Some(interner.name(input.str()?)))?,
                name: interner.name(input.str()?),
                root: input.option(|input| Some(interner.name(input.str()?)))?,
                steps: input
                    .list(|input| Some(interner.name(input.str()?)))?
                    .into_boxed_slice(),
                key: input.option(|input| {
                    let tag = input.u8()?;
                    let name = interner.name(input.str()?);
                    match tag {
                        0 => Some(crate::ReturnedKey::Text(name)),
                        1 => Some(crate::ReturnedKey::Class(name)),
                        _ => None,
                    }
                })?,
            })
        })?
        .into_boxed_slice();
    let constants = input
        .list(|input| {
            Some(crate::constant::ConstantRow {
                symbol: input.u32()?,
                value: read_constant_value(input, interner, CONSTANT_VALUE_DEPTH)?,
            })
        })?
        .into_boxed_slice();
    let stated = input
        .list(|input| {
            Some(crate::StatedTable {
                declarer: input.str()?.into(),
                rows: input.list(|input| {
                    Some(crate::StatedRow {
                        name: interner.name(input.str()?),
                        at: input.range()?,
                        value: match input.u8()? {
                            0 => crate::StatedValue::Listed,
                            1 => crate::StatedValue::Text(input.str()?.into()),
                            2 => crate::StatedValue::Class(input.str()?.into()),
                            3 => crate::StatedValue::Other,
                            _ => return None,
                        },
                        source: input.str()?.into(),
                    })
                })?,
                getters: input.list(|input| {
                    Some(crate::StatedGetter {
                        method: interner.name(input.str()?),
                        ty: members::read_ty(input)??,
                    })
                })?,
            })
        })?
        .into_boxed_slice();
    let renders = input
        .list(|input| {
            Some(crate::RenderSite {
                template: input.str()?.into(),
                data: phrost_model::TextSize::from(input.u32()?),
                keys: input
                    .list(|input| Some(Box::<str>::from(input.str()?)))?
                    .into_boxed_slice(),
                kind: match input.u8()? {
                    0 => crate::RenderKind::Array,
                    1 => crate::RenderKind::Compact,
                    2 => crate::RenderKind::Shared,
                    3 => crate::RenderKind::Named,
                    4 => crate::RenderKind::Writes,
                    _ => return None,
                },
                class: input.option(|input| Some(interner.name(input.str()?)))?,
                inherits: input.bool()?,
            })
        })?
        .into_boxed_slice();
    let configured_this = input
        .list(|input| {
            Some(crate::ConfiguredThis {
                classes: input
                    .list(|input| Some(interner.name(input.str()?)))?
                    .into_boxed_slice(),
                within: input.option(|input| {
                    Some(
                        input
                            .list(|input| Some(Box::<str>::from(input.str()?)))?
                            .into_boxed_slice(),
                    )
                })?,
            })
        })?
        .into_boxed_slice();
    let schema = crate::schema::FileSchema {
        changes: input.list(|input| {
            use crate::schema::TableChangeKind;
            use phrost_model::schema::ColumnChange;
            Some(crate::schema::TableChange {
                table: input.str()?.into(),
                kind: match input.u8()? {
                    0 => TableChangeKind::Created,
                    1 => TableChangeKind::Altered,
                    2 => TableChangeKind::Dropped,
                    3 => TableChangeKind::RenamedTo(input.str()?.into()),
                    _ => return None,
                },
                columns: input.list(|input| {
                    let change = match input.u8()? {
                        0 => ColumnChange::Defined {
                            name: interner.name(input.str()?).as_str().to_owned(),
                            holds: super::types::builtin_of(input.u8()?)?,
                            nullable: input.bool()?,
                            constrained: input.bool()?,
                        },
                        3 => ColumnChange::Constrained(input.str()?.to_owned()),
                        1 => ColumnChange::Dropped(input.str()?.to_owned()),
                        2 => ColumnChange::Renamed {
                            from: input.str()?.to_owned(),
                            to: interner.name(input.str()?).as_str().to_owned(),
                        },
                        _ => return None,
                    };
                    Some(crate::schema::WrittenColumn {
                        change,
                        at: input.range()?,
                    })
                })?,
            })
        })?,
        named: input.list(|input| {
            Some(crate::schema::NamedTable {
                class: input.str()?.into(),
                table: input.str()?.into(),
            })
        })?,
    };
    let class_defaults = input
        .list(|input| {
            Some(crate::class_defaults::ClassDefault {
                declarer: input.str()?.into(),
                property: input.str()?.into(),
                class: input.str()?.into(),
            })
        })?
        .into_boxed_slice();
    let constructions = input
        .list(|input| {
            Some(crate::constructions::ConstructionSite {
                catalog: input.str()?.into(),
                key: input.str()?.into(),
                key_range: input.range()?,
                declarer: input.str()?.into(),
                callable: match input.u32()? {
                    0 => crate::constructions::Callable::Function(input.str()?.into()),
                    1 => crate::constructions::Callable::Method {
                        class: input.str()?.into(),
                        method: input.str()?.into(),
                    },
                    _ => crate::constructions::Callable::Unnamed,
                },
            })
        })?
        .into_boxed_slice();
    let macros = input
        .list(|input| {
            Some(crate::macros::MacroSite {
                class: interner.name(input.str()?),
                name: interner.name(input.str()?),
                at: input.range()?,
                returns: input.option(|input| Some(phrost_ty::TypeId::from_raw(input.u32()?)))?,
                chain: input.option(|input| {
                    Some(crate::macros::MacroChain {
                        root: input.option(|input| Some(interner.name(input.str()?)))?,
                        steps: input
                            .list(|input| Some(interner.name(input.str()?)))?
                            .into_boxed_slice(),
                    })
                })?,
            })
        })?
        .into_boxed_slice();

    Some((
        file,
        FileSkeleton::from_parts(
            origin,
            stamp,
            fingerprint,
            path,
            scope,
            classes,
            functions,
            words,
            bound_scopes,
            strict_types,
            throws,
            returned_paths,
            constants,
        ),
        crate::skeleton::Said {
            dispatches: dispatches.into_boxed_slice(),
            tables: tables.into_boxed_slice(),
            stated,
            renders,
            configured_this,
            schema,
            class_defaults,
            constructions,
            macros,
        },
    ))
}

fn write_class(out: &mut Writer<'_>, class: &ClassSkeleton) {
    out.str(class.fqn().as_str());
    out.u8(members::class_kind_tag(class.kind()));
    out.bool(class.is_abstract());
    members::write_ty(out, class.enum_backing());
    out.bool(class.is_final());
    out.bool(class.is_readonly());
    out.bool(class.is_attribute());
    out.bool(class.is_deprecated());
    out.list(class.extends(), members::write_class_ref);
    out.list(class.implements(), members::write_class_ref);
    out.list(class.uses(), members::write_class_ref);
    out.list(class.adaptations(), members::write_adaptation);
    out.list(class.members(), members::write_member);
    out.list(class.templates(), members::write_template);
    out.list(class.supertype_arguments(), |out, id| out.u32(id.as_raw()));
    out.list(class.mixins(), members::write_class_ref);
}

fn read_class(input: &mut Reader<'_>, interner: &mut Interner) -> Option<ClassSkeleton> {
    Some(ClassSkeleton::from_parts(
        interner.name(input.str()?),
        members::class_kind_of(input.u8()?)?,
        input.bool()?,
        members::read_ty(input)?,
        input.bool()?,
        input.bool()?,
        input.bool()?,
        input.bool()?,
        input.list(|input| members::read_class_ref(input, interner))?,
        input.list(|input| members::read_class_ref(input, interner))?,
        input.list(|input| members::read_class_ref(input, interner))?,
        input.list(|input| members::read_adaptation(input, interner))?,
        input.list(|input| members::read_member(input, interner))?,
        input.list(|input| members::read_template(input, interner))?,
        input
            .list(|input| Some(phrost_ty::TypeId::from_raw(input.u32()?)))?
            .into_boxed_slice(),
        input
            .list(|input| members::read_class_ref(input, interner))?
            .into_boxed_slice(),
    ))
}

fn origin_tag(origin: SourceOrigin) -> u8 {
    match origin {
        SourceOrigin::User => 0,
        SourceOrigin::Vendor => 1,
        SourceOrigin::Stub => 2,
    }
}

fn origin_of(tag: u8) -> Option<SourceOrigin> {
    Some(match tag {
        0 => SourceOrigin::User,
        1 => SourceOrigin::Vendor,
        2 => SourceOrigin::Stub,
        _ => return None,
    })
}
