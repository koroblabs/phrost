use phrost_model::{FileId, Name, NameKey, SymbolKind};
use std::{collections::BTreeMap, ops::Bound};

use rustc_hash::FxHashMap;

/// Which files declare which fully-qualified names.
///
/// Three tables, matching PHP's three symbol tables, so the case rules are structural rather than
/// a condition someone has to remember: class-like and function names are keyed by their folded
/// form, constants by the name exactly as written. A single folded map would resolve `max` to a
/// constant named `MAX`, which PHP does not.
#[derive(Debug, Default)]
pub struct OwnerCatalog {
    class_like: FxHashMap<NameKey, Vec<Declarer>>,
    functions: FxHashMap<NameKey, Vec<Declarer>>,
    constants: FxHashMap<Name, Vec<Declarer>>,
    /// Folded **short** name to declarers, ordered, for prefix queries.
    ///
    /// A separate structure because the question is a different one. Resolution asks "what is
    /// `App\Repo`" and wants a hash lookup; completion asks "what starts with `rep`" and wants a
    /// range. A hash map cannot answer the second, and scanning it per keystroke on a project with
    /// tens of thousands of classes is not affordable.
    ///
    /// Folded even for constants, whose names PHP compares exactly. Completion is a suggestion, and
    /// suggesting `MAX` for `ma` is what a person wants; resolution still goes through the exact
    /// table, so nothing *resolves* case-insensitively that should not.
    short_names: BTreeMap<NameKey, ShortName>,
    /// Every namespace a nameable declaration sits under, by folded path, ordered for the same
    /// reason the short names are: completion asks what a word *begins*, and a hash map cannot say.
    ///
    /// A namespace is declared by nothing of its own -- what says one exists is that something sits
    /// under it -- so this counts what does, and the path leaves with the last of it. Every
    /// ancestor and not only the innermost: `Illuminate\JsonSchema` holds no class of its own and is
    /// still where `Types\ArrayType` is reached from. Kept by the same two calls that keep the tables,
    /// so a cold start, a warm one and an edit cannot disagree about it; what it replaced was a walk
    /// over every qualified name on every keystroke of a bare word.
    namespaces: BTreeMap<NameKey, Namespace>,
}

/// One namespace, as the catalog knows it: each way a declaration under it writes it, with how
/// many nameable declarations at any depth write it that way.
///
/// Counted per spelling rather than once, so that which spelling is shown is the least of those
/// still written -- the same answer whatever order files were read and edited in. Keeping only the
/// least one seen let an edit that removed it leave a spelling nothing writes any more, where an
/// index built from scratch would show another.
#[derive(Debug, Default)]
struct Namespace {
    /// Sorted, so the first is the one shown. Nearly always one entry.
    spellings: Vec<(Name, u32)>,
}

impl Namespace {
    fn spelled(&self) -> Option<&Name> {
        self.spellings.first().map(|(spelled, _)| spelled)
    }

    fn claims(&self) -> u32 {
        self.spellings.iter().map(|(_, claims)| claims).sum()
    }
}

/// Every claim on one folded short name, with the letters that name holds.
///
/// The mask rides with the declarers rather than in a map beside them, for the reason the memory
/// rule gives: a second map keyed by the same name is the same bytes twice and one more place for
/// the two to disagree.
#[derive(Debug, Default)]
struct ShortName {
    /// One bit per `a`-`z` the name holds, computed when it was first claimed.
    letters: u32,
    declarers: Vec<Declarer>,
}

/// One file's claim on a name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Declarer {
    pub file: FileId,
    /// Index into that file's declared symbols.
    pub symbol: u32,
    pub kind: SymbolKind,
}

impl Declarer {
    /// Whether this is the claim the symbol at `index` of `file` made.
    fn made(&self, (file, index): (FileId, u32)) -> bool {
        self.file == file && self.symbol == index
    }
}

