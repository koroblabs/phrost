use phrost_hir::{AttributeArgument, ClassKind, FileHir, ImportId, ImportKind, NamespaceId};
use phrost_model::{FileId, Interner, Name, NameKey, SymbolKind, TextRange, TextSize};

use crate::symbol::{DeclarationSource, DeclaredSymbol, SymbolWords};

/// One file's namespaces, imports, and declared symbols.
///
/// Built from the HIR once per file revision. This is the per-file half of name resolution; the
/// project-wide half — which file declares which FQN — belongs to whoever owns the catalog.
#[derive(Debug)]
pub struct FileScope {
    file: FileId,
    /// Index 0 is always the global scope, so there is no "no scope" case to handle. Namespace
    /// scopes follow in source order.
    scopes: Vec<NamespaceScope>,
    symbols: Vec<DeclaredSymbol>,
    attribute_literals: Vec<ScopedAttributeLiteral>,
}

/// A named string literal an attribute wrote on one of this file's declarations.
///
/// Kept on the scope rather than on each symbol, because nearly no declaration has one: a real
/// thirty-thousand-file project holds about three thousand of these against a hundred thousand
/// symbols, so a field per symbol would cost more than the whole table.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScopedAttributeLiteral {
    /// Index into [`FileScope::symbols`], which is the identity the catalog stores.
    pub symbol: u32,
    /// The attribute's name as written. Compared by its last segment, which is how one is written.
    pub attribute: Name,
    /// Which argument of the attribute it was: a name, or a position for an argument the call did
    /// not name.
    pub argument: AttributeArgument,
    /// What was written. For an entry of an array argument, the entry's value.
    pub value: Box<str>,
    /// The key the entry was written under, for an argument a framework reads the keys of.
    pub key: Option<Box<str>>,
}

impl lspkit_memsize::MemSize for ScopedAttributeLiteral {
    /// The value and any key. The names are pointers into the interner and a position is a number.
    fn heap_size(&self) -> usize {
        self.value.len() + self.key.as_ref().map_or(0, |key| key.len())
    }
}

impl lspkit_memsize::MemSize for FileScope {
    /// The scopes and the symbols. Names inside them are pointers into the interner.
    fn heap_size(&self) -> usize {
        lspkit_memsize::vec(&self.scopes)
            + lspkit_memsize::vec_flat(&self.symbols)
            + lspkit_memsize::vec(&self.attribute_literals)
    }
}

impl lspkit_memsize::MemSize for NamespaceScope {
    fn heap_size(&self) -> usize {
        self.imports.heap_size()
    }
}

impl lspkit_memsize::MemSize for ScopeImports {
    /// Three lists, one per symbol table PHP keeps. Each binding is two names and a kind, and the
    /// names are pointers.
    fn heap_size(&self) -> usize {
        lspkit_memsize::vec_flat(&self.class_like)
            + lspkit_memsize::vec_flat(&self.function)
            + lspkit_memsize::vec_flat(&self.constant)
    }
}

impl FileScope {
    /// Builds a file's scope from its HIR.
    pub fn build(interner: &mut Interner, hir: &FileHir) -> Self {
        // Exactly as many as there will be, because the count is known and a vector grown by
        // doubling keeps what it grew past: see the note at the end of this function.
        let mut scopes = Vec::with_capacity(hir.namespaces().len() + 1);
        scopes.push(NamespaceScope {
            namespace: None,
            name: None,
            // The global scope covers the whole file, so a lookup always lands somewhere even in
            // a file that declares no namespace at all.
            range: TextRange::new(TextSize::from(0), TextSize::from(u32::MAX)),
            imports: ScopeImports::default(),
        });

        for (id, namespace) in hir.namespaces() {
            scopes.push(NamespaceScope {
                namespace: Some(id),
                // Interned rather than cloned: the HIR may have been lowered against another
                // interner, and a clone is then a second allocation of a namespace every file in
                // the package repeats. See `ProjectSkeleton::ingest_lowered`.
                name: namespace
                    .name
                    .as_ref()
                    .map(|name| interner.name(name.as_str())),
                range: namespace.full_range,
                imports: ScopeImports::default(),
            });
        }

        // Both counts are exact and both are O(1) to take, so these lists are allocated once at the
        // size they will end at. See [`Self::tighten`] for what the alternative costs.
        let mut scope = Self {
            file: hir.file(),
            scopes,
            symbols: Vec::with_capacity(
                hir.classes().len() + hir.functions().len() + hir.constants().len(),
            ),
            attribute_literals: Vec::with_capacity(
                hir.classes()
                    .map(|(_, class)| class.attribute_literals.len())
                    .sum(),
            ),
        };

        for (id, import) in hir.imports() {
            let binding = ImportBinding {
                import: id,
                key: interner.key_of(&import.local),
                local: import.local.clone(),
                path: import.path.clone(),
            };
            scope
                .scope_mut(import.namespace)
                .imports
                .push(import.kind, binding);
        }

        scope.collect_symbols(interner, hir);
        scope.tighten();
        scope
    }

