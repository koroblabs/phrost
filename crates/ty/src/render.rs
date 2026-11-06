use std::fmt::Write as _;

use phrost_model::{BuiltinTy, RelativeTy, short_name};

use crate::{
    table::{TypeId, TypeTable},
    type_ref::TypeRef,
};

/// How much a rendering is allowed to say beyond the node it is reading.
///
/// Carried as one value rather than as a growing parameter list, because every recursive step has
/// to pass the whole of it on: a relative type nested three arguments deep is exactly the one this
/// exists for.
struct Spelling<'a> {
    /// Every class name written out in full, for anywhere two short names could collide.
    qualified: bool,
    /// What `self`, `static` and `parent` mean for the value being described, when the caller
    /// knows. `None` -- either no resolver or a resolver with no answer -- leaves the word as
    /// written, which is all a declaration read with no receiver in hand can honestly say.
    relative: Option<&'a dyn Fn(RelativeTy) -> Option<String>>,
}

impl Spelling<'_> {
    /// Class names cut to their last segment, relative words left as written.
    const SHORT: Self = Self {
        qualified: false,
        relative: None,
    };

    /// The same with every name written out in full.
    const QUALIFIED: Self = Self {
        qualified: true,
        relative: None,
    };

    /// The class a relative word means here, already shortened by whoever answered.
    fn resolve(&self, relative: RelativeTy) -> Option<String> {
        (self.relative?)(relative)
    }
}

/// One shape key, quoted where the docblock grammar needs it.
///
/// A bare key is a PHP identifier and nothing else. Drupal's render arrays are keyed `#type`,
/// `#title`, `#attributes` -- and an unquoted `array{#type: string}` does not parse at all: the
/// whole shape reads back as a bare `array`, so every key it named is gone with no error anywhere.
///
/// Here rather than in each renderer because a type that cannot be read back is only a bug once
/// something *writes* one, and the docblock generator does -- two spellings of this rule is how one
/// of them comes to be missing a case the other has.
pub fn shape_key(key: &str) -> std::borrow::Cow<'_, str> {
    // An integer key is written bare too, and it has to be: `array{0: string}` names the int slot a
    // positional literal fills, and `array{'0': string}` names the *string* key `'0'`, which is a
    // different type. PHP's own canonicalisation is the rule -- `'01'` stays a string key, so only a
    // run of digits with no leading zero is one, and `'0'` alone is.
    if phrost_model::is_an_integer_key(key) {
        return std::borrow::Cow::Borrowed(key);
    }
    let bare = !key.is_empty()
        && !key.starts_with(|first: char| first.is_ascii_digit())
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if bare {
        return std::borrow::Cow::Borrowed(key);
    }
    // A key holding a control character is written the way the source wrote it: double quotes and
    // PHP's own escapes. Symfony keys arrays by terminal escape sequences (`"\0337"`) and by
    // `"\0~separator=\0"`, and rendered between single quotes those bytes went raw into a hover,
    // into the docblock a quick fix writes back into the file, and into the coverage dump -- where
    // one NUL made `diff` call the whole dump binary and report no changed answers at all.
    if key.chars().any(char::is_control) {
        let mut written = String::with_capacity(key.len() + 2);
        written.push('"');
        for ch in key.chars() {
            match ch {
                '"' | '\\' | '$' => {
                    written.push('\\');
                    written.push(ch);
                }
                ch if ch.is_control() => {
                    for byte in ch.to_string().bytes() {
                        written.push_str(&format!("\\x{byte:02x}"));
                    }
                }
                ch => written.push(ch),
            }
        }
        written.push('"');
        return std::borrow::Cow::Owned(written);
    }
    // Single quotes otherwise, which take no escape but the quote itself -- and a key holding one is
    // not a thing PHP writes, so it is left to read oddly rather than given a rule with no case to
    // exercise it.
    std::borrow::Cow::Owned(format!("'{key}'"))
}

/// Whether a trailing `[]` would bind to the whole of this type.
///
/// It binds tighter than `|`, `&` and `?`, so only a type with no operator at its top level can
/// take the suffix and still mean what it meant.
fn brackets_bind(ty: &TypeRef) -> bool {
    !matches!(
        ty,
        TypeRef::Union(_)
            | TypeRef::Intersection(_)
            | TypeRef::Nullable(_)
            | TypeRef::Shape { .. }
            | TypeRef::ObjectShape { .. }
    )
}

impl TypeTable {
    /// A type as an editor shows it: class names shortened to their last segment.
    ///
    /// Short rather than fully qualified because that is what the declaration being described says,
    /// and a hover reading `?\Drupal\Core\Entity\EntityInterface` costs a line of screen to repeat
    /// what the `use` at the top of the file already established. [`Self::render_qualified`] is
    /// there for the cases where the ambiguity matters.
    pub fn render(&self, id: TypeId) -> String {
        let mut out = String::new();
        self.write(&mut out, id, &Spelling::SHORT);
        out
    }

