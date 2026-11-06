//! The methods a class may declare because something above it declared one first.
//!
//! Two questions, and they belong together because the second is only ever asked when the first
//! says yes: *is the cursor in a class body*, and *what could that class declare there*.
//!
//! # Why a declaration is sometimes refused
//!
//! PHP checks an override at class load, and a stub that does not fit is a fatal error before any
//! of the file runs. So the only method offered is one whose declaration this can write back
//! **exactly**, and two things in the index cannot be written back:
//!
//! - **A parameter's default.** The index stores that there was one, never what it was: a default
//!   is an expression, and expressions live with bodies -- which closed files do not keep. Making a
//!   formerly optional parameter required is a fatal error, so a method with one is not offered.
//! - **A docblock type.** `list<User>` and `array{id: int}` are claims about code, not type hints.
//!   Writing one into a signature produces a file that does not parse, so only the native
//!   declaration is read and a method typed only by its comment is offered without that type.
//!
//! Both refusals lose a method from a list. That is the failure this project prefers: a name a
//! person cannot find is recoverable by typing it, and a stub that stops the class from loading is
//! not.

use phrost_defmap::DeclarationSource;
use phrost_model::{Name, RelativeTy, TextSize, Visibility};
use phrost_sema::{ClassKind, MemberKind, MemberProvenance, MemberRow};
use phrost_ty::{TypeId, TypeList, TypeRef, TypeTable};

use crate::view::IndexView;

/// One method a class could declare in place of a supertype's.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OverridableMethod {
    pub name: Name,
    /// The class whose declaration this would replace.
    pub declared_by: Name,
    /// The visibility the replacement has to keep, or widen. Never narrowed: PHP refuses that.
    pub visibility: Visibility,
    pub is_static: bool,
    /// Whether the supertype left it to be written: an `abstract` method, or one an interface names.
    pub is_required: bool,
    /// The declaration from the name onwards, written so PHP can read it back:
    /// `setUp(): void`, `write(\App\Buffer &$into, string ...$parts): int`.
    ///
    /// Class names are fully qualified and anchored, because a stub is inserted into a file whose
    /// imports it knows nothing about and `\App\Buffer` means the same thing in all of them.
    pub signature: String,
    /// Whether the declaration being replaced says it is deprecated.
    pub deprecated: bool,
}

impl IndexView<'_> {
    /// The class whose body directly holds `offset` -- not one of its members, and not a class the
    /// offset only reaches through one.
    ///
    /// The distinction is the whole point: `{ $x = 1; se| }` inside a method and `{ se| }` inside a
    /// class body have the same characters before them, and only one of the two is a place a member
    /// declaration begins. Answered from the tree because that is where the ranges are; the text
    /// would have to count braces and cannot see into a string.
    pub fn class_body_at(&self, offset: TextSize) -> Option<Name> {
        let open = self.current()?;
        let hir = open.hir();

        let (id, class) = hir
            .classes()
            .filter(|(_, class)| class.full_range.contains_inclusive(offset))
            .min_by_key(|(_, class)| class.full_range.len())?;

        let inside_a_member = hir
            .member_list(class.members)
            .iter()
            .any(|&member| hir.member(member).full_range.contains_inclusive(offset));
        if inside_a_member {
            return None;
        }

        // The declared symbol rather than the written name, for the reason `enclosing_class` gives:
        // an anonymous class's written name is synthesised and the catalog has never heard of it.
        open.scope()
            .symbol_of(DeclarationSource::Class(id))
            .map(|symbol| symbol.fqn.clone())
    }

    /// Every method above `class_fqn` that it could declare instead, required ones first.
    ///
    /// Required first because they are the ones PHP will not load the class without; alphabetical
    /// within each half, so the order does not move when an unrelated supertype gains a method.
    ///
    /// `wanted` narrows by name before anything is rendered. Given as a test rather than as a
    /// prefix because what a typed prefix matches is the feature layer's rule -- and it has to be
    /// applied *here*, since building a declaration is the whole cost of this and a supertype chain
    /// on a real project is a couple of hundred methods deep.
    pub fn overridable_methods(
        &self,
        class_fqn: &str,
        wanted: impl Fn(&str) -> bool,
    ) -> Vec<OverridableMethod> {
        // An interface declares signatures and no bodies, so what it could write is a different
        // shape entirely -- and an interface restating a method it already inherits is unusual
        // enough that a second stub shape would be built for almost nobody.
        if self
            .class_header(class_fqn)
            .is_none_or(|header| header.kind == ClassKind::Interface)
        {
            return Vec::new();
        }

        let types = self.skeleton.types();
        let mut found = self
            .members(class_fqn)
            .members()
            .iter()
            .filter_map(|member| {
                let declared_by = match &member.provenance {
                    MemberProvenance::Inherited(fqn) | MemberProvenance::Interface(fqn) => fqn,
                    // The class writes it already, or a trait wrote it in -- and PHP treats a
                    // trait's method as the class's own. Declaring either again is a redeclaration.
                    MemberProvenance::Own | MemberProvenance::Trait(_) => return None,
                };
                let row = &member.row;
                // `final` cannot be replaced; `private` was never inherited, so a child declaring
                // the name is not overriding anything and PHP checks nothing about it; and an
                // `@method` tag is a claim about generated code rather than a declaration to fit.
                // A `@mixin` forwards calls and declares nothing above the class, which is the same.
                if row.kind != MemberKind::Method
                    || row.from_docblock
                    || member.forwarded
                    || row.modifiers.is_final()
                    || member.visibility == Visibility::Private
                    || !wanted(member.name.as_str())
                {
                    return None;
                }

                Some(OverridableMethod {
                    signature: writable_signature(member.name.as_str(), row, types)?,
                    name: member.name.clone(),
                    declared_by: declared_by.clone(),
                    visibility: member.visibility,
                    is_static: row.modifiers.is_static(),
                    is_required: row.modifiers.is_abstract()
                        || matches!(member.provenance, MemberProvenance::Interface(_)),
                    deprecated: row.modifiers.is_deprecated(),
                })
            })
            .collect::<Vec<_>>();

        found.sort_by(|left, right| {
            right
                .is_required
                .cmp(&left.is_required)
                .then_with(|| left.name.as_str().cmp(right.name.as_str()))
        });
        found
    }
}

