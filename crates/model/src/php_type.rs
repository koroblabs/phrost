//! PHP's reserved type names.
//!
//! Here rather than in one of the IR layers because both the written form (a syntax tree's type
//! hint) and the resolved form (what the index stores) need the same vocabulary, and neither owns
//! it. The rule for this crate applies: more than one layer names these, and no layer owns them.

/// Reserved scalar and pseudo-types. PHP matches these case-insensitively and they can never be
/// class names.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BuiltinTy {
    Int,
    Float,
    String,
    Bool,
    Array,
    Object,
    Callable,
    Iterable,
    Mixed,
    Void,
    Never,
    Null,
    True,
    False,
    /// A stream, a file handle, a database link -- the engine's opaque pointer.
    ///
    /// Deliberately **not** in [`Self::from_name`], and PHP is why. Asked on 8.5.10:
    /// `function f(resource $x)` warns "resource is not a supported builtin type and will be
    /// interpreted as a class name", and `class Resource { }` then `new Resource()` runs. So the
    /// word is class-like wherever a *declaration* uses it, and this kind is reached from a
    /// docblock and from nowhere else -- which is where the standard library writes it, 320 times.
    /// Putting it in `from_name` compiled clean and made a legal class name unreachable, which one
    /// fixture declaring `class Resource` is what caught.
    Resource,
    /// `non-empty-string`: a string holding at least one character.
    ///
    /// A refinement of [`Self::String`] in exactly the way [`Self::True`] and [`Self::False`] are of
    /// [`Self::Bool`]: a value of it is a string to every question but the one about what it holds,
    /// and `string` is the union of it and `''`. A variant rather than a flag beside the type,
    /// because the three spellings a value travels in -- a builtin a cast or a literal states, a
    /// type written in the file, a type interned from another -- all carry a `BuiltinTy` already, so
    /// the refinement reaches every one of them without a fourth shape. The cost is the one
    /// `Resource` names above: a reader matching `String` alone does not see this, so every such
    /// reader asks [`Self::is_string`] or [`Self::unrefined`] instead.
    ///
    /// Docblock-only, like `Resource`, and deliberately absent from [`Self::from_name`]: no hint
    /// can write it, and a native declaration that renders it would not parse. See
    /// [`Self::native`].
    NonEmptyString,
    /// `non-falsy-string`, and its alias `truthy-string`: non-empty and not `'0'`, the one other
    /// string PHP reads as false. Every one of these is a [`Self::NonEmptyString`] too.
    NonFalsyString,
}

