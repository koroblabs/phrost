use phrost_model::{BuiltinTy, Name, RelativeTy};
use rustc_hash::FxHashMap;

use crate::type_ref::TypeRef;

/// A resolved type, by identity.
///
/// Two ids are equal exactly when the types are structurally identical, because the table dedupes
/// on insert. That makes `a == b` a correct type comparison for the written form, and it is why
/// nothing outside this crate constructs one.
///
/// # Why the index is stored off by one
///
/// So that `Option<TypeId>` costs four bytes instead of eight. A bare `u32` has no value the
/// compiler can use for `None`, so every optional type in the index paid for a discriminant --
/// three of them on every parameter row alone (`ty`, `doc_ty`, `out_ty`), and a member row has more.
/// `NonZeroU32` gives the niche, and reserving zero is the whole of the cost.
///
/// The reservation is contained: three places in this file turn an index into an id or back, and
/// [`Self::as_raw`] keeps meaning *the index* so the disk format does not move. Measured on a
/// 30 916-file checkout, against a baseline taken with nothing else in flight.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct TypeId(std::num::NonZeroU32);

impl TypeId {
    /// The raw index, for writing it to a cache.
    ///
    /// The *index*, not the stored value: the off-by-one above is this type's business and a cache
    /// written before it must still read back.
    pub fn as_raw(self) -> u32 {
        self.0.get() - 1
    }

    /// Rebuilds one from [`Self::as_raw`]. Only meaningful against the table it came from.
    pub fn from_raw(raw: u32) -> Self {
        Self::at(raw as usize)
    }

    /// The id for a node at `index`.
    fn at(index: usize) -> Self {
        let stored = u32::try_from(index)
            .ok()
            .and_then(|index| index.checked_add(1))
            .and_then(std::num::NonZeroU32::new)
            .expect("fewer than 4G distinct types");
        Self(stored)
    }

    /// Where the node lives.
    fn index(self) -> usize {
        self.0.get() as usize - 1
    }
}

/// The operands of a union or intersection, as a range in the table's shared list storage.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TypeList {
    start: u32,
    len: u32,
}

impl TypeList {
    /// The offset and length this list occupies, for writing it to a cache.
    pub fn parts(self) -> (u32, u32) {
        (self.start, self.len)
    }

    /// Rebuilds one from what [`Self::parts`] gave.
    pub fn from_parts(start: u32, len: u32) -> Self {
        Self { start, len }
    }

    pub fn len(self) -> usize {
        self.len as usize
    }

    pub fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// Every distinct type written anywhere in the project, stored once.
///
/// One table per index rather than one per file: the dedup only pays if `string` written in ten
/// thousand files is one entry, and a `TypeId` is only meaningful relative to the table that
/// produced it, so having exactly one removes the question of which table an id belongs to.
///
/// Entries are never removed. Re-ingesting an edited file re-interns its types and almost always
/// finds them all already present, so the table converges on the project's distinct type vocabulary
/// rather than growing with edits — the same bargain the name interner makes.
#[derive(Debug, Default)]
pub struct TypeTable {
    nodes: Vec<TypeRef>,
    by_node: FxHashMap<TypeRef, TypeId>,
    /// Backing storage for every union and intersection operand list.
    list_items: Vec<TypeId>,
    by_list: FxHashMap<Box<[TypeId]>, TypeList>,
}

impl lspkit_memsize::MemSize for TypeTable {
    /// The nodes, the operand lists, and the two lookup maps derived from them.
    ///
    /// A `Named` node holds a `Name`, which is a pointer into the interner -- counted there, not
    /// here. What is here is the table's own storage, which is the point of interning: a project
    /// with hundreds of thousands of members writes a few thousand distinct types.
    fn heap_size(&self) -> usize {
        lspkit_memsize::vec_flat(&self.nodes)
            + lspkit_memsize::vec_flat(&self.list_items)
            + lspkit_memsize::table::<TypeRef, TypeId>(self.by_node.len())
            + lspkit_memsize::table::<Box<[TypeId]>, TypeList>(self.by_list.len())
            + self
                .by_list
                .keys()
                .map(|key| lspkit_memsize::flat(key))
                .sum::<usize>()
    }
}

impl TypeTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// The interned nodes, in id order.
    ///
    /// For writing the table to a cache. Order is the whole point: an id is an index, and a member
    /// row stored beside them holds ids -- so a table rebuilt in a different order would give every
    /// one of them a different meaning.
    pub fn nodes(&self) -> &[TypeRef] {
        &self.nodes
    }

