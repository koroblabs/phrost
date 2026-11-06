//! Which of a native declaration and a docblock's claim about the same thing is read.
//!
//! Its own module because it is its own question: the table above stores what was written, and this
//! decides between two things that were. The written-type form of the same rule is
//! `phrost_hir::refined_type_ref`; the two representations cannot share code, so they share a name
//! and the comments.

use phrost_model::BuiltinTy;

use crate::{TypeId, TypeTable, type_ref::TypeRef};

/// One operand of a union, with the `null` a nullable holds counted as one of them.
#[derive(Clone, Copy)]
enum Operand {
    Type(TypeId),
    Null,
}

impl TypeTable {
    /// The more informative of a native declaration and a docblock's claim about the same thing.
    ///
    /// A native hint is what the runtime enforces, so it normally wins outright. The exception is
    /// the one PHP cannot express: `array` and `iterable` say nothing about what is inside, and
    /// `@param list<Repo>` beside `array $items` is not a competing claim but the missing half of the
    /// same one. Taking the native type there discards the element type, which is what a `foreach`
    /// needs and the most common thing anyone documents.
    ///
    /// Deliberately narrow. Only a bare container is refined, so a docblock that disagrees with a
    /// native class cannot override it -- the risk of trusting a stale comment over a declaration the
    /// runtime enforces is exactly why the general rule is the other way round.
    ///
    /// The written-type form of this rule lives in
    /// [`phrost_hir::refined_type_ref`](../../phrost_hir/fn.refined_type_ref.html); the two
    /// representations cannot share code, so they share a name and this comment.
    pub fn refined(&self, native: TypeId, doc: TypeId) -> TypeId {
        if self.is_bare_container(native) && self.has_arguments(doc) {
            return doc;
        }
        // `object` and `mixed` are what an author writes when PHP has no syntax for what they mean.
        // `@return T` beside `: object` is the whole of what a factory declares, and ranking the
        // hint first there answers `object` for every factory in the language.
        if self.says_nothing_specific(native, doc) {
            return doc;
        }
        // The same claim with the arguments added: `@param Items<Special>` beside `Items $items` is
        // not a comment disagreeing with a declaration, it is the half PHP has no syntax for.
        // Required to be the *same* base, so a docblock naming a different class still loses.
        if self.argues_about(native, doc) {
            return doc;
        }
        if self.says_what_calling_it_gives(native, doc) {
            return doc;
        }
        if self.narrows_the_builtin(native, doc) {
            return doc;
        }
        // The same relationship operand by operand: `array|false $lines` beside `@param
        // list<string>|false $lines`. Iterating such a parameter typed nothing while its nullable
        // spelling typed the element.
        if self.refines_each_operand(native, doc) {
            return doc;
        }
        native
    }

    /// Whether a docblock is a native union with each bare container in it given arguments, and
    /// every other operand restated.
    ///
    /// Both directions are required: an operand the docblock drops is one the runtime still admits,
    /// and one it adds is one the runtime refuses. Either way the docblock is a competing claim and
    /// the hint wins, as it does for a lone type.
    fn refines_each_operand(&self, native: TypeId, doc: TypeId) -> bool {
        if !matches!(self.get(native), TypeRef::Union(_) | TypeRef::Nullable(_)) {
            return false;
        }
        let (mut natives, mut docs) = (Vec::new(), Vec::new());
        self.operands_of(native, &mut natives);
        self.operands_of(doc, &mut docs);
        let alike = |native: Operand, doc: Operand| match (native, doc) {
            (Operand::Null, Operand::Null) => true,
            (Operand::Null, Operand::Type(id)) | (Operand::Type(id), Operand::Null) => {
                matches!(self.get(id), TypeRef::Builtin(BuiltinTy::Null))
            }
            (Operand::Type(native), Operand::Type(doc)) => native == doc,
        };
        let refines = |native: Operand, doc: Operand| match (native, doc) {
            (Operand::Type(native), Operand::Type(doc)) => {
                (self.is_bare_container(native) && self.has_arguments(doc))
                    || self.argues_about(native, doc)
                    || self.narrows_the_builtin(native, doc)
                    || (matches!(self.get(native), TypeRef::Builtin(BuiltinTy::Object))
                        && self.says_which_object(doc))
            }
            _ => false,
        };
        let mut refined = false;
        for &doc in &docs {
            if natives.iter().any(|&native| alike(native, doc)) {
                continue;
            }
            if !natives.iter().any(|&native| refines(native, doc)) {
                return false;
            }
            refined = true;
        }
        refined
            && natives.iter().all(|&native| {
                docs.iter()
                    .any(|&doc| alike(native, doc) || refines(native, doc))
            })
    }