impl BuiltinTy {
    /// Recognizes a reserved type name, case-insensitively as PHP does.
    ///
    /// Returning `None` is the signal that a name is class-like, so this list being complete is
    /// what keeps `int` from being looked up as a class.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "int" => Self::Int,
            "float" => Self::Float,
            "string" => Self::String,
            "bool" => Self::Bool,
            "array" => Self::Array,
            "object" => Self::Object,
            "callable" => Self::Callable,
            "iterable" => Self::Iterable,
            "mixed" => Self::Mixed,
            "void" => Self::Void,
            "never" => Self::Never,
            "null" => Self::Null,
            "true" => Self::True,
            "false" => Self::False,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "float",
            Self::String => "string",
            Self::Bool => "bool",
            Self::Array => "array",
            Self::Object => "object",
            Self::Callable => "callable",
            Self::Iterable => "iterable",
            Self::Mixed => "mixed",
            Self::Void => "void",
            Self::Never => "never",
            Self::Null => "null",
            Self::True => "true",
            Self::False => "false",
            Self::Resource => "resource",
            Self::NonEmptyString => "non-empty-string",
            Self::NonFalsyString => "non-falsy-string",
        }
    }

    /// Whether this is a string, whatever it is known to hold.
    ///
    /// The question nearly every reader of [`Self::String`] is really asking -- whether `is_string`
    /// is true of it, whether it may key an array, whether `.` may take it. Asked through here so a
    /// refinement answers the way the plain string does.
    pub fn is_string(self) -> bool {
        matches!(
            self,
            Self::String | Self::NonEmptyString | Self::NonFalsyString
        )
    }

    /// The type a refinement narrows, and every other type unchanged: what a type predicate sees.
    ///
    /// Only the string refinements. `true` and `false` are refinements of `bool` too, but every
    /// reader of them was written knowing that, and folding them here would change what each of
    /// those answers.
    pub fn unrefined(self) -> Self {
        match self.is_string() {
            true => Self::String,
            false => self,
        }
    }

    /// The spelling a native declaration can carry: the refinement taken off, since PHP has no
    /// syntax for one. For a repair that writes a type into a signature.
    pub fn native(self) -> Self {
        self.unrefined()
    }

    /// How much a string is known to hold: nothing, something, something that is not `'0'`.
    ///
    /// `None` for anything that is not a string. Ordered, which is the whole use of it: one string
    /// type admits another exactly when its rank is no higher, and two joined keep the lower.
    pub fn string_rank(self) -> Option<u8> {
        Some(match self {
            Self::String => 0,
            Self::NonEmptyString => 1,
            Self::NonFalsyString => 2,
            _ => return None,
        })
    }

    /// What a string whose text is known holds: nothing, `'0'`, or something truthy.
    ///
    /// The one reading of a literal's text, for every layer that has the text -- a quoted string
    /// lowered from source, a literal type in a docblock, the argument a call wrote. `''` is the
    /// plain `string` rather than a type of its own, since nothing here keeps literal types: it is
    /// the half of `string` a refinement refuses, which is all a judgement needs to know of it.
    pub fn string_holding(text: &[u8]) -> Self {
        match text {
            [] => Self::String,
            b"0" => Self::NonEmptyString,
            _ => Self::NonFalsyString,
        }
    }

    /// What two pieces joined by `.` hold: the more of the two, since the result holds both and
    /// joining never empties a string nor shortens one to `'0'`. The dual of
    /// [`Self::string_join`], which is what *one of* two strings holds. Anything that is not a
    /// string holds nothing as a piece.
    pub fn concatenated(self, other: Self) -> Self {
        let rank = |kind: Self| kind.string_rank().unwrap_or(0);
        Self::string_of_rank(rank(self).max(rank(other)))
    }

    /// The string type of a rank, the inverse of [`Self::string_rank`].
    pub fn string_of_rank(rank: u8) -> Self {
        match rank {
            0 => Self::String,
            1 => Self::NonEmptyString,
            _ => Self::NonFalsyString,
        }
    }

    /// Two string types joined: what a value that is one or the other is known to hold.
    ///
    /// `None` unless both are strings. `non-empty-string|string` is `string`, because the empty
    /// string the plain one admits is a value of the union.
    pub fn string_join(self, other: Self) -> Option<Self> {
        Some(Self::string_of_rank(
            self.string_rank()?.min(other.string_rank()?),
        ))
    }

    /// Whether `?T` is a legal spelling of "this, or null".
    ///
    /// `?false` is not PHP; `false|null` is, and 58 stub functions declare exactly that -- `parse_url`
    /// and `shell_exec` among them -- so a nullable whose inner type is a boolean literal has to be
    /// spelled as the union or hover prints something nobody can write back. The `|null` suffix is
    /// always legal, which is what makes it the fallback rather than the other way round.
    ///
    /// Only the two literals. Every renderer folds a bare `null` into the flag before it gets here,
    /// and `?void` / `?never` / `?mixed` are types nothing constructs a nullable of.
    pub fn takes_a_question_mark(self) -> bool {
        !matches!(self, Self::True | Self::False)
    }
}

/// Type names whose meaning depends on the enclosing class.
///
/// Kept as their own case all the way into the index rather than being resolved at ingest: `static`
/// means the *called* class, so a stored `static` return type has to stay unresolved until there is
/// a receiver to resolve it against.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RelativeTy {
    /// The declaring class.
    SelfTy,
    /// The called class — late static binding.
    Static,
    /// The parent class.
    Parent,
}

impl RelativeTy {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "self" => Self::SelfTy,
            "static" => Self::Static,
            "parent" => Self::Parent,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SelfTy => "self",
            Self::Static => "static",
            Self::Parent => "parent",
        }
    }
}