    /// The backing storage every operand list points into.
    pub fn list_items(&self) -> &[TypeId] {
        &self.list_items
    }

    /// Rebuilds a table from what [`Self::nodes`] and [`Self::list_items`] gave.
    ///
    /// The lookup maps are derived rather than stored: they are the same information a second time,
    /// and one copy of a fact cannot disagree with itself. Ids survive because the nodes go back in
    /// the order they came out.
    pub fn from_parts(nodes: Vec<TypeRef>, list_items: Vec<TypeId>) -> Self {
        let mut by_node = FxHashMap::default();
        for (index, node) in nodes.iter().enumerate() {
            by_node.insert(node.clone(), TypeId::at(index));
        }

        let mut by_list = FxHashMap::default();
        for node in &nodes {
            let list = match node {
                TypeRef::Union(list)
                | TypeRef::Intersection(list)
                | TypeRef::Shape { fields: list, .. }
                | TypeRef::ObjectShape { fields: list } => *list,
                TypeRef::Generic { args, .. } => *args,
                _ => continue,
            };
            let start = list.start as usize;
            let Some(items) = list_items.get(start..start + list.len()) else {
                continue;
            };
            by_list.insert(items.to_vec().into_boxed_slice(), list);
        }

        Self {
            nodes,
            by_node,
            list_items,
            by_list,
        }
    }

    pub fn builtin(&mut self, builtin: BuiltinTy) -> TypeId {
        self.intern(TypeRef::Builtin(builtin))
    }

    pub fn relative(&mut self, relative: RelativeTy) -> TypeId {
        self.intern(TypeRef::Relative(relative))
    }

    /// Interns a class-like type. `fqn` is expected fully qualified and without a leading `\`.
    pub fn named(&mut self, fqn: Name) -> TypeId {
        self.intern(TypeRef::Named(fqn))
    }

    /// Interns `?T`. Already-nullable stays as it is, so a docblock and a native hint that agree
    /// about nullability cannot produce a different node from either one alone.
    pub fn nullable(&mut self, inner: TypeId) -> TypeId {
        if matches!(self.get(inner), TypeRef::Nullable(_)) {
            return inner;
        }
        self.intern(TypeRef::Nullable(inner))
    }

    /// Interns `base<args...>`. With no arguments it is the base: `array<>` is not a thing anyone
    /// writes, and a node with an empty list would make two spellings compare unequal.
    pub fn generic(&mut self, base: TypeId, args: &[TypeId]) -> TypeId {
        self.generic_of(base, args, false)
    }

    /// [`Self::generic`], for an array a declaration says holds an element.
    pub fn generic_of(&mut self, base: TypeId, args: &[TypeId], non_empty: bool) -> TypeId {
        if args.is_empty() {
            return base;
        }
        let args = self.intern_list(args);
        self.intern(TypeRef::Generic {
            base,
            args,
            non_empty,
        })
    }

    /// What iterating a value of this type yields, when the type says.
    ///
    /// The last argument, because that is where every shape puts it: `list<V>`, `array<K, V>`,
    /// `iterable<K, V>`. `None` for a type with no arguments -- an untyped `array` says nothing about
    /// what is in it, and guessing would type a loop variable from thin air.
    pub fn element_of(&self, id: TypeId) -> Option<TypeId> {
        match self.get(id) {
            TypeRef::Generic { args, .. } => self.operands(*args).last().copied(),
            TypeRef::Nullable(inner) => self.element_of(*inner),
            _ => None,
        }
    }