impl lspkit_memsize::MemSize for OwnerCatalog {
    /// Four tables from a name to the files that declare it, and the sorted short-name index.
    ///
    /// This is the one that must not scale with `vendor/`, and the report exists to say whether it
    /// does. Keys are names -- pointers into the interner -- so what is counted is the tables and
    /// the declarer lists.
    fn heap_size(&self) -> usize {
        let lists = |values: &mut dyn Iterator<Item = &Vec<Declarer>>| {
            values.map(lspkit_memsize::vec_flat).sum::<usize>()
        };

        lspkit_memsize::table::<NameKey, Vec<Declarer>>(self.class_like.len())
            + lists(&mut self.class_like.values())
            + lspkit_memsize::table::<NameKey, Vec<Declarer>>(self.functions.len())
            + lists(&mut self.functions.values())
            + lspkit_memsize::table::<Name, Vec<Declarer>>(self.constants.len())
            + lists(&mut self.constants.values())
            // A `BTreeMap` node holds up to eleven entries; counting one entry's worth each is the
            // same documented estimate the hash tables get.
            + self.short_names.len() * (size_of::<NameKey>() + size_of::<ShortName>())
            + lists(&mut self.short_names.values().map(|short| &short.declarers))
            + self.namespaces.len() * (size_of::<NameKey>() + size_of::<Namespace>())
            + self
                .namespaces
                .values()
                .map(|held| lspkit_memsize::vec_flat(&held.spellings))
                .sum::<usize>()
    }
}

impl OwnerCatalog {
    /// Adds a claim. Order of insertion does not matter; ranking happens on read.
    ///
    /// `short` is `None` for a declaration whose name cannot be written -- an anonymous class. Such
    /// a name still has to *resolve*, because `$this` inside its body names it, so it goes into the
    /// table resolution reads; the short-name index is the one every "what could be typed here"
    /// walk ranges over, so leaving it out there is what keeps the name unofferable by
    /// construction rather than by a filter each of those walks has to remember.
    ///
    /// `namespaces` is what [`Self::namespaces_of`] says the declaration sits under, and is counted
    /// only for a nameable one, for the reason the short name is.
    pub(crate) fn insert(
        &mut self,
        key: CatalogKey,
        short: Option<NameKey>,
        declarer: Declarer,
        namespaces: Vec<(NameKey, Name)>,
    ) {
        match key {
            CatalogKey::ClassLike(name) => self.class_like.entry(name).or_default(),
            CatalogKey::Function(name) => self.functions.entry(name).or_default(),
            CatalogKey::Constant(name) => self.constants.entry(name).or_default(),
        }
        .push(declarer);

        if let Some(short) = short {
            self.short_names
                .entry(short.clone())
                .or_insert_with(|| ShortName {
                    letters: phrost_model::letters_of(short.as_str()),
                    declarers: Vec::new(),
                })
                .declarers
                .push(declarer);
            for (path, spelled) in namespaces {
                let held = self.namespaces.entry(path).or_default();
                match held
                    .spellings
                    .binary_search_by(|(known, _)| known.as_str().cmp(spelled.as_str()))
                {
                    Ok(at) => held.spellings[at].1 += 1,
                    Err(at) => held.spellings.insert(at, (spelled, 1)),
                }
            }
        }
    }

    /// Drops one claim: the entry `file` made for `key`, and its short-name entry.
    ///
    /// Named claims rather than "everything this file declared" because the alternative is a scan
    /// of every table. That scan is what made bootstrap quadratic -- each of 29 000 files paying for
    /// the whole catalog built so far -- while looking like a per-file operation. The caller knows
    /// exactly which keys a file claimed, because it still holds the file's previous symbols.
    ///
    /// `claimed_by` is the file and the index of the symbol that made the claim, so what leaves is
    /// exactly what that symbol put in. Dropping every claim the file had on a name instead was
    /// wrong for the namespaces as soon as they were counted: a short name is shared by a class and
    /// a function, or by two classes in two namespaces of one file, and the first withdrawal took
    /// the second one's claims out of the first one's namespaces. The edit fuzz found it the moment
    /// the namespaces were in the dump.
    pub(crate) fn remove_claim(
        &mut self,
        key: &CatalogKey,
        short: Option<&NameKey>,
        claimed_by: (FileId, u32),
        namespaces: &[(NameKey, Name)],
    ) {
        let released = match key {
            CatalogKey::ClassLike(name) => Self::release(&mut self.class_like, name, claimed_by),
            CatalogKey::Function(name) => Self::release(&mut self.functions, name, claimed_by),
            CatalogKey::Constant(name) => Self::release(&mut self.constants, name, claimed_by),
        };
        let released = u32::from(released);

        if let Some(short) = short {
            if let Some(held) = self.short_names.get_mut(short) {
                held.declarers.retain(|declarer| !declarer.made(claimed_by));
                if held.declarers.is_empty() {
                    self.short_names.remove(short);
                }
            }
            for (path, spelled) in namespaces {
                let Some(held) = self.namespaces.get_mut(path) else {
                    continue;
                };
                if let Some(at) = held
                    .spellings
                    .iter()
                    .position(|(known, _)| known == spelled)
                {
                    held.spellings[at].1 = held.spellings[at].1.saturating_sub(released);
                    if held.spellings[at].1 == 0 {
                        held.spellings.remove(at);
                    }
                }
                if held.spellings.is_empty() {
                    self.namespaces.remove(path);
                }
            }
        }
    }

