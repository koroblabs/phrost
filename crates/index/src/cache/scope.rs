//! A file's declarations and imports, written and read back.

use lspkit_arena::ArenaId as _;
use phrost_defmap::{
    DeclarationSource, DeclaredSymbol, FileScope, ImportBinding, NamespaceScope, ScopeImports,
    ScopedAttributeLiteral,
};
use phrost_hir::{
    AttributeArgument, ClassId, ConstantId, FunctionId, ImportId, ImportKind, NamespaceId,
};
use phrost_model::{FileId, Interner, SymbolKind};

use super::bytes::{Reader, Writer};

pub(super) fn write(out: &mut Writer<'_>, scope: &FileScope) {
    out.list(scope.scopes(), write_namespace);
    out.list(scope.symbols(), write_symbol);
    // Written rather than re-derived, because there is no HIR to re-derive it from: a warm start
    // never lowers the file. A catalog key a stored file forgot to claim is a service or an entity
    // that resolves on a cold start and not on a warm one, which is the shape of "it was wrong until
    // I typed something".
    out.list(scope.attribute_literals(), |out, literal| {
        out.usize(literal.symbol as usize);
        out.str(literal.attribute.as_str());
        // A name or a position, and which it is has to survive: a contract on `id:` must not match
        // a literal that was written first and unnamed, or a warm start would claim keys a cold one
        // does not.
        match &literal.argument {
            AttributeArgument::Named(name) => {
                out.usize(0);
                out.str(name.as_str());
            }
            AttributeArgument::At(position) => {
                out.usize(1);
                out.usize(*position as usize);
            }
        }
        out.str(&literal.value);
        // Written even though nearly every literal has none: a key is what tells an entry of an
        // array argument from the argument itself, and a warm start that lost it would register
        // fewer catalog keys than a cold one.
        out.option(literal.key.as_ref(), |out, key| out.str(key));
    });
}

pub(super) fn read(
    input: &mut Reader<'_>,
    interner: &mut Interner,
    file: FileId,
) -> Option<FileScope> {
    let scopes = input.list(|input| read_namespace(input, interner))?;
    let symbols = input.list(|input| read_symbol(input, interner))?;
    let literals = input.list(|input| {
        let symbol = u32::try_from(input.usize()?).ok()?;
        let attribute = interner.name(input.str()?);
        let argument = match input.usize()? {
            0 => AttributeArgument::Named(interner.name(input.str()?)),
            1 => AttributeArgument::At(u16::try_from(input.usize()?).ok()?),
            _ => return None,
        };
        Some(ScopedAttributeLiteral {
            symbol,
            attribute,
            argument,
            value: input.str()?.into(),
            key: input.option(|input| input.str().map(Into::into))?,
        })
    })?;
    Some(FileScope::from_parts(file, scopes, symbols, literals))
}

fn write_namespace(out: &mut Writer<'_>, scope: &NamespaceScope) {
    out.option(scope.namespace().as_ref(), |out, id| {
        out.usize(id.index());
    });
    out.option(scope.name(), |out, name| out.str(name.as_str()));
    out.range(scope.range());

    for kind in [
        ImportKind::ClassLike,
        ImportKind::Function,
        ImportKind::Constant,
    ] {
        out.list(scope.imports().table(kind), |out, binding| {
            out.usize(binding.import().index());
            out.str(binding.key().as_str());
            out.str(binding.local().as_str());
            out.str(binding.path().as_str());
        });
    }
}

fn read_namespace(input: &mut Reader<'_>, interner: &mut Interner) -> Option<NamespaceScope> {
    let namespace = input.option(|input| Some(NamespaceId::from_index(input.usize()?)))?;
    let name = input.option(|input| Some(interner.name(input.str()?)))?;
    let range = input.range()?;

    let mut tables = Vec::with_capacity(3);
    for _ in 0..3 {
        tables.push(input.list(|input| {
            let import = ImportId::from_index(input.usize()?);
            let key = interner.key(input.str()?);
            let local = interner.name(input.str()?);
            let path = interner.name(input.str()?);
            Some(ImportBinding::from_parts(import, key, local, path))
        })?);
    }
    let mut tables = tables.into_iter();
    let imports = ScopeImports::from_parts(tables.next()?, tables.next()?, tables.next()?);

    Some(NamespaceScope::from_parts(namespace, name, range, imports))
}

fn write_symbol(out: &mut Writer<'_>, symbol: &DeclaredSymbol) {
    out.str(symbol.fqn.as_str());
    out.u8(symbol_tag(symbol.kind));
    match symbol.source {
        DeclarationSource::Class(id) => {
            out.u8(0);
            out.usize(id.index());
        }
        DeclarationSource::Function(id) => {
            out.u8(1);
            out.usize(id.index());
        }
        DeclarationSource::Constant(id) => {
            out.u8(2);
            out.usize(id.index());
        }
    }
    out.u8(symbol.words.bits());
    out.range(symbol.name_range);
    out.range(symbol.full_range);
    out.option(symbol.docblock_range.as_ref(), |out, range| {
        out.range(*range);
    });
}

fn read_symbol(input: &mut Reader<'_>, interner: &mut Interner) -> Option<DeclaredSymbol> {
    let fqn = interner.name(input.str()?);
    let kind = symbol_of(input.u8()?)?;
    let source = match input.u8()? {
        0 => DeclarationSource::Class(ClassId::from_index(input.usize()?)),
        1 => DeclarationSource::Function(FunctionId::from_index(input.usize()?)),
        2 => DeclarationSource::Constant(ConstantId::from_index(input.usize()?)),
        _ => return None,
    };

    Some(DeclaredSymbol {
        fqn,
        kind,
        source,
        words: phrost_defmap::SymbolWords::from_bits(input.u8()?),
        name_range: input.range()?,
        full_range: input.range()?,
        docblock_range: input.option(Reader::range)?,
    })
}

/// Tags rather than the discriminant: a reordered enum must not change what a written file means.
pub(super) fn symbol_tag(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Class => 0,
        SymbolKind::Interface => 1,
        SymbolKind::Trait => 2,
        SymbolKind::Enum => 3,
        SymbolKind::Function => 4,
        SymbolKind::Method => 5,
        SymbolKind::Property => 6,
        SymbolKind::ClassConstant => 7,
        SymbolKind::EnumCase => 8,
        SymbolKind::Constant => 9,
        SymbolKind::Namespace => 10,
        SymbolKind::Variable => 11,
        SymbolKind::Parameter => 12,
        // Never written: nothing declares a keyword or a catalog key, so no scope holds one. Tagged
        // anyway, because the reader below has to be able to say "not a symbol I know" rather than
        // panic.
        SymbolKind::Keyword => 13,
        SymbolKind::CatalogKey => 14,
        // Nor a file, which declares nothing: the kind exists so "who calls this" can name where a
        // top-level call was written.
        SymbolKind::File => 15,
    }
}

pub(super) fn symbol_of(tag: u8) -> Option<SymbolKind> {
    Some(match tag {
        0 => SymbolKind::Class,
        1 => SymbolKind::Interface,
        2 => SymbolKind::Trait,
        3 => SymbolKind::Enum,
        4 => SymbolKind::Function,
        5 => SymbolKind::Method,
        6 => SymbolKind::Property,
        7 => SymbolKind::ClassConstant,
        8 => SymbolKind::EnumCase,
        9 => SymbolKind::Constant,
        10 => SymbolKind::Namespace,
        11 => SymbolKind::Variable,
        12 => SymbolKind::Parameter,
        _ => return None,
    })
}