    /// Interns `A|B|...` flat, with `null` lifted out into a [`TypeRef::Nullable`] wrapper.
    ///
    /// Flat because a union of unions is not a shape any reader below here handles -- see
    /// [`Self::spread`], which is where all three degenerate forms are folded out.
    ///
    /// A one-operand union is that operand: PHP cannot express one, and a degenerate node would
    /// make two spellings of the same type compare unequal. `Foo|null` is the same argument one
    /// step further -- it is `?Foo` written the other way, and PHPDoc uses the union spelling far
    /// more often than the native one.
    ///
    /// Which is what makes this worth doing here rather than in each reader: every projection that
    /// already sees through `?T` -- member lookup, what iterating yields, the null-safety
    /// diagnostic -- sees through the union spelling as well, and none of them needed a second rule.
    /// Before this, a docblock `@return Foo|null` left member lookup on the result dead.
    ///
    /// Only `null` lifts out. `int|false` stays a union, because `false` is a value and not the
    /// absence of one.
    pub fn union(&mut self, operands: &[TypeId]) -> TypeId {
        let mut flat = Vec::with_capacity(operands.len());
        let mut nullable = false;
        for &operand in operands {
            self.spread(operand, &mut flat, &mut nullable);
        }
        if flat.is_empty() {
            // `null|null`, which nobody writes and a generated docblock can still produce. There is
            // nothing left for the null to wrap, so the union stands as it was given.
            return self.composite(operands, TypeRef::Union);
        }

        self.fold_bool(&mut flat);
        self.fold_strings(&mut flat);
        self.fold_refined_arguments(&mut flat);
        self.fold_non_empty(&mut flat);
        let inner = self.composite(&flat, TypeRef::Union);
        match nullable {
            true => self.nullable(inner),
            false => inner,
        }
    }

    /// `non-empty-list<K>|list<K>` is `list<K>`: the non-empty array is one of the arrays the other
    /// already admits. What a conditional answers when it cannot decide its test --
    /// `($array is non-empty-array ? non-empty-list<K> : list<K>)` -- and before the flag existed the
    /// two arms were one type and folded by interning.
    fn fold_non_empty(&self, flat: &mut Vec<TypeId>) {
        if !flat.iter().any(|&id| {
            matches!(
                self.get(id),
                TypeRef::Generic {
                    non_empty: true,
                    ..
                }
            )
        }) {
            return;
        }
        let admitted_elsewhere = |table: &Self, flat: &[TypeId], id: TypeId| {
            let TypeRef::Generic {
                base,
                args,
                non_empty: true,
            } = table.get(id)
            else {
                return false;
            };
            flat.iter().any(|&other| {
                matches!(
                    table.get(other),
                    TypeRef::Generic { base: b, args: a, non_empty: false } if b == base && a == args
                )
            })
        };
        let kept: Vec<TypeId> = flat
            .iter()
            .copied()
            .filter(|&id| !admitted_elsewhere(self, flat, id))
            .collect();
        *flat = kept;
    }

    /// `true|false` is `bool`, and so is `bool|true`.
    ///
    /// The fourth degenerate form, folded here as well as in the resolved vocabulary one layer up
    /// for the reason the other three are: two constructors of one concept is how one of them comes
    /// to be missing a rule the other has. A docblock writing `@return true|false` interns here and
    /// never reaches the other.
    ///
    /// The first literal is replaced in place rather than a `bool` appended, so an operand keeps the
    /// position the source wrote it in.
    fn fold_bool(&mut self, flat: &mut Vec<TypeId>) {
        let is = |table: &Self, id: TypeId, wanted: BuiltinTy| matches!(table.get(id), TypeRef::Builtin(kind) if *kind == wanted);
        let has_true = flat.iter().any(|&id| is(self, id, BuiltinTy::True));
        let has_false = flat.iter().any(|&id| is(self, id, BuiltinTy::False));
        let has_bool = flat.iter().any(|&id| is(self, id, BuiltinTy::Bool));
        if !((has_true && has_false) || (has_bool && (has_true || has_false))) {
            return;
        }

        let boolean = self.builtin(BuiltinTy::Bool);
        let mut stood_in = has_bool;
        flat.retain_mut(|id| {
            if !is(self, *id, BuiltinTy::True) && !is(self, *id, BuiltinTy::False) {
                return true;
            }
            if stood_in {
                return false;
            }
            stood_in = true;
            *id = boolean;
            true
        });
    }