    /// The same, with every class name fully qualified. For anywhere two short names could collide.
    pub fn render_qualified(&self, id: TypeId) -> String {
        let mut out = String::new();
        self.write(&mut out, id, &Spelling::QUALIFIED);
        out
    }

    /// The same again, with `self`, `static` and `parent` written as the class they mean.
    ///
    /// For describing a *value* rather than a declaration. `static` is an answer about where a type
    /// was written, and a reader asking what a value is has already been told that much by the
    /// declaration -- so a rendered `static` is never a right answer to "what is this".
    ///
    /// Applied at every position a relative type can appear, which is why it lives here rather than
    /// in the one caller: substituting only the outermost node left `list<static>` -- what every
    /// backed enum's `cases()` returns -- rendering as `static[]`.
    ///
    /// `resolve` answers for one word and may decline, in which case the word stands as written --
    /// and it answers with the name spelled as the caller wants it, because only the caller knows
    /// whether this rendering leaves the file.
    pub fn render_relative(
        &self,
        id: TypeId,
        qualified: bool,
        resolve: &dyn Fn(RelativeTy) -> Option<String>,
    ) -> String {
        let mut out = String::new();
        self.write(
            &mut out,
            id,
            &Spelling {
                qualified,
                relative: Some(resolve),
            },
        );
        out
    }

    fn write(&self, out: &mut String, id: TypeId, how: &Spelling<'_>) {
        match self.get(id) {
            TypeRef::Builtin(builtin) => out.push_str(builtin.as_str()),
            TypeRef::Relative(relative) => match how.resolve(*relative) {
                Some(class) => out.push_str(&class),
                None => out.push_str(relative.as_str()),
            },
            TypeRef::Named(fqn) => {
                if how.qualified {
                    let _ = write!(out, "\\{fqn}");
                } else {
                    out.push_str(short_name(fqn.as_str()));
                }
            }
            TypeRef::Nullable(inner) => self.write_nullable(out, *inner, how, false),
            // `class-string<T>` is the one generic a docblock ever writes with a plain `string`
            // base, so the base alone is enough to know this is that shape and not an actual
            // `string<T>`, which nobody wrote and nothing else here produces.
            TypeRef::Generic { base, args, .. }
                if matches!(self.get(*base), TypeRef::Builtin(BuiltinTy::String)) =>
            {
                out.push_str("class-string");
                // A bare `class-string` is lowered as one of `object`, which is what it means and
                // not what anybody writes.
                if !matches!(
                    self.operands(*args),
                    [only] if matches!(self.get(*only), TypeRef::Builtin(BuiltinTy::Object))
                ) {
                    out.push('<');
                    self.write_arguments(out, *args, how);
                    out.push('>');
                }
            }
            // `callable(...): T` -- a docblock's `callable(Foo): Bar` keeps only its return, so the
            // parameters are written as an ellipsis rather than invented. The same spelling a
            // first-class callable already renders with, and `callable<Bar>` is not a type PHP can
            // write.
            TypeRef::Generic { base, args, .. }
                if matches!(self.get(*base), TypeRef::Builtin(BuiltinTy::Callable))
                    && let [only] = self.operands(*args) =>
            {
                out.push_str("callable(...): ");
                self.write(out, *only, how);
            }
            // `Item[]`, which is the spelling PHP writes most and the shorter one in a signature.
            // Only for a single element and only when it is a plain name: `Foo|Bar[]` binds the
            // brackets to `Bar` alone and `?Foo[]` to the array rather than the element, so a
            // composite keeps the unambiguous `array<...>`.
            TypeRef::Generic {
                base,
                args,
                non_empty: false,
            } if matches!(self.get(*base), TypeRef::Builtin(BuiltinTy::Array))
                && let [only] = self.operands(*args)
                && brackets_bind(self.get(*only)) =>
            {
                self.write(out, *only, how);
                out.push_str("[]");
            }
            TypeRef::Generic {
                base,
                args,
                non_empty,
            } => {
                match non_empty {
                    true => out.push_str("non-empty-array"),
                    false => self.write(out, *base, how),
                }
                out.push('<');
                self.write_arguments(out, *args, how);
                out.push('>');
            }
            TypeRef::Union(operands) => self.write_composite(out, *operands, "|", how),
            TypeRef::Intersection(operands) => self.write_composite(out, *operands, "&", how),
            TypeRef::Shape { fields, open, list } => {
                out.push_str(if *list { "list{" } else { "array{" });
                self.write_arguments(out, *fields, how);
                if *open {
                    out.push_str(", ...");
                }
                out.push('}');
            }
            TypeRef::ObjectShape { fields } => {
                out.push_str("object{");
                self.write_arguments(out, *fields, how);
                out.push('}');
            }
            TypeRef::Field { key, optional, ty } => {
                let _ = write!(
                    out,
                    "{}{}: ",
                    shape_key(key.as_str()),
                    if *optional { "?" } else { "" }
                );
                // Spelled out, because a `?` one character earlier already means something else
                // here: `Username?: string` says the key may be absent and `Username: ?string` says
                // the value may be null, and the two differ by a space. A message that reports the
                // second against the first is right and reads as nonsense -- which is how a correct
                // finding came back as a bug report.
                match self.get(*ty) {
                    TypeRef::Nullable(inner) => self.write_nullable(out, *inner, how, true),
                    _ => self.write(out, *ty, how),
                }
            }
        }
    }