    /// Gives back the capacity the import lists grew past, which nothing will ever use.
    ///
    /// A scope is built once per file and never appended to -- `scope_mut` is private to
    /// [`Self::build`] and a re-ingest builds a new one -- so what a vector holds beyond its length
    /// here is held for the life of the process. A `Vec` grown by pushing keeps up to twice what it
    /// needs, and `lspkit-memsize` counts capacity because capacity is what is resident.
    ///
    /// What made this worth doing is that the *cache* path already had it: on a 30 916-file checkout
    /// the scope table was 27.4 MB built and 14.0 MB read back, because `from_parts` is handed
    /// vectors of a known length. So a session that started cold carried 13.4 MB until it ended and
    /// an identical warm session did not.
    ///
    /// Only the imports, because only they cannot be sized ahead: the three lists live inside each
    /// namespace and which kind a `use` line goes to is known when it is read. Sizing the two lists
    /// that *can* be sized rather than shrinking them is what keeps this off the bootstrap clock --
    /// shrinking all four cost 3.03 s -> 3.17 s on that checkout, and sizing two of them costs
    /// nothing measurable.
    fn tighten(&mut self) {
        for scope in &mut self.scopes {
            scope.imports.class_like.shrink_to_fit();
            scope.imports.function.shrink_to_fit();
            scope.imports.constant.shrink_to_fit();
        }
    }

    /// Rebuilds a scope from what the accessors below gave, for reading one back from a cache.
    ///
    /// The only other way to make one is [`Self::build`], from a file's HIR. Two constructors is a
    /// risk worth naming: they must agree, and what keeps them agreeing is the `cold == warm`
    /// invariant, which builds an index both ways and compares the whole rendering.
    pub fn from_parts(
        file: FileId,
        scopes: Vec<NamespaceScope>,
        symbols: Vec<DeclaredSymbol>,
        attribute_literals: Vec<ScopedAttributeLiteral>,
    ) -> Self {
        Self {
            file,
            scopes,
            symbols,
            attribute_literals,
        }
    }

    pub fn file(&self) -> FileId {
        self.file
    }

    pub fn scopes(&self) -> &[NamespaceScope] {
        &self.scopes
    }

    pub fn symbols(&self) -> &[DeclaredSymbol] {
        &self.symbols
    }

    pub fn attribute_literals(&self) -> &[ScopedAttributeLiteral] {
        &self.attribute_literals
    }

    /// The symbol a HIR declaration produced.
    ///
    /// By identity rather than by matching ranges: two declarations can share a range in broken
    /// source, and a positional match would then attach the wrong fully-qualified name.
    pub fn symbol_of(&self, source: DeclarationSource) -> Option<&DeclaredSymbol> {
        self.symbols.iter().find(|symbol| symbol.source == source)
    }

    /// The innermost namespace scope containing `offset`.
    ///
    /// Later declarations win, which is what makes several brace-delimited namespaces in one file
    /// resolve independently. Falls back to the global scope, which always exists.
    /// Drops the import tables, keeping the namespaces and the declared symbols.
    ///
    /// For a file nobody edits. Imports are read where a name is *written*: resolving a reference
    /// in an open buffer, offering a completion, judging a `use` unused. A closed dependency's
    /// declarations were resolved against its imports when it was ingested -- every type in a
    /// member row and every parent in a class row is a qualified name -- and no reader asks a closed
    /// file's scope anything but which symbols it declares. On a 30 965-file checkout the tables
    /// were about half of the 14 MB the scopes cost, and the same bytes again in the stored index.
    pub fn forget_imports(&mut self) {
        for scope in &mut self.scopes {
            scope.imports = ScopeImports::default();
        }
    }