    /// `non-empty-string|string` is `string`, and `non-falsy-string|non-empty-string` is the
    /// non-empty one: two strings are one string, holding what both are known to hold.
    ///
    /// The fold `true|false` already has, for the other refinement of a builtin, and kept beside it
    /// for that reason -- `Held::one_of` folds the same pair one layer up. Without it a conditional
    /// whose two branches are `non-empty-string` and `string` renders as both, and a reader taking
    /// the union apart judges the refined half on its own. In place of the first, as the boolean
    /// fold does.
    fn fold_strings(&mut self, flat: &mut Vec<TypeId>) {
        let string = |table: &Self, id: TypeId| match table.get(id) {
            TypeRef::Builtin(kind) if kind.is_string() => Some(*kind),
            _ => None,
        };
        let mut joined: Option<BuiltinTy> = None;
        let mut strings = 0;
        for &id in flat.iter() {
            if let Some(kind) = string(self, id) {
                strings += 1;
                joined = Some(joined.map_or(kind, |seen| seen.string_join(kind).unwrap_or(seen)));
            }
        }
        let Some(joined) = joined.filter(|_| strings > 1) else {
            return;
        };
        let joined = self.builtin(joined);
        let mut stood_in = false;
        flat.retain_mut(|id| {
            if string(self, *id).is_none() {
                return true;
            }
            if stood_in {
                return false;
            }
            stood_in = true;
            *id = joined;
            true
        });
    }

    /// `list<non-empty-string>|list<string>` is `list<string>`: two containers of one base whose
    /// arguments differ only in what a string holds are one container, holding the weaker.
    ///
    /// What a conditional answers when it cannot decide its test -- `preg_split()`'s branches differ
    /// in nothing else -- and before strings could hold something the two were one type and folded by
    /// interning. A union of the two said the same thing twice, and rendered it twice.
    fn fold_refined_arguments(&mut self, flat: &mut Vec<TypeId>) {
        let generics = flat
            .iter()
            .filter(|&&id| matches!(self.get(id), TypeRef::Generic { .. }))
            .count();
        if generics < 2 {
            return;
        }
        let mut at = 0;
        while at < flat.len() {
            let mut other = at + 1;
            while other < flat.len() {
                match self.joined_containers(flat[at], flat[other]) {
                    Some(joined) => {
                        flat[at] = joined;
                        flat.remove(other);
                    }
                    None => other += 1,
                }
            }
            at += 1;
        }
    }

    /// Two containers of one base as one, where every argument they disagree about is a string.
    fn joined_containers(&mut self, one: TypeId, other: TypeId) -> Option<TypeId> {
        let (
            TypeRef::Generic {
                base,
                args,
                non_empty,
            },
            TypeRef::Generic {
                base: other_base,
                args: other_args,
                non_empty: other_non_empty,
            },
        ) = (self.get(one).clone(), self.get(other).clone())
        else {
            return None;
        };
        if base != other_base || args.len() != other_args.len() {
            return None;
        }
        let pairs = self
            .operands(args)
            .iter()
            .copied()
            .zip(self.operands(other_args).iter().copied())
            .collect::<Vec<_>>();
        let mut joined = Vec::with_capacity(pairs.len());
        for (left, right) in pairs {
            if left == right {
                joined.push(left);
                continue;
            }
            let (TypeRef::Builtin(left), TypeRef::Builtin(right)) =
                (self.get(left), self.get(right))
            else {
                return None;
            };
            let string = left.string_join(*right)?;
            joined.push(self.builtin(string));
        }
        Some(self.generic_of(base, &joined, non_empty && other_non_empty))
    }

    /// One operand's contribution to a union, with the degenerate forms taken apart.
    ///
    /// Both grammars that write a `|` are binary, so `string|int|null` arrives as
    /// `string|(int|null)` -- the same type, and not the same node. Left nested, the inner union is
    /// an operand that answers nothing about its own class or kind, so every reader that takes a
    /// union apart sees two operands where there are three and reduces the second to whatever its
    /// halves happen to share. It also renders as `string|?int`, which PHP has no way to write and
    /// no reader can act on.
    ///
    /// So a nested union is spliced in, a `?T` operand contributes its `T` and the null, and a
    /// repeat is dropped -- the same three degenerate forms `Held::one_of` folds out one layer up.
    /// Recursive rather than one pass because each unwrapping can uncover another: `?(A|B)` is one
    /// of each.
    fn spread(&self, operand: TypeId, out: &mut Vec<TypeId>, nullable: &mut bool) {
        match self.get(operand) {
            TypeRef::Builtin(BuiltinTy::Null) => *nullable = true,
            TypeRef::Nullable(inner) => {
                *nullable = true;
                self.spread(*inner, out, nullable);
            }
            TypeRef::Union(parts) => {
                for &part in self.operands(*parts) {
                    self.spread(part, out, nullable);
                }
            }
            _ if out.contains(&operand) => {}
            _ => out.push(operand),
        }
    }