    /// A type's operands, flattened through unions and the `?` of a nullable.
    fn operands_of(&self, id: TypeId, out: &mut Vec<Operand>) {
        match self.get(id) {
            TypeRef::Union(parts) => {
                for &part in self.operands(*parts) {
                    self.operands_of(part, out);
                }
            }
            TypeRef::Nullable(inner) => {
                self.operands_of(*inner, out);
                out.push(Operand::Null);
            }
            _ => out.push(Operand::Type(id)),
        }
    }

    /// Whether the docblock is the hint's builtin with what it holds narrowed: `@param
    /// non-empty-string $name` beside `string $name`.
    ///
    /// The half PHP has no syntax for, which is this rule's whole ground -- the refinement is every
    /// value the hint admits but one, and a reader keeping the hint threw the claim away. That is
    /// also what makes the docblock the *contract*: a caller passing a plain `string` is reported,
    /// the way PHPStan reads the same pair.
    fn narrows_the_builtin(&self, native: TypeId, doc: TypeId) -> bool {
        match (self.get(native), self.get(doc)) {
            (TypeRef::Builtin(native), TypeRef::Builtin(doc)) => native
                .string_rank()
                .zip(doc.string_rank())
                .is_some_and(|(native, doc)| doc > native),
            _ => false,
        }
    }

    /// Whether the hint is `\Closure` and the docblock says what invoking it gives back.
    ///
    /// `: callable` beside `@param callable(Item): string` already works, because the docblock's base
    /// *is* the hint and [`Self::argues_about`] recognises that. `: \Closure` is the same claim with
    /// PHP's class spelling of a callable, and the base then differs -- so the one shape people
    /// actually write in modern code was the one that lost.
    ///
    /// What is given up is `Closure`'s own three methods: `$fn->bindTo()` stops resolving. Worth it
    /// and measured to be safe rather than assumed -- the result names no class, so an access on it
    /// is an unknown receiver and reports nothing. Three rarely-used methods become silent so that
    /// invoking the parameter, which is the entire point of taking one, answers at all.
    fn says_what_calling_it_gives(&self, native: TypeId, doc: TypeId) -> bool {
        let names_closure = match self.get(native) {
            TypeRef::Named(fqn) => fqn.as_str().eq_ignore_ascii_case("Closure"),
            _ => false,
        };
        names_closure
            && matches!(
                self.get(doc),
                TypeRef::Generic { base, args, .. }
                    if matches!(self.get(*base), TypeRef::Builtin(BuiltinTy::Callable))
                        && !self.operands(*args).is_empty()
            )
    }