/// PHP's superglobals, which exist in every scope without being declared anywhere.
///
/// A fixed list because that is what they are: the language defines them, no file declares them, and
/// there is no table to look them up in. Without this, `$_` offers nothing in a project full of
/// `$_GET` and `$_SERVER`.
pub const SUPERGLOBALS: &[&str] = &[
    "GLOBALS", "_SERVER", "_GET", "_POST", "_FILES", "_COOKIE", "_SESSION", "_REQUEST", "_ENV",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_names_are_matched_case_insensitively() {
        assert_eq!(BuiltinTy::from_name("INT"), Some(BuiltinTy::Int));
        assert_eq!(BuiltinTy::from_name("Void"), Some(BuiltinTy::Void));
        assert_eq!(RelativeTy::from_name("STATIC"), Some(RelativeTy::Static));
    }

    #[test]
    fn a_class_name_is_not_mistaken_for_a_reserved_one() {
        // If this ever returned Some, `Integer` would be looked up as a scalar and never resolve.
        assert_eq!(BuiltinTy::from_name("Integer"), None);
        assert_eq!(BuiltinTy::from_name("Str"), None);
        assert_eq!(RelativeTy::from_name("SelfHelp"), None);
    }
}

/// The ways a return branch says its value may not be there.
///
/// A branch reducing to one template keeps the sentinel beside it -- `@return V|null` and
/// `@return V|false` both bind `V` to the same thing and say different things about the result.
/// Apart rather than one flag, because PHP's `false` is a value and `null` is the absence of one,
/// which is the rule the interned union constructor already states as *only `null` lifts out*. Held
/// as one flag, `reset($rows)` answered `?Row`: a guard written `=== FALSE`, which is the one PHP
/// makes true, left the finding standing, and `=== NULL`, which proves nothing, cleared it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Absence {
    /// `@return V|null`, and the `?V` spelling of it.
    Null,
    /// `@return V|false` -- every array-pointer function, and `strtotime`'s neighbours.
    False,
    /// Both, which the DOM, mysqli and intl stubs each write: `@return DOMAttr|null|false`.
    NullOrFalse,
}

impl Absence {
    /// One more sentinel met by a collector that walks a union one operand at a time.
    pub fn and(self, other: Self) -> Self {
        match self == other {
            true => self,
            false => Self::NullOrFalse,
        }
    }

    pub fn admits_null(self) -> bool {
        matches!(self, Self::Null | Self::NullOrFalse)
    }

    pub fn admits_false(self) -> bool {
        matches!(self, Self::False | Self::NullOrFalse)
    }
}

/// What a value a framework declares by name holds, in the few shapes such a declaration can say.
///
/// A Laravel command's `{user}` is a string, `{user?}` a string or nothing, `{--force}` a flag and
/// `{ids*}` a list of strings. Three layers have to agree on it -- the framework that reads the
/// declaration, the index that carries the rule down, the view that answers a read -- and none of
/// them owns the others, which is this crate's bar.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct ValueShape {
    pub builtin: BuiltinTy,
    /// Whether the value -- or, for a list, each element of it -- may be null.
    pub nullable: bool,
    /// Whether it is a list of such values.
    pub list: bool,
}

/// As PHP's docblocks write it: `?string`, `list<?string>`, `bool`.
impl std::fmt::Display for ValueShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let nullable = if self.nullable { "?" } else { "" };
        match self.list {
            true => write!(f, "list<{nullable}{}>", self.builtin.as_str()),
            false => write!(f, "{nullable}{}", self.builtin.as_str()),
        }
    }
}

/// One input a framework's declaration writes: its name, what it holds, and where it is written.
///
/// Where, because the declaration is the only place an input is declared at all -- `{--queue=}` in a
/// Laravel command's `$signature` -- so it is where a read of it is declared, and the only thing a
/// rename has to move besides the reads.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeclaredInput {
    /// As the declaration writes it before the name a reader is given: `user`, or `--queue` for an
    /// option read as `queue`.
    pub name: String,
    pub shape: ValueShape,
    /// The bare name in the text the declaration was read from: `queue` of `{--queue=}`.
    pub range: crate::TextRange,
    /// What the declaration says the input is for, when it says: Laravel's `{user : The user}`.
    pub description: Option<String>,
}