    /// Interns `A&B&...`, with the same one-operand rule as [`Self::union`].
    pub fn intersection(&mut self, operands: &[TypeId]) -> TypeId {
        self.composite(operands, TypeRef::Intersection)
    }

    /// Interns one field of a shape.
    ///
    /// Its own node, so a shape's fields go through the operand-list machinery every other composite
    /// uses. It is not a type anything else should hold.
    pub fn field(&mut self, key: Name, optional: bool, ty: TypeId) -> TypeId {
        self.intern(TypeRef::Field { key, optional, ty })
    }

    /// Interns `array{...}` from fields made by [`Self::field`]; `open` when the array may hold keys
    /// the fields do not name.
    ///
    /// An *open* shape with no fields is just `array`: there is nothing it says that the word does
    /// not. A sealed one is `array{}`, which says the array is empty.
    pub fn shape(&mut self, fields: &[TypeId], open: bool) -> TypeId {
        self.shape_of(fields, open, false)
    }

    /// The same, for `list{...}` as well: `list` when the keys are `0` upwards in order.
    pub fn shape_of(&mut self, fields: &[TypeId], open: bool, list: bool) -> TypeId {
        if open && fields.is_empty() {
            return self.builtin(BuiltinTy::Array);
        }
        // An empty shape is kept as one, and that is a rule this constructor had the other way
        // round while its twin in lowering did too -- two constructors of one collapse. `array{}`
        // is a statement that there is nothing in the array, which is what makes
        // `array{...}|array{}` readable at a key: PHP answers null on the empty arm. Interned as a
        // bare `array` it said "an array of something unstated" instead, and the arm then vetoed
        // the whole union's key.
        //
        // Not `composite`, whose one-operand rule collapses a single member to itself -- right for a
        // union and wrong here, where `array{id: int}` is a shape with one field and not that field.
        let fields = self.intern_list(fields);
        self.intern(TypeRef::Shape { fields, open, list })
    }

    /// An object whose named properties are known, from its [`TypeRef::Field`]s.
    pub fn object_shape(&mut self, fields: &[TypeId]) -> TypeId {
        let fields = self.intern_list(fields);
        self.intern(TypeRef::ObjectShape { fields })
    }

    /// The type an object shape's property holds, and whether it may be absent.
    pub fn object_shape_field(&self, id: TypeId, name: &str) -> Option<(TypeId, bool)> {
        let TypeRef::ObjectShape { fields } = self.get(id) else {
            return None;
        };
        self.operands(*fields)
            .iter()
            .find_map(|field| match self.get(*field) {
                // A property name is case-sensitive, as a key is.
                TypeRef::Field { key, optional, ty } if key.as_str() == name => {
                    Some((*ty, *optional))
                }
                _ => None,
            })
    }

    /// The type a shape's key holds, and whether the key may be absent.
    ///
    /// `None` when the type is not a shape or has no such key -- which is the honest answer for
    /// `$row['typo']` and the thing a report could be built on later.
    pub fn shape_field(&self, id: TypeId, key: &str) -> Option<(TypeId, bool)> {
        let TypeRef::Shape { fields, .. } = self.get(id) else {
            return None;
        };
        self.operands(*fields).iter().find_map(|field| {
            match self.get(*field) {
                // Keys are compared exactly: a PHP array key is case-sensitive, and `$row['ID']` is
                // a different key from `$row['id']`.
                TypeRef::Field {
                    key: named,
                    optional,
                    ty,
                } if named.as_str() == key => Some((*ty, *optional)),
                _ => None,
            }
        })
    }

    /// Whether a shape may hold keys it does not name: written with `...`, or missing a key it was
    /// written with. `false` for anything that is not a shape.
    pub fn shape_is_open(&self, id: TypeId) -> bool {
        matches!(self.get(id), TypeRef::Shape { open: true, .. })
    }