    /// The namespaces a declaration named `fqn` sits under, outermost first: folded, and as written.
    ///
    /// The one place they are derived, for the claim and for its withdrawal alike -- a namespace
    /// counted by one rule and released by another is a count that drifts.
    pub(crate) fn namespaces_of(
        interner: &mut phrost_model::Interner,
        fqn: &Name,
    ) -> Vec<(NameKey, Name)> {
        let written = fqn.as_str().trim_start_matches('\\');
        written
            .match_indices('\\')
            .map(|(at, _)| {
                let path = &written[..at];
                (interner.key(path), interner.name(path))
            })
            .collect()
    }

    /// Whether anything nameable sits under the namespace at this folded path.
    pub fn holds_namespace(&self, folded: &str) -> bool {
        self.namespaces.contains_key(folded)
    }

    /// The namespaces directly under `parent` whose own segment begins with `begins`, as a
    /// declaration under each spells it, in path order.
    ///
    /// Both folded. `parent` is a path without its trailing separator, or empty for the root.
    pub fn namespaces_under(&self, parent: &str, begins: &str) -> Vec<&Name> {
        let (start, segment_at) = match parent.is_empty() {
            true => (begins.to_owned(), 0),
            false => (format!("{parent}\\{begins}"), parent.len() + 1),
        };
        self.namespaces
            .range::<str, _>((Bound::Included(start.as_str()), Bound::Unbounded))
            .take_while(|(path, _)| path.as_str().starts_with(start.as_str()))
            .filter(|(path, _)| !path.as_str()[segment_at..].contains('\\'))
            .filter_map(|(_, held)| held.spelled())
            .collect()
    }

    /// Every namespace with its claims, for dumping: what an edit must keep exactly as a fresh
    /// build would have it.
    pub(crate) fn namespace_entries(&self) -> impl Iterator<Item = (&str, &str, u32)> {
        self.namespaces.iter().filter_map(|(path, held)| {
            Some((path.as_str(), held.spelled()?.as_str(), held.claims()))
        })
    }