    pub fn scope_at(&self, offset: TextSize) -> &NamespaceScope {
        self.scopes
            .iter()
            .skip(1)
            .rev()
            .find(|scope| scope.range.contains_inclusive(offset))
            .unwrap_or(&self.scopes[0])
    }

    /// The scope a HIR namespace id belongs to.
    pub fn scope_of(&self, namespace: Option<NamespaceId>) -> &NamespaceScope {
        self.scopes
            .iter()
            .find(|scope| scope.namespace == namespace)
            .unwrap_or(&self.scopes[0])
    }

    fn scope_mut(&mut self, namespace: Option<NamespaceId>) -> &mut NamespaceScope {
        let index = self
            .scopes
            .iter()
            .position(|scope| scope.namespace == namespace)
            .unwrap_or(0);
        &mut self.scopes[index]
    }

    /// Computes the fully-qualified name of every declaration in the file.
    fn collect_symbols(&mut self, interner: &mut Interner, hir: &FileHir) {
        for (id, class) in hir.classes() {
            let fqn = self.qualify(interner, class.namespace, &class.name);
            let symbol = u32::try_from(self.symbols.len()).expect("symbol count fits in u32");
            for literal in hir.attribute_literal_list(class.attribute_literals) {
                let data = hir.attribute_literal(*literal);
                self.attribute_literals.push(ScopedAttributeLiteral {
                    symbol,
                    attribute: data.attribute.clone(),
                    argument: data.argument.clone(),
                    value: data.value.clone(),
                    key: data.key.clone(),
                });
            }
            self.symbols.push(DeclaredSymbol {
                fqn,
                kind: match class.kind {
                    ClassKind::Class => SymbolKind::Class,
                    ClassKind::Interface => SymbolKind::Interface,
                    ClassKind::Trait => SymbolKind::Trait,
                    ClassKind::Enum => SymbolKind::Enum,
                },
                source: DeclarationSource::Class(id),
                // `new class { ... }` declares a class whose name is synthesised from where its
                // `class` keyword sits. It is claimed like any other declaration -- `$this`,
                // `self`, `static` and `parent` inside the body resolve through the catalog, and
                // without a claim every member reached through them is unresolvable. Nobody can
                // write the name, so it is marked unnameable and never offered as one.
                words: SymbolWords::new(
                    !class.is_anonymous,
                    class.is_conditional,
                    class.is_ignored,
                    class.modifiers.is_deprecated,
                ),
                name_range: class.name_range,
                full_range: class.full_range,
                docblock_range: class.docblock.map(|id| hir.docblock(id).range),
            });
        }

        for (id, function) in hir.functions() {
            let fqn = self.qualify(interner, function.namespace, &function.name);
            self.symbols.push(DeclaredSymbol {
                fqn,
                kind: SymbolKind::Function,
                source: DeclarationSource::Function(id),
                words: SymbolWords::new(
                    true,
                    function.is_conditional,
                    function.is_ignored,
                    function.is_deprecated,
                ),
                name_range: function.name_range,
                full_range: function.full_range,
                docblock_range: function.docblock.map(|id| hir.docblock(id).range),
            });
        }

        for (id, constant) in hir.constants() {
            let fqn = self.qualify(interner, constant.namespace, &constant.name);
            self.symbols.push(DeclaredSymbol {
                fqn,
                kind: SymbolKind::Constant,
                source: DeclarationSource::Constant(id),
                words: SymbolWords::new(
                    true,
                    constant.is_conditional,
                    constant.is_ignored,
                    constant.is_deprecated,
                ),
                name_range: constant.name_range,
                full_range: constant.full_range,
                docblock_range: constant.docblock.map(|id| hir.docblock(id).range),
            });
        }
    }

    fn qualify(
        &self,
        interner: &mut Interner,
        namespace: Option<NamespaceId>,
        name: &Name,
    ) -> Name {
        match self.scope_of(namespace).name.as_ref() {
            Some(prefix) => interner.name(&format!("{prefix}\\{name}")),
            None => interner.name(name.as_str()),
        }
    }
}