    /// Whether a shape was written `list{...}`. `false` for anything that is not a shape.
    pub fn shape_is_list(&self, id: TypeId) -> bool {
        matches!(self.get(id), TypeRef::Shape { list: true, .. })
    }

    /// Every key a shape names, in the order written.
    pub fn shape_keys(&self, id: TypeId) -> Vec<(&Name, TypeId, bool)> {
        let TypeRef::Shape { fields, .. } = self.get(id) else {
            return Vec::new();
        };
        self.operands(*fields)
            .iter()
            .filter_map(|field| match self.get(*field) {
                TypeRef::Field { key, optional, ty } => Some((key, *ty, *optional)),
                _ => None,
            })
            .collect()
    }

    pub fn get(&self, id: TypeId) -> &TypeRef {
        &self.nodes[id.index()]
    }

    pub fn operands(&self, list: TypeList) -> &[TypeId] {
        let start = list.start as usize;
        &self.list_items[start..start + list.len()]
    }

    /// How many distinct types the project has written. Reported by the memory tooling, because
    /// this number staying small is the reason the table is affordable.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    fn composite(&mut self, operands: &[TypeId], wrap: fn(TypeList) -> TypeRef) -> TypeId {
        if let [single] = operands {
            return *single;
        }
        let list = self.intern_list(operands);
        self.intern(wrap(list))
    }

    fn intern(&mut self, node: TypeRef) -> TypeId {
        if let Some(&id) = self.by_node.get(&node) {
            return id;
        }
        let id = TypeId::at(self.nodes.len());
        self.nodes.push(node.clone());
        self.by_node.insert(node, id);
        id
    }

    fn intern_list(&mut self, operands: &[TypeId]) -> TypeList {
        if let Some(&list) = self.by_list.get(operands) {
            return list;
        }
        let list = TypeList {
            start: u32::try_from(self.list_items.len()).expect("fewer than 4G type operands"),
            len: u32::try_from(operands.len()).expect("fewer than 4G operands in one type"),
        };
        self.list_items.extend_from_slice(operands);
        self.by_list.insert(operands.into(), list);
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_type_written_twice_is_one_entry() {
        let mut types = TypeTable::new();
        let first = types.builtin(BuiltinTy::String);
        let second = types.builtin(BuiltinTy::String);

        assert_eq!(first, second);
        assert_eq!(types.len(), 1);
    }

    #[test]
    fn a_composite_dedupes_by_its_operands() {
        // The point of interning: `int|string` written in two files is one entry and one id, so
        // comparing two types never walks a tree.
        let mut types = TypeTable::new();
        let int = types.builtin(BuiltinTy::Int);
        let string = types.builtin(BuiltinTy::String);

        let first = types.union(&[int, string]);
        let second = types.union(&[int, string]);

        assert_eq!(first, second);
        assert_eq!(types.len(), 3);
    }

    #[test]
    fn a_union_with_null_is_the_nullable_of_the_rest() {
        // `Foo|null` and `?Foo` are one type written two ways, and PHPDoc writes it the first way.
        // One representation is what lets every reader that already sees through `?T` see through
        // this too, instead of each of them needing a second rule.
        let mut interner = phrost_model::Interner::default();
        let mut types = TypeTable::new();
        let foo = types.named(interner.name("Foo"));
        let null = types.builtin(BuiltinTy::Null);

        assert_eq!(types.union(&[foo, null]), types.nullable(foo));
        assert_eq!(types.union(&[null, foo]), types.nullable(foo));
    }

    #[test]
    fn a_union_of_several_with_null_keeps_the_union_inside() {
        let mut interner = phrost_model::Interner::default();
        let mut types = TypeTable::new();
        let foo = types.named(interner.name("Foo"));
        let bar = types.named(interner.name("Bar"));
        let null = types.builtin(BuiltinTy::Null);

        let lifted = types.union(&[foo, bar, null]);
        let expected = types.union(&[foo, bar]);
        assert_eq!(lifted, types.nullable(expected));
    }

    /// Both grammars that write a `|` are binary, so `string|int|null` arrives as
    /// `string|(int|null)` -- three operands wearing the shape of two, and rendering as
    /// `string|?int`, which PHP has no way to write.
    #[test]
    fn a_union_of_unions_is_one_flat_union() {
        let mut types = TypeTable::new();
        let string = types.builtin(BuiltinTy::String);
        let int = types.builtin(BuiltinTy::Int);
        let null = types.builtin(BuiltinTy::Null);

        let nested = types.union(&[int, null]);
        let whole = types.union(&[string, nested]);

        let flat = types.union(&[string, int]);
        assert_eq!(whole, types.nullable(flat));
        assert_eq!(types.render(whole), "string|int|null");
    }

    /// The same rule with no null in it, because the nesting is the defect and the null only made
    /// it visible.
    #[test]
    fn a_nested_union_keeps_its_operands_in_source_order() {
        let mut types = TypeTable::new();
        let string = types.builtin(BuiltinTy::String);
        let int = types.builtin(BuiltinTy::Int);
        let no = types.builtin(BuiltinTy::False);

        let nested = types.union(&[int, no]);
        let whole = types.union(&[string, nested]);

        assert_eq!(whole, types.union(&[string, int, no]));
        assert_eq!(types.render(whole), "string|int|false");
    }

    /// An operand that is already `?T` says the same thing a `null` operand does, and saying it
    /// twice is how `string|?int` was reached.
    #[test]
    fn a_nullable_operand_lifts_its_null_out_like_a_bare_null() {
        let mut interner = phrost_model::Interner::default();
        let mut types = TypeTable::new();
        let foo = types.named(interner.name("Foo"));
        let bar = types.named(interner.name("Bar"));
        let maybe_bar = types.nullable(bar);

        let lifted = types.union(&[foo, maybe_bar]);
        let both = types.union(&[foo, bar]);
        assert_eq!(lifted, types.nullable(both));
    }

    #[test]
    fn a_repeated_operand_appears_once() {
        // `A|(A|B)` is `A|B`, and a duplicate operand would make two spellings of one type compare
        // unequal -- the same reason the one-operand and null cases fold out.
        let mut types = TypeTable::new();
        let int = types.builtin(BuiltinTy::Int);
        let string = types.builtin(BuiltinTy::String);

        let inner = types.union(&[int, string]);
        assert_eq!(types.union(&[int, inner]), inner);
        assert_eq!(types.union(&[int, int]), int);
    }

    #[test]
    fn only_null_lifts_out_of_a_union() {
        // `false` is a value, not the absence of one, and `int|false` is what half the standard
        // library returns.
        let mut types = TypeTable::new();
        let int = types.builtin(BuiltinTy::Int);
        let no = types.builtin(BuiltinTy::False);

        let union = types.union(&[int, no]);
        assert!(matches!(types.get(union), TypeRef::Union(_)));
    }

    #[test]
    fn nullable_does_not_stack() {
        // A native `?Foo` and a docblock `Foo|null` describing the same member must not produce two
        // different ids for the same type.
        let mut interner = phrost_model::Interner::default();
        let mut types = TypeTable::new();
        let foo = types.named(interner.name("Foo"));
        let once = types.nullable(foo);

        assert_eq!(types.nullable(once), once);
    }

    #[test]
    fn operand_order_is_part_of_the_type() {
        // `int|string` and `string|int` mean the same thing but do not *read* the same, and the
        // written form is what hover shows. Normalizing the order belongs to inference, above here.
        let mut types = TypeTable::new();
        let int = types.builtin(BuiltinTy::Int);
        let string = types.builtin(BuiltinTy::String);

        assert_ne!(types.union(&[int, string]), types.union(&[string, int]));
    }

    #[test]
    fn a_one_operand_composite_collapses_to_that_operand() {
        let mut types = TypeTable::new();
        let int = types.builtin(BuiltinTy::Int);

        assert_eq!(types.union(&[int]), int);
        assert_eq!(types.len(), 1);
    }

    #[test]
    fn a_union_and_an_intersection_of_the_same_operands_differ() {
        // `(A&B)` and `(A|B)` share their operand list, so a table that keyed only on the list
        // would return the same id for both.
        let mut interner = phrost_model::Interner::new();
        let mut types = TypeTable::new();
        let a = types.named(interner.name("A"));
        let b = types.named(interner.name("B"));

        assert_ne!(types.union(&[a, b]), types.intersection(&[a, b]));
    }
}