    /// Declarers whose short name starts with `prefix`, in name order.
    ///
    /// `prefix` must already be folded. An empty prefix matches everything, which is what a
    /// completion request with nothing typed yet asks for.
    pub fn with_short_prefix<'a>(
        &'a self,
        prefix: &'a str,
    ) -> impl Iterator<Item = (&'a NameKey, &'a [Declarer])> {
        self.short_names
            .range::<str, _>((Bound::Included(prefix), Bound::Unbounded))
            .take_while(move |(name, _)| name.as_str().starts_with(prefix))
            .map(|(name, held)| (name, held.declarers.as_slice()))
    }

    /// Declarers whose short name is exactly `key`.
    ///
    /// The short-name index and not [`Self::class_like`], which is keyed by the *qualified* name --
    /// a reader holding a short name and asking that one gets an answer only for a class in the
    /// global namespace, which is the shape §8 calls two spellings of one thing.
    pub fn short_named(&self, key: &NameKey) -> &[Declarer] {
        self.short_names
            .get(key)
            .map_or(&[], |held| held.declarers.as_slice())
    }

    /// Declarers whose short name holds every letter of `letters`, in name order.
    ///
    /// The gate in front of a loose search. A name cannot hold a subsequence unless it holds each of
    /// its letters, so one `&` per name rejects most of a project before anything reads a string --
    /// and the mask was computed once, when the name was first claimed. See
    /// [`phrost_model::letters_of`].
    ///
    /// A superset test and not an equality: what comes back still has to be matched and scored.
    pub fn with_letters(&self, letters: u32) -> impl Iterator<Item = (&NameKey, &[Declarer])> {
        self.short_names
            .iter()
            .filter(move |(_, held)| held.letters & letters == letters)
            .map(|(name, held)| (name, held.declarers.as_slice()))
    }

    /// Every name a namespace could be recognised by: a class-like, a function, a constant.
    ///
    /// A namespace is declared by nothing of its own -- what says one exists is that something sits
    /// under it -- and a namespace holding only *functions* is ordinary in a library written that
    /// way: `use Psl\Crypto\Internal;` beside `Internal\call_sodium(...)`. Asking only about
    /// classes reported the import as an undefined type.
    pub fn declared_names(&self) -> impl Iterator<Item = &NameKey> + '_ {
        self.class_like.keys().chain(self.functions.keys())
    }

    /// Every claim on a class-like name, unranked.
    /// Every fully-qualified name in all three tables, with its claims.
    ///
    /// For a walk over qualified names, which is what a path being written asks: `\sprintf` and
    /// `use function App\render` name the function table, `\E_ALL` the constant one. A class-like
    /// or function name is the folded key; a constant's is as written, since PHP compares those
    /// exactly -- so a caller matching a prefix compares without case, and resolution, which goes
    /// through the exact tables, is unaffected.
    pub fn qualified_names(&self) -> impl Iterator<Item = (&str, &[Declarer])> + '_ {
        self.class_like
            .iter()
            .chain(self.functions.iter())
            .map(|(name, held)| (name.as_str(), held.as_slice()))
            .chain(
                self.constants
                    .iter()
                    .map(|(name, held)| (name.as_str(), held.as_slice())),
            )
    }

    pub fn class_like(&self, key: &NameKey) -> &[Declarer] {
        self.class_like.get(key).map_or(&[], Vec::as_slice)
    }

    pub fn function(&self, key: &NameKey) -> &[Declarer] {
        self.functions.get(key).map_or(&[], Vec::as_slice)
    }

    pub fn constant(&self, name: &Name) -> &[Declarer] {
        self.constants.get(name).map_or(&[], Vec::as_slice)
    }

    /// The same, for a reader that has the text and not an interned name.
    ///
    /// Exact, because PHP compares constant names exactly: `JSON_THROW_ON_ERROR` and
    /// `json_throw_on_error` are two constants and only one of them exists.
    pub fn constant_named(&self, name: &str) -> &[Declarer] {
        self.constants.get(name).map_or(&[], Vec::as_slice)
    }

    /// Every entry, for dumping. Sorted by key so two builds of the same project agree.
    pub(crate) fn entries(&self) -> Vec<(String, &'static str, &[Declarer])> {
        let mut entries = Vec::new();
        for (key, declarers) in &self.class_like {
            entries.push((key.as_str().to_owned(), "class", declarers.as_slice()));
        }
        for (key, declarers) in &self.functions {
            entries.push((key.as_str().to_owned(), "function", declarers.as_slice()));
        }
        for (key, declarers) in &self.constants {
            entries.push((key.as_str().to_owned(), "const", declarers.as_slice()));
        }
        entries.sort_by(|left, right| (&left.1, &left.0).cmp(&(&right.1, &right.0)));
        entries
    }

    /// Drops `file`'s claims on `key`, and says how many there were.
    /// Drops the claim one symbol made on `key`, and says whether there was one.
    fn release<K>(
        table: &mut FxHashMap<K, Vec<Declarer>>,
        key: &K,
        claimed_by: (FileId, u32),
    ) -> bool
    where
        K: std::hash::Hash + Eq + Clone,
    {
        let Some(declarers) = table.get_mut(key) else {
            return false;
        };
        let before = declarers.len();
        declarers.retain(|declarer| !declarer.made(claimed_by));
        let released = declarers.len() < before;
        if declarers.is_empty() {
            table.remove(key);
        }
        released
    }
}

/// The key a symbol is catalogued under, with the case rule of its table already applied.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum CatalogKey {
    ClassLike(NameKey),
    Function(NameKey),
    Constant(Name),
}

impl CatalogKey {
    /// Builds the key for one declared symbol, or `None` for a kind that cannot be declared at the
    /// top level of a file.
    pub(crate) fn for_symbol(
        interner: &mut phrost_model::Interner,
        kind: SymbolKind,
        fqn: &Name,
    ) -> Option<Self> {
        match kind {
            SymbolKind::Class | SymbolKind::Interface | SymbolKind::Trait | SymbolKind::Enum => {
                Some(Self::ClassLike(interner.key_of(fqn)))
            }
            SymbolKind::Function => Some(Self::Function(interner.key_of(fqn))),
            // Case-sensitive on purpose: PHP compares constant names exactly.
            SymbolKind::Constant => Some(Self::Constant(fqn.clone())),
            SymbolKind::Method
            | SymbolKind::Property
            | SymbolKind::ClassConstant
            | SymbolKind::EnumCase
            | SymbolKind::Namespace
            | SymbolKind::Variable
            | SymbolKind::Parameter
            | SymbolKind::Keyword
            // A file is not declared by a file.
            | SymbolKind::File
            // Not declared by any file, so no file owns one.
            | SymbolKind::CatalogKey => None,
        }
    }
}