/// One namespace's worth of resolution context.
#[derive(Debug)]
pub struct NamespaceScope {
    namespace: Option<NamespaceId>,
    name: Option<Name>,
    range: TextRange,
    imports: ScopeImports,
}

impl NamespaceScope {
    /// Rebuilds one, for reading a scope back from a cache. See [`FileScope::from_parts`].
    pub fn from_parts(
        namespace: Option<NamespaceId>,
        name: Option<Name>,
        range: TextRange,
        imports: ScopeImports,
    ) -> Self {
        Self {
            namespace,
            name,
            range,
            imports,
        }
    }

    /// The namespace prefix, or `None` for the global namespace.
    pub fn name(&self) -> Option<&Name> {
        self.name.as_ref()
    }

    pub fn namespace(&self) -> Option<NamespaceId> {
        self.namespace
    }

    pub fn range(&self) -> TextRange {
        self.range
    }

    pub fn imports(&self) -> &ScopeImports {
        &self.imports
    }

    pub fn is_global(&self) -> bool {
        self.name.is_none()
    }
}

/// A scope's imports, split by the symbol table they bind into.
///
/// Three separate lists rather than one with a kind field: every lookup already knows which table
/// it wants, and filtering a mixed list on every resolution is work that the shape can avoid.
#[derive(Debug, Default)]
pub struct ScopeImports {
    class_like: Vec<ImportBinding>,
    function: Vec<ImportBinding>,
    constant: Vec<ImportBinding>,
}

impl ScopeImports {
    /// Rebuilds one, for reading a scope back from a cache. See [`FileScope::from_parts`].
    pub fn from_parts(
        class_like: Vec<ImportBinding>,
        function: Vec<ImportBinding>,
        constant: Vec<ImportBinding>,
    ) -> Self {
        Self {
            class_like,
            function,
            constant,
        }
    }

    pub fn table(&self, kind: ImportKind) -> &[ImportBinding] {
        match kind {
            ImportKind::ClassLike => &self.class_like,
            ImportKind::Function => &self.function,
            ImportKind::Constant => &self.constant,
        }
    }

    /// Finds the binding for a locally-written name, using the case rules of its table.
    ///
    /// Class and function names are matched case-insensitively, constants exactly — which is why
    /// `use App\Repo;` lets you write `repo` while `use const App\MAX;` does not let you write
    /// `max`. Folding constants too would resolve names PHP would not.
    pub fn lookup(&self, kind: ImportKind, local: &str) -> Option<&ImportBinding> {
        let table = self.table(kind);
        match kind {
            ImportKind::Constant => table.iter().find(|binding| binding.local.as_str() == local),
            ImportKind::ClassLike | ImportKind::Function => table
                .iter()
                .find(|binding| binding.key.as_str().eq_ignore_ascii_case(local)),
        }
    }

    fn push(&mut self, kind: ImportKind, binding: ImportBinding) {
        match kind {
            ImportKind::ClassLike => self.class_like.push(binding),
            ImportKind::Function => self.function.push(binding),
            ImportKind::Constant => self.constant.push(binding),
        }
    }
}

/// One `use` statement's binding, from the local name to the path it stands for.
#[derive(Clone, Debug)]
pub struct ImportBinding {
    import: ImportId,
    /// Case-folded local name, for the two tables PHP matches case-insensitively.
    key: NameKey,
    /// Local name as written, for the constant table, which matches exactly.
    local: Name,
    path: Name,
}

impl ImportBinding {
    /// Rebuilds one, for reading a scope back from a cache. See [`FileScope::from_parts`].
    pub fn from_parts(import: ImportId, key: NameKey, local: Name, path: Name) -> Self {
        Self {
            import,
            key,
            local,
            path,
        }
    }

    /// The folded local name, which is the key the two case-insensitive tables match on.
    pub fn key(&self) -> &NameKey {
        &self.key
    }

    /// The HIR import this came from — the goto target for the name, and what rename must rewrite.
    pub fn import(&self) -> ImportId {
        self.import
    }

    pub fn local(&self) -> &Name {
        &self.local
    }

    pub fn path(&self) -> &Name {
        &self.path
    }
}
