use lspkit_arena::IdSlice;
use phrost_model::{BuiltinTy, RelativeTy};

use crate::ids::{NameRefId, TypeRefId};

/// A PHP native type expression, as written.
///
/// Syntax-shaped, not semantic: `?Foo` stays a `Nullable` wrapping a `Named`, and nothing here
/// knows whether `Foo` exists. Turning this into a semantic type — flattening `?T` into
/// `T|null`, merging it with the docblock type, resolving names — is the type layer's job.
///
/// Docblock types are richer than this (generics, array shapes, literal types) and get their own
/// representation. Keeping them apart means a change to generics parsing cannot disturb the
/// native-type path that most code actually uses.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TypeRefData {
    /// A reserved scalar or pseudo-type: `int`, `string`, `bool`, `float`, `array`, `object`,
    /// `callable`, `iterable`, `mixed`, `void`, `never`, `null`, `true`, `false`.
    Builtin(BuiltinTy),
    /// `self`, `static`, or `parent` — meaningful only relative to the enclosing class.
    Relative(RelativeTy),
    /// A class-like name.
    Named(NameRefId),
    /// `?T`. Kept as a wrapper rather than desugared to `T|null` so that hover and rename can
    /// reproduce what the author wrote.
    Nullable(TypeRefId),
    /// `A|B|C`.
    Union(IdSlice<TypeRefId>),
    /// `A&B`.
    Intersection(IdSlice<TypeRefId>),
    /// A type with arguments, which only a docblock can write: `list<User>`, `array<int, User>`,
    /// `User[]`, `Collection<User>`.
    ///
    /// The base is what the value *is* — an array, or a class with members — and the arguments say
    /// what it contains. Both halves are needed: the base answers `$items->`, the last argument
    /// answers `foreach ($items as $item)`.
    Generic {
        base: TypeRefId,
        args: IdSlice<TypeRefId>,
        /// `non-empty-array<K, V>` and `non-empty-list<V>`: an array that holds an element.
        ///
        /// A flag beside the arguments rather than a base of its own, so that every reader that
        /// asks what an array holds keeps answering for this one without knowing it exists -- and
        /// the few that ask whether it can be empty read it here. Folded into the plain array it
        /// was the one thing a declaration said that nothing could recover, which is how
        /// `reset($rows)` on a `non-empty-list` came to be reported as possibly `false`.
        non_empty: bool,
    },
    /// `array{id: int, name: string}` -- an array whose keys are known individually.
    ///
    /// Its own shape rather than a `Generic` over one element type, because that is the whole
    /// difference: `array<string, mixed>` says every key holds the same thing and this says which key
    /// holds what. `$row['id']` is an `int` and `$row['name']` is a `string`, and no single element
    /// type can say both.
    ///
    /// The operands are [`Self::ShapeField`]s and nothing else.
    Shape {
        fields: IdSlice<TypeRefId>,
        /// Written with a trailing `...`, or holding fewer keys than were written: the array may
        /// hold keys the shape does not name. Without it a shape is sealed, which is how PHPStan and
        /// Psalm read `array{foo: int}` and what makes `...` worth spelling.
        open: bool,
        /// Written `list{...}`: the keys are `0` upwards, in order, which `array{1: A, 0: B}` with the
        /// same two keys is not.
        list: bool,
    },
    /// `object{pivot: Pivot}` -- an object whose named properties are known.
    ///
    /// Not a [`Self::Shape`] with a flag, because every reader of a shape reads an *array*: its keys
    /// are subscripts, it is iterated, a literal can be one. None of that is true here, and a flag
    /// is a spelling a reader of the other kind would answer for without knowing it exists. What
    /// this is, is an object -- and what it adds is what its properties hold. Laravel's
    /// `BelongsToMany` loads `TRelatedModel&object{pivot: TPivotModel}`, and `->pivot` is the read.
    ///
    /// The operands are [`Self::ShapeField`]s and nothing else.
    ObjectShape { fields: IdSlice<TypeRefId> },
    /// One field of a [`Self::Shape`]: `id: int`, or `id?: int` when the key may be absent.
    ///
    /// A node of the type expression rather than a side table, so that a shape's fields are stored,
    /// compared and rendered by the machinery every other operand list already uses. It never appears
    /// outside a shape's operands.
    ShapeField {
        key: phrost_model::Name,
        /// Written `id?:`. The key may be missing entirely, which is not the same as holding null --
        /// though PHP's answer for reading an absent key is a warning and null.
        optional: bool,
        ty: TypeRefId,
    },
}

