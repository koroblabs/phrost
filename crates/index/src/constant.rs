use phrost_defmap::{DeclarationSource, FileScope};
use phrost_hir::{ConstantValueData, FileHir, ImportKind, body::ArithmeticOp};
use phrost_model::{Interner, Name};
use phrost_ty::{TypeId, TypeTable};

use crate::types::TypeLowerer;

/// What one global or namespaced constant's value states, as the file declared it.
///
/// For a constant whose value the syntax settles -- `const LIMIT = 5`, `define('ABSPATH',
/// __DIR__ . '/')` -- and for one whose value is built from other constants, which only the whole
/// index can settle. The name and every range live on the declared symbol, for the reason
/// [`FunctionRow`](crate::FunctionRow) gives.
///
/// Stored, because the value is text in whichever file declares the constant and a closed file has
/// none: reading it back at the use would be the disk read §1.2 refuses.
#[derive(Debug)]
pub struct ConstantRow {
    /// Index into the file's declared symbols, the identity the catalog hands out.
    pub(crate) symbol: u32,
    pub(crate) value: ConstantValue,
}

/// What a constant's value states: a type, or the parts one is read off.
///
/// The parts are resolved as far as one file can take them -- a written name to the
/// fully-qualified ones PHP would try -- and no further, because which declaration a name reaches is
/// a question about every file and is asked where the type is wanted. See
/// [`ConstantValueData`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConstantValue {
    /// What the syntax settled.
    Ty(TypeId),
    /// Another constant: the names PHP would try, in its order -- the current namespace, then the
    /// global one for an unqualified name.
    Constant(Box<[Name]>),
    Arithmetic(ArithmeticOp, Box<[ConstantValue; 2]>),
    /// `&`, `|`, `^`, and `~x` as `x & x`.
    Bitwise(Box<[ConstantValue; 2]>),
}

impl ConstantRow {
    pub fn symbol(&self) -> u32 {
        self.symbol
    }

    pub fn value(&self) -> &ConstantValue {
        &self.value
    }

    /// One row per declared constant whose value names a type or is built from parts that may.
    pub(crate) fn collect(
        interner: &mut Interner,
        types: &mut TypeTable,
        scope: &FileScope,
        hir: &FileHir,
    ) -> Box<[ConstantRow]> {
        scope
            .symbols()
            .iter()
            .enumerate()
            .filter_map(|(index, symbol)| {
                let DeclarationSource::Constant(id) = symbol.source else {
                    return None;
                };
                let data = hir.constant(id);
                let mut lower = TypeLowerer {
                    hir,
                    namespace: scope.scope_of(data.namespace),
                    interner,
                    types,
                };
                let value = match (data.ty, &data.value) {
                    (Some(written), _) => ConstantValue::Ty(lower.lower(written)),
                    (None, Some(parts)) => ConstantValue::of(parts, scope, &mut lower),
                    (None, None) => return None,
                };
                Some(ConstantRow {
                    symbol: u32::try_from(index).expect("fewer than 4G symbols in one file"),
                    value,
                })
            })
            .collect()
    }
}

impl ConstantValue {
    fn of(parts: &ConstantValueData, scope: &FileScope, lower: &mut TypeLowerer<'_>) -> Self {
        match parts {
            ConstantValueData::Settled(written) => Self::Ty(lower.lower(*written)),
            // Against the namespace and imports where the name is written, which for a `define()`
            // inside a namespace is not the namespace the constant it declares lives in.
            ConstantValueData::Constant { path, anchor, at } => Self::Constant(
                scope
                    .scope_at(*at)
                    .resolve(path.as_str(), *anchor, ImportKind::Constant)
                    .candidates()
                    .map(|candidate| lower.interner.name(candidate.fqn()))
                    .collect(),
            ),
            ConstantValueData::Arithmetic { operator, operands } => {
                let [left, right] = operands.as_ref();
                Self::Arithmetic(
                    *operator,
                    Box::new([Self::of(left, scope, lower), Self::of(right, scope, lower)]),
                )
            }
            ConstantValueData::Bitwise(operands) => {
                let [left, right] = operands.as_ref();
                Self::Bitwise(Box::new([
                    Self::of(left, scope, lower),
                    Self::of(right, scope, lower),
                ]))
            }
        }
    }
}

impl lspkit_memsize::MemSize for ConstantRow {
    fn heap_size(&self) -> usize {
        self.value.heap_size()
    }
}

impl lspkit_memsize::MemSize for ConstantValue {
    /// The names a part may reach are interned and counted with the interner, so a part owns only
    /// the slices and boxes that hold it.
    fn heap_size(&self) -> usize {
        match self {
            Self::Ty(_) => 0,
            Self::Constant(names) => std::mem::size_of_val(&**names),
            Self::Arithmetic(_, operands) | Self::Bitwise(operands) => {
                std::mem::size_of_val(&**operands)
                    + operands.iter().map(Self::heap_size).sum::<usize>()
            }
        }
    }
}