    /// Whether the native hint is one PHP offers for "some value" and the docblock says which one.
    ///
    /// `mixed` admits anything, so any docblock narrows it. `object` admits any instance, so a
    /// docblock narrows it only by naming a class -- a `@return array` beside `: object` is a mistake
    /// in the source, and there the hint is what the runtime enforces.
    ///
    /// A nullable is deliberately not looked through. `?object` beside `@return T` would answer `T`
    /// and lose the null, which is the one fact a reader opens a hover card to find.
    fn says_nothing_specific(&self, native: TypeId, doc: TypeId) -> bool {
        match self.get(native) {
            // Unless it names a class beside the `mixed`: `@return mixed|SettingService` on
            // BookStack's `setting(): mixed` is what calling it with no key gives, and the class is
            // what a reader navigating wants. A judgement still reads the union as the `mixed` it
            // holds -- the sentinel, admission and compatibility readers all fold it -- so taking
            // the docblock claims nothing a check can see.
            TypeRef::Builtin(BuiltinTy::Mixed) => {
                !self.mentions_mixed(doc) || self.has_a_class_operand(doc)
            }
            TypeRef::Builtin(BuiltinTy::Object) => self.says_which_object(doc),
            _ => false,
        }
    }

    /// Whether a docblock says which instances an `object` hint admits: a class, several, or the
    /// properties of an `object{cid: string}` -- each the half PHP has no syntax for.
    ///
    /// Asked of a lone hint and of each operand of a union alike. Asked of the lone one only, it
    /// read `: FALSE|object` beside `@return FALSE|CachedItem` as the hint, and every property of
    /// the cache item a project declared as an alias for exactly that reason was unknown.
    fn says_which_object(&self, doc: TypeId) -> bool {
        self.names_a_class(doc) || matches!(self.get(doc), TypeRef::ObjectShape { .. })
    }

    /// Whether some operand of a union, or the type itself, is an instance of something named.
    fn has_a_class_operand(&self, id: TypeId) -> bool {
        match self.get(id) {
            TypeRef::Union(parts) => self
                .operands(*parts)
                .iter()
                .any(|part| self.has_a_class_operand(*part)),
            TypeRef::Nullable(inner) => self.has_a_class_operand(*inner),
            _ => self.names_a_class(id),
        }
    }

    /// Whether `mixed` appears anywhere in a union or behind a `?`.
    ///
    /// `@return array|mixed|null` is `mixed` written out longhand. A docblock beside a hint says one
    /// of three things and only one of them is noise -- this is that one, and taking it would replace
    /// a `mixed` nobody complains about with a nullable that reports every argument the value is
    /// passed on to.
    fn mentions_mixed(&self, id: TypeId) -> bool {
        match self.get(id) {
            TypeRef::Builtin(BuiltinTy::Mixed) => true,
            TypeRef::Nullable(inner) => self.mentions_mixed(*inner),
            TypeRef::Union(parts) | TypeRef::Intersection(parts) => self
                .operands(*parts)
                .iter()
                .any(|part| self.mentions_mixed(*part)),
            _ => false,
        }
    }

    /// Whether a type is an instance of something named, however it is wrapped.
    fn names_a_class(&self, id: TypeId) -> bool {
        match self.get(id) {
            TypeRef::Named(_) | TypeRef::Relative(_) => true,
            TypeRef::Generic { base, .. } => self.names_a_class(*base),
            TypeRef::Nullable(inner) => self.names_a_class(*inner),
            // Every operand, and a union of classes is a class the hint could not spell just as
            // much as one class is. Without this, `: object` beside `@return Foo|Bar` kept the hint
            // and answered `object` -- while the same declaration with one class in the docblock
            // answered `Foo`. A rule that holds for one operand and not for two is the shape a
            // reader finds by writing the second one.
            TypeRef::Union(parts) => {
                let operands = self.operands(*parts);
                !operands.is_empty() && operands.iter().all(|&part| self.names_a_class(part))
            }
            _ => false,
        }
    }

    /// Whether `doc` is `native` with type arguments supplied.
    fn argues_about(&self, native: TypeId, doc: TypeId) -> bool {
        match (self.get(native), self.get(doc)) {
            (_, TypeRef::Generic { base, .. }) => *base == native,
            (TypeRef::Nullable(native), _) => self.argues_about(*native, doc),
            (_, TypeRef::Nullable(doc)) => self.argues_about(native, *doc),
            _ => false,
        }
    }