/// The more informative of a native declaration and a docblock's claim about the same thing.
///
/// A native hint is what the runtime enforces, so it normally wins outright. The exception is the one
/// PHP cannot express: `array` and `iterable` say nothing about what is inside, and `@param
/// list<Repo>` beside `array $items` is not a competing claim but the missing half of the same one.
/// Taking the native type there discards the element type, which is what a `foreach` needs and the
/// most common thing anyone documents.
///
/// Deliberately narrow. Only a bare container is refined, so a docblock that disagrees with a native
/// class cannot override it -- the risk of trusting a stale comment over a declaration the runtime
/// enforces is exactly why the general rule is the other way round.
///
/// The resolved-type form of this rule is [`phrost_ty::TypeTable::refined`]; the two representations
/// cannot share code, so they share a name and this comment.
pub fn refined_type_ref(
    hir: &crate::FileHir,
    native: Option<TypeRefId>,
    doc: Option<TypeRefId>,
) -> Option<TypeRefId> {
    match (native, doc) {
        // `mixed` is the declaration saying nothing at all, so whatever the docblock says is more
        // than it -- including a union or a plain class, where a bare container is only refined by
        // a type that supplies arguments. `mixed $value` with `@param Row|list<Row> $value` is how
        // a function taking either is written since PHP 8.
        (Some(native), Some(doc)) if is_mixed(hir, native) => Some(doc),
        (Some(native), Some(doc)) if is_bare_container(hir, native) && has_arguments(hir, doc) => {
            Some(doc)
        }
        // The same relationship operand by operand: `array|false $lines` beside `@param
        // list<string>|false $lines`. Iterating such a parameter typed nothing while its nullable
        // spelling typed the element.
        (Some(native), Some(doc)) if refines_each_operand(hir, native, doc) => Some(doc),
        // The same claim with the arguments added, which PHP has no syntax for: `@param
        // Items<Special>` beside `Items $items`. Compared by what was written, because this layer
        // resolves nothing -- two different spellings of one class therefore decline to refine,
        // which is a miss and not a wrong answer.
        (Some(native), Some(doc)) if argues_about(hir, native, doc) => Some(doc),
        // `object` is what an author writes when PHP has no syntax for which class they mean, so a
        // docblock naming one -- or naming several -- is more than it. `@return array` beside
        // `: object` is a mistake in the source, and there the hint is what the runtime enforces.
        (Some(native), Some(doc)) if is_object(hir, native) && says_which_object(hir, doc) => {
            Some(doc)
        }
        // `\Closure` is PHP's class spelling of a callable, and a docblock's `Closure(Item): Row`
        // says what invoking it gives. See `says_what_calling_it_gives` on the interned side for what
        // is given up: `Closure`'s own three methods stop resolving, and invoking the parameter --
        // the entire point of taking one -- starts.
        (Some(native), Some(doc))
            if names_closure(hir, native) && returns_when_called(hir, doc) =>
        {
            Some(doc)
        }
        // `@param non-empty-string $name` beside `string $name`: the hint's builtin with what it
        // holds narrowed, which PHP cannot spell. See `narrows_the_builtin` on the interned side.
        (Some(native), Some(doc)) if narrows_the_builtin(hir, native, doc) => Some(doc),
        (Some(native), _) => Some(native),
        (None, doc) => doc,
    }
}

/// `mixed`, `?` and all -- though PHP rejects `?mixed`, a docblock writes it.
fn is_mixed(hir: &crate::FileHir, id: TypeRefId) -> bool {
    match hir.type_ref(id) {
        TypeRefData::Builtin(BuiltinTy::Mixed) => true,
        TypeRefData::Nullable(inner) => is_mixed(hir, *inner),
        _ => false,
    }
}