    /// `?X`, or `X|null` where a leading `?` would read as something other than "or null".
    ///
    /// `long` is the caller saying it is at such a position. Two are unconditional: PHP has no way
    /// to write a nullable union as a `?` -- `?A|B` parses as `(?A)|B` and `?(A|B)` is not a type at
    /// all -- and `?false` is not a type it can parse either. See `BuiltinTy::takes_a_question_mark`.
    fn write_nullable(&self, out: &mut String, inner: TypeId, how: &Spelling<'_>, long: bool) {
        match self.get(inner) {
            TypeRef::Union(parts) => {
                self.write_composite(out, *parts, "|", how);
                out.push_str("|null");
            }
            TypeRef::Builtin(kind) if !kind.takes_a_question_mark() => {
                out.push_str(kind.as_str());
                out.push_str("|null");
            }
            _ if long => {
                self.write(out, inner, how);
                out.push_str("|null");
            }
            _ => {
                out.push('?');
                self.write(out, inner, how);
            }
        }
    }

    /// The arguments of a generic, or the fields of a shape.
    ///
    /// Unparenthesized, unlike an operand of a union: `<`, `,` and `>` already say where each one
    /// ends, so `array<(A|B)>` writes a bracket the reader has to undo and `array<A|B>` cannot be
    /// read any other way.
    fn write_arguments(
        &self,
        out: &mut String,
        operands: crate::table::TypeList,
        how: &Spelling<'_>,
    ) {
        for (position, &operand) in self.operands(operands).iter().enumerate() {
            if position > 0 {
                out.push_str(", ");
            }
            self.write(out, operand, how);
        }
    }

    fn write_composite(
        &self,
        out: &mut String,
        operands: crate::table::TypeList,
        separator: &str,
        how: &Spelling<'_>,
    ) {
        for (position, &operand) in self.operands(operands).iter().enumerate() {
            if position > 0 {
                out.push_str(separator);
            }
            self.write_operand(out, operand, how);
        }
    }

    /// One operand of a composite, parenthesized when it is itself composite.
    ///
    /// PHP only allows this nesting in DNF (`(A&B)|C`), and without the parens `(A&B)|C` and
    /// `A&(B|C)` would render identically — so a flattening bug would not show up in any snapshot.
    fn write_operand(&self, out: &mut String, id: TypeId, how: &Spelling<'_>) {
        match self.get(id) {
            TypeRef::Union(_) | TypeRef::Intersection(_) => {
                out.push('(');
                self.write(out, id, how);
                out.push(')');
            }
            _ => self.write(out, id, how),
        }
    }
}

#[cfg(test)]
mod tests {
    use phrost_model::{BuiltinTy, Interner, RelativeTy};

    use crate::TypeTable;

    #[test]
    fn a_class_name_renders_short_and_qualified() {
        let mut interner = Interner::new();
        let mut types = TypeTable::new();
        let user = types.named(interner.name("App\\Entity\\User"));
        let nullable = types.nullable(user);

        assert_eq!(types.render(nullable), "?User");
        assert_eq!(types.render_qualified(nullable), "?\\App\\Entity\\User");
    }

    #[test]
    fn a_root_namespace_class_has_no_short_form_to_trim() {
        let mut interner = Interner::new();
        let mut types = TypeTable::new();
        let drupal = types.named(interner.name("Drupal"));

        assert_eq!(types.render(drupal), "Drupal");
        assert_eq!(types.render_qualified(drupal), "\\Drupal");
    }