    /// `array` or `iterable`, with no arguments, `?` and all.
    fn is_bare_container(&self, id: TypeId) -> bool {
        match self.get(id) {
            TypeRef::Builtin(BuiltinTy::Array | BuiltinTy::Iterable) => true,
            TypeRef::Nullable(inner) => self.is_bare_container(*inner),
            _ => false,
        }
    }

    /// Whether the type says what it contains.
    fn has_arguments(&self, id: TypeId) -> bool {
        match self.get(id) {
            // A shape counts. `@return array{app: Repo}` beside a native `: array` is the same
            // relationship as `@return list<Repo>` beside one -- the docblock supplying what PHP has
            // no syntax for -- and reading it as a competing claim let the bare `array` win and threw
            // every key away.
            TypeRef::Generic { .. } | TypeRef::Shape { .. } => true,
            TypeRef::Nullable(inner) => self.has_arguments(*inner),
            // And a union of them, which is the spelling nobody added after the shape was. `@return
            // array{type: string, ...}|array{}` beside a native `: array` is one claim written as
            // two branches -- "the whole thing, or nothing" -- and losing it to the bare hint threw
            // away every key exactly as reading a lone shape as a competing claim once did. 29 of
            // the 38 `Offset 'x' might not exist` findings PHPStan reports on the reference project
            // are that union, written by hand in the project's own `@return`.
            //
            // Every operand, sentinels aside: one that says nothing leaves the union saying nothing
            // about the value on that branch, and `array|Foo` beside `: array` really is a
            // competing claim rather than the missing half of one.
            TypeRef::Union(operands) => {
                let operands = self.operands(*operands);
                let mut said_something = false;
                for &operand in operands {
                    if matches!(
                        self.get(operand),
                        TypeRef::Builtin(BuiltinTy::Null | BuiltinTy::False)
                    ) {
                        continue;
                    }
                    if !self.has_arguments(operand) {
                        return false;
                    }
                    said_something = true;
                }
                said_something
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use phrost_model::{BuiltinTy, Interner};

    use crate::TypeTable;

    #[test]
    fn a_docblock_gives_a_unions_container_its_element() {
        // `array|false $lines` beside `@param array<int, Row>|false $lines`: the nullable spelling of
        // the same declaration was refined, and this one kept the bare `array`.
        let mut interner = Interner::default();
        let mut types = TypeTable::new();
        let (array, false_, int) = (
            types.builtin(BuiltinTy::Array),
            types.builtin(BuiltinTy::False),
            types.builtin(BuiltinTy::Int),
        );
        let row = types.named(interner.name("Row"));
        let rows = types.generic(array, &[int, row]);
        let native = types.union(&[array, false_]);
        let doc = types.union(&[rows, false_]);

        assert_eq!(types.refined(native, doc), doc);
    }

    #[test]
    fn the_null_of_a_nullable_union_is_one_of_its_operands() {
        // `array|string|null` beside `@param Row[]|string|null`, which interns as a `?` around the
        // rest on both sides.
        let mut interner = Interner::default();
        let mut types = TypeTable::new();
        let (array, string, null) = (
            types.builtin(BuiltinTy::Array),
            types.builtin(BuiltinTy::String),
            types.builtin(BuiltinTy::Null),
        );
        let row = types.named(interner.name("Row"));
        let rows = types.generic(array, &[row]);
        let native = types.union(&[array, string, null]);
        let doc = types.union(&[rows, string, null]);

        assert_eq!(types.refined(native, doc), doc);
    }

    #[test]
    fn a_class_operand_is_given_its_arguments_too() {
        let mut interner = Interner::default();
        let mut types = TypeTable::new();
        let false_ = types.builtin(BuiltinTy::False);
        let row = types.named(interner.name("Row"));
        let collection = types.named(interner.name("Collection"));
        let rows = types.generic(collection, &[row]);
        let native = types.union(&[collection, false_]);
        let doc = types.union(&[rows, false_]);

        assert_eq!(types.refined(native, doc), doc);
    }

    #[test]
    fn a_docblock_that_drops_an_operand_the_runtime_admits_loses() {
        // `@param array<int, Row>` beside `array|false`: the `false` still gets through, and taking
        // the docblock would read every test for it as dead.
        let mut interner = Interner::default();
        let mut types = TypeTable::new();
        let (array, false_, int) = (
            types.builtin(BuiltinTy::Array),
            types.builtin(BuiltinTy::False),
            types.builtin(BuiltinTy::Int),
        );
        let row = types.named(interner.name("Row"));
        let rows = types.generic(array, &[int, row]);
        let native = types.union(&[array, false_]);

        assert_eq!(types.refined(native, rows), native);
    }

    #[test]
    fn a_docblock_that_adds_an_operand_the_runtime_refuses_loses() {
        let mut interner = Interner::default();
        let mut types = TypeTable::new();
        let (array, false_, null) = (
            types.builtin(BuiltinTy::Array),
            types.builtin(BuiltinTy::False),
            types.builtin(BuiltinTy::Null),
        );
        let row = types.named(interner.name("Row"));
        let rows = types.generic(array, &[row]);
        let native = types.union(&[array, false_]);
        let doc = types.union(&[rows, false_, null]);

        assert_eq!(types.refined(native, doc), native);
    }

    #[test]
    fn a_docblock_saying_what_a_string_holds_is_the_contract() {
        // `@param non-empty-string $name` beside `string $name`: every value the hint admits but
        // one, and the one PHP cannot spell. Operand by operand inside a nullable as well, and never
        // the other way round -- a docblock saying less than the hint is not read.
        let mut types = TypeTable::new();
        let (string, non_empty, null) = (
            types.builtin(BuiltinTy::String),
            types.builtin(BuiltinTy::NonEmptyString),
            types.builtin(BuiltinTy::Null),
        );
        let maybe = types.union(&[string, null]);
        let maybe_non_empty = types.union(&[non_empty, null]);

        assert_eq!(types.refined(string, non_empty), non_empty);
        assert_eq!(types.refined(maybe, maybe_non_empty), maybe_non_empty);
        assert_eq!(types.refined(non_empty, string), non_empty);
    }

    #[test]
    fn an_object_shape_names_what_a_bare_object_does_not() {
        // `@return object{cid: string}` beside `: object`, and its twin in lowering's written rule.
        let mut interner = Interner::default();
        let mut types = TypeTable::new();
        let (object, string) = (
            types.builtin(BuiltinTy::Object),
            types.builtin(BuiltinTy::String),
        );
        let cid = types.field(interner.name("cid"), false, string);
        let doc = types.object_shape(&[cid]);

        assert_eq!(types.refined(object, doc), doc);
    }

    #[test]
    fn an_object_operand_is_told_which_object_too() {
        // `: FALSE|object` beside `@return FALSE|CachedItem` and beside the alias's shape spelled
        // out; and the `false` still has to be restated, as for any other operand.
        let mut interner = Interner::default();
        let mut types = TypeTable::new();
        let (object, false_, bool_) = (
            types.builtin(BuiltinTy::Object),
            types.builtin(BuiltinTy::False),
            types.builtin(BuiltinTy::Bool),
        );
        let native = types.union(&[false_, object]);
        let item = types.named(interner.name("CachedItem"));
        let valid = types.field(interner.name("valid"), false, bool_);
        let shape = types.object_shape(&[valid]);
        let named = types.union(&[false_, item]);
        let shaped = types.union(&[false_, shape]);

        assert_eq!(types.refined(native, named), named);
        assert_eq!(types.refined(native, shaped), shaped);
        assert_eq!(types.refined(native, shape), native);
    }
}