/// `array` or `iterable`, with no arguments, `?` and all.
fn is_bare_container(hir: &crate::FileHir, id: TypeRefId) -> bool {
    match hir.type_ref(id) {
        TypeRefData::Builtin(BuiltinTy::Array | BuiltinTy::Iterable) => true,
        TypeRefData::Nullable(inner) => is_bare_container(hir, *inner),
        _ => false,
    }
}

/// `object`, `?` and all.
fn is_object(hir: &crate::FileHir, id: TypeRefId) -> bool {
    match hir.type_ref(id) {
        TypeRefData::Builtin(BuiltinTy::Object) => true,
        TypeRefData::Nullable(inner) => is_object(hir, *inner),
        _ => false,
    }
}

/// Whether a written type names a class, or several.
///
/// Every operand of a union, because a union of classes is a class the hint could not spell just as
/// much as one class is -- the same rule the interned side's `names_a_class` follows, and it was
/// missing there too until a declaration with two classes in its docblock turned up.
fn names_a_class(hir: &crate::FileHir, id: TypeRefId) -> bool {
    match hir.type_ref(id) {
        TypeRefData::Named(_) | TypeRefData::Relative(_) => true,
        TypeRefData::Generic { base, .. } => names_a_class(hir, *base),
        TypeRefData::Nullable(inner) => names_a_class(hir, *inner),
        TypeRefData::Union(parts) => {
            let operands = hir.type_ref_list(*parts);
            !operands.is_empty() && operands.iter().all(|&part| names_a_class(hir, part))
        }
        _ => false,
    }
}

/// Whether a docblock says which instances an `object` hint admits: a class, several, or the
/// properties of an `object{cid: string}`. The interned side's rule of the same name says why it is
/// asked of each operand of a union too.
fn says_which_object(hir: &crate::FileHir, id: TypeRefId) -> bool {
    names_a_class(hir, id) || matches!(hir.type_ref(id), TypeRefData::ObjectShape { .. })
}

/// Whether a written docblock type is the hint's builtin with what it holds narrowed -- a
/// `non-empty-string` beside a `string`.
fn narrows_the_builtin(hir: &crate::FileHir, native: TypeRefId, doc: TypeRefId) -> bool {
    match (hir.type_ref(native), hir.type_ref(doc)) {
        (TypeRefData::Builtin(native), TypeRefData::Builtin(doc)) => native
            .string_rank()
            .zip(doc.string_rank())
            .is_some_and(|(native, doc)| doc > native),
        _ => false,
    }
}

/// Whether a written type is the class `Closure` and nothing else.
fn names_closure(hir: &crate::FileHir, id: TypeRefId) -> bool {
    match hir.type_ref(id) {
        TypeRefData::Named(reference) => hir
            .name_ref(*reference)
            .path
            .as_str()
            .rsplit('\\')
            .next()
            .is_some_and(|last| last.eq_ignore_ascii_case("Closure")),
        TypeRefData::Nullable(inner) => names_closure(hir, *inner),
        _ => false,
    }
}

/// Whether a written type is a callable that says what invoking it gives back.
fn returns_when_called(hir: &crate::FileHir, id: TypeRefId) -> bool {
    match hir.type_ref(id) {
        TypeRefData::Generic { base, args, .. } => {
            matches!(
                hir.type_ref(*base),
                TypeRefData::Builtin(BuiltinTy::Callable)
            ) && !hir.type_ref_list(*args).is_empty()
        }
        TypeRefData::Nullable(inner) => returns_when_called(hir, *inner),
        _ => false,
    }
}

/// Whether the docblock type is the native one with its arguments supplied.
fn argues_about(hir: &crate::FileHir, native: TypeRefId, doc: TypeRefId) -> bool {
    match (hir.type_ref(native), hir.type_ref(doc)) {
        (_, TypeRefData::Generic { base, .. }) => written_alike(hir, native, *base),
        (TypeRefData::Nullable(native), _) => argues_about(hir, *native, doc),
        (_, TypeRefData::Nullable(doc)) => argues_about(hir, native, *doc),
        _ => false,
    }
}

