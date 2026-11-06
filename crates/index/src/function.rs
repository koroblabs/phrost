use phrost_sema::ParamRow;
use phrost_ty::{TypeId, TypeTable};

/// One free function's signature, as the file declared it.
///
/// Only the parts not already on the [`DeclaredSymbol`](phrost_defmap::DeclaredSymbol) that the
/// catalog points at: the name, the fully-qualified name, and every range live there, and storing
/// them twice would make the two able to disagree.
///
/// Parameter rows are shared with class members rather than reinvented: a parameter reads the same
/// whether it belongs to a function or a method, and two renderings of it would drift.
///
/// Built with a struct literal rather than a constructor: the two places that build one are both in
/// this crate, and a positional constructor of eight arguments is a codec bug waiting to happen --
/// §8 records one where a cache read them back in the wrong order and nothing failed.
#[derive(Debug)]
pub struct FunctionRow {
    /// Index into the file's declared symbols — the same identity the catalog's `Declarer` uses, so
    /// going from "this file declares `App\make`" to its signature is a lookup rather than a search.
    pub(crate) symbol: u32,
    pub(crate) params: Box<[ParamRow]>,
    /// Return type. `None` means unannotated, which is not `mixed`.
    pub(crate) ty: Option<TypeId>,
    /// What its docblock says it returns, when that says more than the signature.
    ///
    /// Kept apart from [`Self::ty`] for the reason a member's is: the two are ranked rather than
    /// merged -- `array` refined by `list<Item>` is the docblock's answer, and `?Thing` refined by
    /// `Thing` is still nullable.
    pub(crate) doc_ty: Option<TypeId>,
    pub(crate) returns_by_reference: bool,
    /// What its docblock promises beyond its signature.
    ///
    /// Boxed for the reason a member's is: almost no function has one, and the widest arm is several
    /// pointers wide.
    pub(crate) contract: Option<Box<phrost_sema::Contract>>,
    /// Whether the body reads arguments the signature does not name.
    ///
    /// `func_get_args()` / `func_num_args()`, which make the parameter list a lower bound rather than
    /// a contract: PHP raises nothing when a caller supplies more, and the extra values come back
    /// through the call rather than through a name. Laravel's
    /// `PendingRequest::throwIf($condition)` reads `func_get_args()[1]`, and its own tests pass two
    /// arguments to it on purpose.
    ///
    /// A fact about the *body*, kept here because this is the only side of the boundary that can
    /// hold one: a closed file's statements are never retained, so nothing above can look. Read by
    /// `IndexView::callee_takes_more_than_it_names` and nothing else.
    pub(crate) reads_own_arguments: bool,
    /// PHP 8.5's `#[\NoDiscard]`, read off the declaration.
    ///
    /// Here rather than worked out at each call: the declaration a call discards is usually in
    /// another file, and a warm start never lowers it.
    pub(crate) is_no_discard: bool,
    /// Whether `@deprecated` or `#[\Deprecated]` says the declaration should not be reached for.
    pub(crate) is_deprecated: bool,
}

impl FunctionRow {
    /// Whether the declaration is `#[\NoDiscard]`. See the field.
    pub fn is_no_discard(&self) -> bool {
        self.is_no_discard
    }

    /// Whether the declaration says it is deprecated. See the field.
    pub fn is_deprecated(&self) -> bool {
        self.is_deprecated
    }

    /// Whether the body reads arguments the signature does not name. See the field.
    pub fn reads_own_arguments(&self) -> bool {
        self.reads_own_arguments
    }

    pub fn contract(&self) -> Option<&phrost_sema::Contract> {
        self.contract.as_deref()
    }

    pub fn symbol(&self) -> u32 {
        self.symbol
    }

    /// Whether it returns by reference, for writing one to a cache.
    pub(crate) fn returns_by_reference(&self) -> bool {
        self.returns_by_reference
    }

    pub fn params(&self) -> &[ParamRow] {
        &self.params
    }

    pub fn ty(&self) -> Option<TypeId> {
        self.ty
    }

    /// What the docblock adds, for writing one to a cache.
    pub(crate) fn doc_ty(&self) -> Option<TypeId> {
        self.doc_ty
    }

    /// The return type both halves of the declaration agree on.
    ///
    /// The same ranking a member uses, and shared with it deliberately: a docblock refines what the
    /// signature said rather than replacing it, and two rules for one question would drift.
    pub fn effective_ty(&self, types: &TypeTable) -> Option<TypeId> {
        match (self.ty, self.doc_ty) {
            (Some(native), Some(doc)) => Some(types.refined(native, doc)),
            (Some(native), None) => Some(native),
            (None, doc) => doc,
        }
    }

    /// `make(int $count): ?Thing`, or `&byRef()` for a function returning a reference.
    ///
    /// Takes the name because the row does not store one: the symbol it belongs to already has the
    /// name, spelled as written and fully qualified, and a second copy could disagree with it.
    pub fn signature(&self, name: &str, types: &TypeTable) -> String {
        let mut out = String::new();
        if self.returns_by_reference {
            out.push('&');
        }
        out.push_str(name);
        out.push('(');
        for (position, param) in self.params.iter().enumerate() {
            if position > 0 {
                out.push_str(", ");
            }
            param.write(&mut out, types);
        }
        out.push(')');
        if let Some(ty) = self.ty {
            out.push_str(": ");
            out.push_str(&types.render(ty));
        }
        out
    }
}