    /// A key PHP writes all day and PHPDoc cannot read bare. Drupal's render arrays are keyed
    /// `#type`, and an unquoted `array{#type: string}` reads back as a plain `array` -- every key
    /// gone, with nothing failing anywhere, which is why the quoting lives in the renderer rather
    /// than in whoever happens to be writing a type out today.
    #[test]
    fn a_key_that_is_not_an_identifier_is_quoted() {
        let mut interner = Interner::new();
        let mut types = TypeTable::new();
        let string = types.builtin(BuiltinTy::String);
        let int = types.builtin(BuiltinTy::Int);
        let hash = types.field(interner.name("#type"), false, string);
        let dashed = types.field(interner.name("data-id"), false, int);
        let plain = types.field(interner.name("plain"), false, int);
        // An integer key is bare, and a key that merely looks like one is not: `array{0: int}`
        // names the slot a positional literal fills and `array{'01': int}` names a string key,
        // which is what PHP keeps `'01'` as.
        let digits = types.field(interner.name("0"), false, int);
        let leading = types.field(interner.name("01"), false, int);
        let optional = types.field(interner.name("#title"), true, string);
        let shape = types.shape(&[hash, dashed, plain, digits, leading, optional], false);

        assert_eq!(
            types.render(shape),
            "array{'#type': string, 'data-id': int, plain: int, 0: int, '01': int, '#title'?: string}"
        );
    }

    /// Symfony's own keys: a terminal escape sequence and a NUL-delimited stub marker. Both read
    /// back as the bytes they are, and neither puts a raw control character into a hover, a written
    /// docblock or a dump.
    #[test]
    fn a_key_holding_a_control_character_is_escaped() {
        let mut interner = Interner::new();
        let mut types = TypeTable::new();
        let string = types.builtin(BuiltinTy::String);
        let escape = types.field(interner.name("\u{1b}[K"), false, string);
        let stub = types.field(interner.name("\0~separator=\0"), false, string);
        let shape = types.shape(&[escape, stub], false);

        assert_eq!(
            types.render(shape),
            r#"array{"\x1b[K": string, "\x00~separator=\x00": string}"#
        );
    }

    /// `?` already means "this key may be absent" one character earlier.
    ///
    /// `Username?: string` and `Username: ?string` are different claims that differ by a space, so a
    /// message reporting the second against the first reads as nonsense however right it is -- which
    /// is how a correct `incompatible-argument` came back as a bug report. PHPStan writes the long
    /// spelling here for the same reason.
    #[test]
    fn a_nullable_value_in_a_shape_is_spelled_out() {
        let mut interner = Interner::new();
        let mut types = TypeTable::new();
        let string = types.builtin(BuiltinTy::String);
        let nullable = types.nullable(string);
        let required = types.field(interner.name("Username"), false, nullable);
        let optional = types.field(interner.name("UserPoolId"), true, nullable);
        let shape = types.shape(&[required, optional], false);

        assert_eq!(
            types.render(shape),
            "array{Username: string|null, UserPoolId?: string|null}"
        );
        // And nowhere else: `?string` is the shorter spelling and the one PHP itself writes.
        assert_eq!(types.render(nullable), "?string");
    }

    #[test]
    fn nested_composites_are_parenthesized() {
        let mut interner = Interner::new();
        let mut types = TypeTable::new();
        let a = types.named(interner.name("A"));
        let b = types.named(interner.name("B"));
        let countable = types.named(interner.name("Countable"));
        let and = types.intersection(&[a, b]);
        let dnf = types.union(&[and, countable]);

        assert_eq!(types.render(dnf), "(A&B)|Countable");
    }

    #[test]
    fn relative_and_builtin_types_render_as_written() {
        let mut types = TypeTable::new();
        let never = types.builtin(BuiltinTy::Never);
        let static_ty = types.relative(RelativeTy::Static);

        assert_eq!(types.render(never), "never");
        assert_eq!(types.render(static_ty), "static");
    }

    /// The shape that made [`TypeTable::render_relative`] necessary: `@return list<static>` on
    /// `UnitEnum`, which every backed enum's `cases()` inherits. Substituting the outermost node
    /// only left the word in the answer.
    #[test]
    fn a_relative_type_is_substituted_wherever_it_is_nested() {
        let mut interner = Interner::new();
        let mut types = TypeTable::new();
        let array = types.builtin(BuiltinTy::Array);
        let static_ty = types.relative(RelativeTy::Static);
        let self_ty = types.relative(RelativeTy::SelfTy);
        let list_of_static = types.generic(array, &[static_ty]);
        let nullable_self = types.nullable(self_ty);

        let name = |relative: RelativeTy| match relative {
            RelativeTy::Static => Some("Stage".to_owned()),
            RelativeTy::SelfTy => Some("Base".to_owned()),
            RelativeTy::Parent => None,
        };

        assert_eq!(
            types.render_relative(list_of_static, false, &name),
            "Stage[]"
        );
        assert_eq!(types.render_relative(nullable_self, false, &name), "?Base");
        // Untouched: the value the caller had nothing to say about keeps the author's word, and
        // every other node renders as it always did.
        let parent = types.relative(RelativeTy::Parent);
        let user = types.named(interner.name("App\\Entity\\User"));
        assert_eq!(types.render_relative(parent, false, &name), "parent");
        assert_eq!(types.render_relative(user, false, &name), "User");
    }
}
