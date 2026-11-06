//! One arm per shape that writes a name, and what that name is.
//!
//! Two of the arms are blind: [`Node::Identifier`] fires wherever a class-like name is written, and
//! [`Node::DirectVariable`] fires on every variable. Both also fire underneath a more specific arm —
//! the callee of a call is an identifier, and a parameter is a variable — and the specific answer
//! wins by rank rather than by either arm knowing about the other. See [`TokenKind::rank`].

use mago_span::HasSpan;
use mago_syntax::cst::{
    ClassLikeConstantSelector, ClassLikeMemberSelector, EnumCaseItem, Expression, Extends,
    Identifier, Implements, Node, PropertyItem, TokenSeparatedSequence, TraitUseMethodReference,
    UseItem, UseType, Variable,
};

use super::{TokenKind, TokenModifiers, names};

/// What `node` itself says about the names written in it.
pub(super) fn from<'a>(
    node: Node<'_, 'a>,
    parameters: &[&str],
    source: &str,
    found: &mut Vec<super::SemanticToken>,
) {
    match node {
        // --- class-like declarations, and the names in their headers ---
        Node::Class(class) => {
            names::plain(
                class.name.span,
                TokenKind::Class,
                TokenModifiers::of_declaration(&class.modifiers),
                found,
            );
            supertypes(
                class.extends.as_ref(),
                class.implements.as_ref(),
                source,
                found,
            );
        }
        Node::Interface(interface) => {
            names::plain(
                interface.name.span,
                TokenKind::Interface,
                TokenModifiers::DECLARATION,
                found,
            );
            // What an interface extends is an interface. The keyword settles it, which is more than
            // can be said at a use site.
            for name in interface
                .extends
                .iter()
                .flat_map(|extends| extends.types.iter())
            {
                used(name, TokenKind::Interface, source, found);
            }
        }
        // A trait is class-like and the protocol has no name for it.
        Node::Trait(r#trait) => names::plain(
            r#trait.name.span,
            TokenKind::Class,
            TokenModifiers::DECLARATION,
            found,
        ),
        Node::Enum(r#enum) => {
            names::plain(
                r#enum.name.span,
                TokenKind::Enum,
                TokenModifiers::DECLARATION,
                found,
            );
            supertypes(None, r#enum.implements.as_ref(), source, found);
        }
        Node::AnonymousClass(class) => supertypes(
            class.extends.as_ref(),
            class.implements.as_ref(),
            source,
            found,
        ),
        Node::TraitUse(r#use) => {
            for name in r#use.trait_names.iter() {
                used(name, TokenKind::Class, source, found);
            }
        }

        // --- member and function declarations ---
        Node::Method(method) => names::plain(
            method.name.span,
            TokenKind::Method,
            TokenModifiers::of_declaration(&method.modifiers),
            found,
        ),
        Node::Function(function) => names::plain(
            function.name.span,
            TokenKind::Function,
            TokenModifiers::DECLARATION,
            found,
        ),
        // `get` and `set`. Function-like enough that colouring them as methods reads correctly.
        Node::PropertyHook(hook) => names::plain(
            hook.name.span,
            TokenKind::Method,
            TokenModifiers::DECLARATION,
            found,
        ),
        Node::PlainProperty(property) => {
            let modifiers = TokenModifiers::of_declaration(&property.modifiers);
            for item in property.items.iter() {
                declared_property(item, modifiers, found);
            }
        }
        Node::HookedProperty(property) => declared_property(
            &property.item,
            TokenModifiers::of_declaration(&property.modifiers),
            found,
        ),
        Node::ClassLikeConstant(constant) => {
            // A class constant is static and cannot be written to, whether or not it says so.
            let modifiers = TokenModifiers {
                is_static: true,
                is_readonly: true,
                ..TokenModifiers::of_declaration(&constant.modifiers)
            };
            for item in constant.items.iter() {
                names::plain(item.name.span, TokenKind::Property, modifiers, found);
            }
        }
        Node::EnumCase(case) => {
            let name = match &case.item {
                EnumCaseItem::Unit(item) => item.name.span,
                EnumCaseItem::Backed(item) => item.name.span,
            };
            names::plain(
                name,
                TokenKind::EnumMember,
                TokenModifiers::DECLARATION,
                found,
            );
        }
        // A namespace-level `const`. The protocol has no constant kind, so it is a variable that
        // cannot be written to.
        Node::Constant(constant) => {
            for item in constant.items.iter() {
                names::plain(
                    item.name.span,
                    TokenKind::Variable,
                    TokenModifiers {
                        is_readonly: true,
                        ..TokenModifiers::DECLARATION
                    },
                    found,
                );
            }
        }
        // Promoted or not, this is the parameter list, so it is reported as a parameter. The
        // property it also declares has no separate range to report.
        Node::FunctionLikeParameter(parameter) => names::plain(
            parameter.variable.span,
            TokenKind::Parameter,
            TokenModifiers::of_declaration(&parameter.modifiers),
            found,
        ),
        Node::Namespace(namespace) => {
            // The last segment as well, which is what stops this from overlapping the blind
            // identifier arm: that arm splits the same name and calls the tail a class.
            if let Some(name) = &namespace.name {
                names::qualified(
                    name.span(),
                    source,
                    TokenKind::Namespace,
                    TokenModifiers::default(),
                    found,
                );
            }
        }

        // --- imports, where the keyword says which symbol table a name is in ---
        // Every one of the four shapes a `use` statement's items can take has an arm, so that the
        // alias -- which is a plain name the blind arm never sees -- always gets one too.
        Node::UseItemSequence(sequence) => imported(&sequence.items, None, source, found),
        Node::TypedUseItemSequence(sequence) => {
            imported(&sequence.items, Some(&sequence.r#type), source, found)
        }
        Node::TypedUseItemList(list) => {
            group_namespace(&list.namespace, source, found);
            imported(&list.items, Some(&list.r#type), source, found);
        }
        // Untyped items, but the prefix in front of the brace still needs saying.
        Node::MixedUseItemList(list) => group_namespace(&list.namespace, source, found),
        Node::MaybeTypedUseItem(item) => used_as(&item.item, item.r#type.as_ref(), source, found),

        // --- member access ---
        Node::PropertyAccess(access) => member(
            &access.property,
            TokenKind::Property,
            TokenModifiers::default(),
            found,
        ),
        Node::NullSafePropertyAccess(access) => member(
            &access.property,
            TokenKind::Property,
            TokenModifiers::default(),
            found,
        ),
        Node::MethodCall(call) => member(
            &call.method,
            TokenKind::Method,
            TokenModifiers::default(),
            found,
        ),
        Node::NullSafeMethodCall(call) => member(
            &call.method,
            TokenKind::Method,
            TokenModifiers::default(),
            found,
        ),
        Node::StaticMethodCall(call) => member(
            &call.method,
            TokenKind::Method,
            TokenModifiers {
                is_static: true,
                ..TokenModifiers::default()
            },
            found,
        ),
        Node::StaticPropertyAccess(access) => {
            if let Variable::Direct(variable) = &access.property {
                names::plain(
                    variable.span,
                    TokenKind::Property,
                    TokenModifiers {
                        is_static: true,
                        ..TokenModifiers::default()
                    },
                    found,
                );
            }
        }
        // `Foo::BAR` is a class constant or an enum case and syntax cannot tell which. Both are
        // static and neither can be written to, so that much is said and the kind is the one they
        // share.
        Node::ClassConstantAccess(access) => {
            if let ClassLikeConstantSelector::Identifier(name) = &access.constant {
                names::plain(
                    name.span,
                    TokenKind::Property,
                    TokenModifiers {
                        is_static: true,
                        is_readonly: true,
                        ..TokenModifiers::default()
                    },
                    found,
                );
            }
        }

        // --- names in use ---
        Node::FunctionCall(call) => {
            if let Expression::Identifier(name) = call.function {
                used(name, TokenKind::Function, source, found);
            }
        }
        // `$this->read(...)`, `Repo::find(...)`, `strlen(...)`: PHP 8.1's first-class callable.
        // Its own node rather than a call with an odd argument, so none of the call arms fire on
        // it and the name went out plain -- the one place a *method* is written and drawn as
        // prose. The syntax exists to be passed around, so it appears where a reader is scanning
        // for what is being passed.
        Node::MethodPartialApplication(made) => member(
            &made.method,
            TokenKind::Method,
            TokenModifiers::default(),
            found,
        ),
        Node::StaticMethodPartialApplication(made) => member(
            &made.method,
            TokenKind::Method,
            TokenModifiers {
                is_static: true,
                ..TokenModifiers::default()
            },
            found,
        ),
        Node::FunctionPartialApplication(made) => {
            if let Expression::Identifier(name) = made.function {
                used(name, TokenKind::Function, source, found);
            }
        }
        // `use Logs { log as writeLine; insteadof Other; }`. Every word of that line but these
        // was already coloured, so the two method names read as the only prose in a declaration.
        // Matched on the adaptation rather than on its two shapes, because the walk hands over the
        // enum and never the arms inside it -- and the alias is a *declaration*, since that name
        // exists nowhere else in the file.
        Node::TraitUseAdaptation(adaptation) => match adaptation {
            mago_syntax::cst::TraitUseAdaptation::Alias(alias) => {
                trait_method(&alias.method_reference, source, found);
                if let Some(named) = &alias.alias {
                    names::plain(
                        named.span,
                        TokenKind::Method,
                        TokenModifiers::DECLARATION,
                        found,
                    );
                }
            }
            mago_syntax::cst::TraitUseAdaptation::Precedence(precedence) => {
                used(
                    &precedence.method_reference.trait_name,
                    TokenKind::Class,
                    source,
                    found,
                );
                names::plain(
                    precedence.method_reference.method_name.span,
                    TokenKind::Method,
                    TokenModifiers::default(),
                    found,
                );
            }
        },
        // A bare name read as a value: `PHP_EOL`, or a global constant.
        Node::ConstantAccess(access) => used_with(
            &access.name,
            TokenKind::Variable,
            TokenModifiers {
                is_readonly: true,
                ..TokenModifiers::default()
            },
            source,
            found,
        ),
        Node::Attribute(attribute) => used(&attribute.name, TokenKind::Decorator, source, found),
        // `new Definition(label: 'X')`. The label names the callee's parameter, which is what the
        // protocol's `parameter` is for -- and it is the one name in a call that no other arm
        // reaches, since the value beside it is an expression of its own and the label is neither
        // an identifier expression nor a variable. Measured on 40 real files: 504 identifiers
        // drawn as plain text, nearly all of them these.
        Node::NamedArgument(argument) => names::plain(
            argument.name.span,
            TokenKind::Parameter,
            TokenModifiers::default(),
            found,
        ),
        Node::DirectVariable(variable) => {
            let name = names::variable_name(variable.name);
            let kind = match name {
                // `$this` is a keyword in everything but the grammar. It used to be skipped, on the
                // reasoning that an editor already draws it better -- which stopped being true the
                // moment this list could be asked to *replace* the grammar, and left `$this` grey.
                "this" => TokenKind::Keyword,
                _ if parameters.contains(&name) => TokenKind::Parameter,
                _ => TokenKind::Variable,
            };
            names::plain(variable.span, kind, TokenModifiers::default(), found);
        }
        // The blind arm. Every class-like name in the language reaches it -- type hints, `extends`,
        // `new`, `instanceof`, the left side of `::`, `catch`, imports -- and none of them can say
        // more than "class" without the index.
        Node::Identifier(name) => used(name, TokenKind::Class, source, found),

        // `declare(strict_types=1)`. The directive name is an identifier to a lexer and is written at
        // the top of nearly every modern PHP file, so leaving it plain was the most visible gap
        // measurement found -- one uncoloured word on the first line a reader looks at.
        Node::DeclareItem(item) => names::plain(
            item.name.span,
            TokenKind::Keyword,
            TokenModifiers::default(),
            found,
        ),

        // --- builtin types in a type position ---
        //
        // A type, not a keyword, and only the tree can say so. Half of these -- `int`, `string`,
        // `void` -- are not reserved words in PHP at all, so a lexer sees plain identifiers; the
        // other half -- `array`, `callable`, `static`, `null` -- are reserved, and the lexer calls
        // them keywords wherever they appear. Both halves are listed, because emitting the second
        // is what overrides the lexer: `Type` outranks `Keyword`, so the ranges agreeing settles it.
        //
        // `Keyword` is what this used to say, and it made `function f(int $n): string` colour `int`
        // and `string` exactly like `function` and `return`. Both other tools that colour PHP
        // disagree -- tree-sitter captures `(primitive_type)` as `@type.builtin`, phpantom emits its
        // `TYPE` -- and they are right: a reader scanning a signature is looking for the types.
        //
        // A named hint is not here. `Hint::Identifier` is a class-like name, the blind identifier
        // arm already reports it as a class, and that says more than `type` does.
        Node::Hint(hint) => {
            use mago_syntax::cst::Hint;

            // Two arms because mago spells them with two node types: the ones PHP does not reserve
            // arrive as identifiers, the reserved ones as keywords. Same span, same answer.
            let span = match hint {
                Hint::Void(word)
                | Hint::Never(word)
                | Hint::Float(word)
                | Hint::Bool(word)
                | Hint::Integer(word)
                | Hint::String(word)
                | Hint::Object(word)
                | Hint::Mixed(word)
                | Hint::Iterable(word) => Some(word.span),
                Hint::Array(word)
                | Hint::Callable(word)
                | Hint::Null(word)
                | Hint::True(word)
                | Hint::False(word)
                | Hint::Static(word)
                | Hint::Self_(word)
                | Hint::Parent(word) => Some(word.span),
                _ => None,
            };
            if let Some(span) = span {
                names::plain(span, TokenKind::Type, TokenModifiers::default(), found);
            }
        }

        _ => {}
    }
}

/// The prefix in front of a group import's brace.
///
/// A namespace to its last segment, which the blind identifier arm would call a class. Emitting the
/// tail as a namespace is what overrides it: the ranges match, so the ranking decides, rather than
/// two tokens overlapping.
fn group_namespace(
    namespace: &Identifier<'_>,
    source: &str,
    found: &mut Vec<super::SemanticToken>,
) {
    names::qualified(
        namespace.span(),
        source,
        TokenKind::Namespace,
        TokenModifiers::default(),
        found,
    );
}

/// `extends` as classes and `implements` as interfaces.
fn supertypes(
    extends: Option<&Extends<'_>>,
    implements: Option<&Implements<'_>>,
    source: &str,
    found: &mut Vec<super::SemanticToken>,
) {
    for name in extends.into_iter().flat_map(|extends| extends.types.iter()) {
        used(name, TokenKind::Class, source, found);
    }
    for name in implements
        .into_iter()
        .flat_map(|implements| implements.types.iter())
    {
        used(name, TokenKind::Interface, source, found);
    }
}

/// The variable a property declaration declares.
fn declared_property(
    item: &PropertyItem<'_>,
    modifiers: TokenModifiers,
    found: &mut Vec<super::SemanticToken>,
) {
    let variable = match item {
        PropertyItem::Abstract(item) => item.variable.span,
        PropertyItem::Concrete(item) => item.variable.span,
    };
    names::plain(variable, TokenKind::Property, modifiers, found);
}

/// A member selector, when it is written as a name rather than as an expression.
fn member(
    selector: &ClassLikeMemberSelector<'_>,
    kind: TokenKind,
    modifiers: TokenModifiers,
    found: &mut Vec<super::SemanticToken>,
) {
    if let ClassLikeMemberSelector::Identifier(name) = selector {
        names::plain(name.span, kind, modifiers, found);
    }
}

/// The method a trait adaptation names, in either spelling: `log` or `Logs::log`.
fn trait_method(
    reference: &TraitUseMethodReference<'_>,
    source: &str,
    found: &mut Vec<super::SemanticToken>,
) {
    match reference {
        TraitUseMethodReference::Identifier(name) => names::plain(
            name.span,
            TokenKind::Method,
            TokenModifiers::default(),
            found,
        ),
        TraitUseMethodReference::Absolute(absolute) => {
            used(&absolute.trait_name, TokenKind::Class, source, found);
            names::plain(
                absolute.method_name.span,
                TokenKind::Method,
                TokenModifiers::default(),
                found,
            );
        }
    }
}

fn imported(
    items: &TokenSeparatedSequence<'_, UseItem<'_>>,
    r#type: Option<&UseType<'_>>,
    source: &str,
    found: &mut Vec<super::SemanticToken>,
) {
    for item in items.iter() {
        used_as(item, r#type, source, found);
    }
}

/// An imported name, in the symbol table the `use` keyword named.
fn used_as(
    item: &UseItem<'_>,
    r#type: Option<&UseType<'_>>,
    source: &str,
    found: &mut Vec<super::SemanticToken>,
) {
    let (kind, modifiers) = match r#type {
        Some(UseType::Function(_)) => (TokenKind::Function, TokenModifiers::default()),
        Some(UseType::Const(_)) => (
            TokenKind::Variable,
            TokenModifiers {
                is_readonly: true,
                ..TokenModifiers::default()
            },
        ),
        None => (TokenKind::Class, TokenModifiers::default()),
    };

    // For an untyped import this repeats what the blind arm already said, and the duplicate is
    // dropped. It is written anyway so that the alias below does not need a second rule.
    used_with(&item.name, kind, modifiers, source, found);

    // The alias is the name the rest of the file writes, so it is the one that has to be right.
    if let Some(alias) = &item.alias {
        names::plain(alias.identifier.span, kind, modifiers, found);
    }
}

fn used(
    name: &Identifier<'_>,
    kind: TokenKind,
    source: &str,
    found: &mut Vec<super::SemanticToken>,
) {
    used_with(name, kind, TokenModifiers::default(), source, found);
}

fn used_with(
    name: &Identifier<'_>,
    kind: TokenKind,
    modifiers: TokenModifiers,
    source: &str,
    found: &mut Vec<super::SemanticToken>,
) {
    names::qualified(name.span(), source, kind, modifiers, found);
}