/// Whether two written type references spell the same thing.
fn written_alike(hir: &crate::FileHir, left: TypeRefId, right: TypeRefId) -> bool {
    match (hir.type_ref(left), hir.type_ref(right)) {
        (TypeRefData::Named(left), TypeRefData::Named(right)) => {
            // PHP looks class names up case-insensitively, and a docblock is where the casing of a
            // hand-written name most often drifts.
            hir.name_ref(*left)
                .path
                .as_str()
                .eq_ignore_ascii_case(hir.name_ref(*right).path.as_str())
        }
        (TypeRefData::Builtin(left), TypeRefData::Builtin(right)) => left == right,
        (TypeRefData::Relative(left), TypeRefData::Relative(right)) => left == right,
        _ => false,
    }
}

/// Whether the type says what it contains.
fn has_arguments(hir: &crate::FileHir, id: TypeRefId) -> bool {
    match hir.type_ref(id) {
        // A shape counts. `@return array{app: Application}` beside a native `: array` is the same
        // relationship as `@return list<Repo>` beside one -- the docblock supplying what PHP has no
        // syntax for -- and treating it as a competing claim let the bare `array` win and threw the
        // keys away.
        TypeRefData::Generic { .. } | TypeRefData::Shape { .. } => true,
        TypeRefData::Nullable(inner) => has_arguments(hir, *inner),
        // Every operand, sentinels aside, as the interned side reads it: `array{x: Row}|array{}`
        // beside `array $row` is one claim written as two branches. Without this a parameter's
        // callers read the shape and its own body read `array`.
        TypeRefData::Union(operands) => {
            let mut said_something = false;
            for &operand in hir.type_ref_list(*operands) {
                if matches!(
                    hir.type_ref(operand),
                    TypeRefData::Builtin(BuiltinTy::Null | BuiltinTy::False)
                ) {
                    continue;
                }
                if !has_arguments(hir, operand) {
                    return false;
                }
                said_something = true;
            }
            said_something
        }
        _ => false,
    }
}

/// One operand of a written union, with the `null` a `?` spells counted as one of them.
#[derive(Clone, Copy)]
enum Operand {
    Written(TypeRefId),
    Null,
}

/// Whether a docblock is a native union with each bare container in it given arguments, and every
/// other operand restated.
///
/// Both directions are required: an operand the docblock drops is one the runtime still admits, and
/// one it adds is one the runtime refuses. Either way the docblock is a competing claim and the hint
/// wins, as it does for a lone type.
fn refines_each_operand(hir: &crate::FileHir, native: TypeRefId, doc: TypeRefId) -> bool {
    if !matches!(
        hir.type_ref(native),
        TypeRefData::Union(_) | TypeRefData::Nullable(_)
    ) {
        return false;
    }
    let (mut natives, mut docs) = (Vec::new(), Vec::new());
    operands(hir, native, &mut natives);
    operands(hir, doc, &mut docs);
    let alike = |native: Operand, doc: Operand| match (native, doc) {
        (Operand::Null, Operand::Null) => true,
        (Operand::Null, Operand::Written(id)) | (Operand::Written(id), Operand::Null) => {
            matches!(hir.type_ref(id), TypeRefData::Builtin(BuiltinTy::Null))
        }
        (Operand::Written(native), Operand::Written(doc)) => written_alike(hir, native, doc),
    };
    let refines = |native: Operand, doc: Operand| match (native, doc) {
        (Operand::Written(native), Operand::Written(doc)) => {
            (is_bare_container(hir, native) && has_arguments(hir, doc))
                || argues_about(hir, native, doc)
                || narrows_the_builtin(hir, native, doc)
                || (is_object(hir, native) && says_which_object(hir, doc))
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

/// A written type's operands, flattened through unions and the `?` of a nullable.
fn operands(hir: &crate::FileHir, id: TypeRefId, out: &mut Vec<Operand>) {
    match hir.type_ref(id) {
        TypeRefData::Union(parts) => {
            for &part in hir.type_ref_list(*parts) {
                operands(hir, part, out);
            }
        }
        TypeRefData::Nullable(inner) => {
            operands(hir, *inner, out);
            out.push(Operand::Null);
        }
        _ => out.push(Operand::Written(id)),
    }
}