/// The declaration as PHP would have to read it back, or `None` when the index cannot say.
///
/// See this module's header for the two things it cannot: a parameter's default, and a type only a
/// docblock declared.
fn writable_signature(name: &str, row: &MemberRow, types: &TypeTable) -> Option<String> {
    let mut out = String::from(name);
    out.push('(');

    for (position, param) in row.params.iter().enumerate() {
        // Promotion writes a property as well as a parameter, and dropping it would leave a
        // constructor that quietly stops assigning one. Only a constructor carries any.
        if param.promotion.is_some() {
            return None;
        }
        // A default is written back from the source the index now keeps. It used to be the other
        // half of this refusal, and it is the half that bit: **21 % of the reference project's own
        // interfaces and 30 % of Drupal core's** declare at least one method with one, so a quarter
        // of every "implement the required methods" was a repair that stopped short. Dropping the
        // value instead was never an option -- an optional parameter made required is a class that
        // still does not load.
        if param.has_default && param.default.is_none() {
            return None;
        }
        if position > 0 {
            out.push_str(", ");
        }
        // Native only. A parameter the parent typed in a comment is written untyped here, which is
        // what the parent declared as far as the runtime is concerned.
        if let Some(ty) = param.ty {
            write_native(&mut out, types, ty, Position::Parameter)?;
            out.push(' ');
        }
        if param.by_reference {
            out.push('&');
        }
        if param.is_variadic {
            out.push_str("...");
        }
        out.push('$');
        out.push_str(param.name.as_str());
        if let Some(default) = &param.default {
            out.push_str(" = ");
            out.push_str(default);
        }
    }

    out.push(')');
    if let Some(ty) = row.ty {
        out.push_str(": ");
        write_native(&mut out, types, ty, Position::Return)?;
    }
    Some(out)
}

/// Where a type is being written, which is what decides whether `self` may stand.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Position {
    Parameter,
    Return,
}

/// Appends a type as PHP declares one, or gives up on anything only a docblock can write.
fn write_native(out: &mut String, types: &TypeTable, id: TypeId, position: Position) -> Option<()> {
    match types.get(id) {
        // A hint is never refined -- `non-empty-string` is a docblock's word -- but the spelling is
        // asked of the builtin anyway, so a refinement that ever reached here would still parse.
        TypeRef::Builtin(builtin) => out.push_str(builtin.native().as_str()),
        // A return type may keep the word: `self` in the child means the child, which is a subclass
        // of what the parent promised, and `static` means the same thing in both. A *parameter* may
        // not -- `self` there narrows what the method accepts, which PHP refuses to load -- and
        // `parent` means a different class in the child either way.
        TypeRef::Relative(relative) => match (position, relative) {
            (Position::Return, RelativeTy::SelfTy | RelativeTy::Static) => {
                out.push_str(relative.as_str());
            }
            _ => return None,
        },
        // Anchored, because the stub lands in a file whose imports this knows nothing about.
        TypeRef::Named(fqn) => {
            out.push('\\');
            out.push_str(fqn.as_str());
        }
        TypeRef::Nullable(inner) => match types.get(*inner) {
            // `?A|B` parses as `(?A)|B` and `?(A|B)` is not a type at all, so a nullable union has
            // exactly one spelling.
            TypeRef::Union(_) => {
                write_native(out, types, *inner, position)?;
                out.push_str("|null");
            }
            _ => {
                out.push('?');
                write_native(out, types, *inner, position)?;
            }
        },
        TypeRef::Union(operands) => {
            write_operands(out, types, *operands, Separator::Union, position)?
        }
        TypeRef::Intersection(operands) => {
            write_operands(out, types, *operands, Separator::Intersection, position)?;
        }
        // A docblock wrote it. `list<User>`, `array{id: int}` and `object{id: int}` are not type
        // hints, and a stub carrying one would not parse.
        TypeRef::Generic { .. }
        | TypeRef::Shape { .. }
        | TypeRef::ObjectShape { .. }
        | TypeRef::Field { .. } => return None,
    }
    Some(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Separator {
    Union,
    Intersection,
}

/// The operands of a composite, with the one nesting PHP allows and no other.
fn write_operands(
    out: &mut String,
    types: &TypeTable,
    operands: TypeList,
    separator: Separator,
    position: Position,
) -> Option<()> {
    for (at, &operand) in types.operands(operands).iter().enumerate() {
        if at > 0 {
            out.push(match separator {
                Separator::Union => '|',
                Separator::Intersection => '&',
            });
        }
        match (types.get(operand), separator) {
            // DNF, which is the only nesting a declaration may have: `(A&B)|C`, never the reverse.
            (TypeRef::Intersection(_), Separator::Union) => {
                out.push('(');
                write_native(out, types, operand, position)?;
                out.push(')');
            }
            (TypeRef::Union(_) | TypeRef::Intersection(_) | TypeRef::Nullable(_), _) => {
                return None;
            }
            _ => write_native(out, types, operand, position)?,
        }
    }
    Some(())
}
