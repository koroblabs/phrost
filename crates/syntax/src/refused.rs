//! Text mago parses and PHP refuses.
//!
//! The parser is deliberately more permissive than the language in places, and each of those places
//! is a file that does not load -- `php -l` says so and nothing else does. A language server that
//! stays quiet there is worse than one that has no opinion: the editor is green and the file is
//! dead.
//!
//! Each rule is asked of PHP 8.5.10 rather than read off an RFC, and each is gated on a substring
//! the shape cannot be written without. A file that does not hold the substring costs one scan of
//! its text and no walk at all, which is what makes this affordable on the indexing path.

use mago_span::HasSpan as _;
use mago_syntax::cst::{Access, BinaryOperator, DocumentIndentation, Expression, Node, Program};

use crate::{SyntaxError, text_range};

/// Every shape in this tree that PHP's own parser would reject.
pub(crate) fn refused_by_php(program: &Program<'_>, source: &str) -> Vec<SyntaxError> {
    let mut found = piped_arrow_functions(program, source);
    found.extend(instantiated_constants(program, source));
    found.extend(
        bare_instanceof_operands(program, source)
            .into_iter()
            .map(|range| SyntaxError {
                message: "The right-hand side of `instanceof` must be a class name or a variable; \
                          write anything else in parentheses"
                    .to_owned(),
                range,
            }),
    );
    found.extend(mixed_heredoc_indentation(program, source));
    found.sort_by_key(|error| error.range.start());
    found
}

/// `new X::CONST(...)`, which PHP refuses and mago parses.
///
/// What may name the class in a `new` is a name, a *variable* -- `$c`, `$h->m[0]`, `H::$m[0]` --
/// or a parenthesised expression. A class **constant** is none of those, so `new H::C[0]()` is
/// `syntax error, unexpected identifier "C", expecting variable or "$"` while `new (H::C[0])()`
/// parses. Every row of that was run against 8.5.10.
///
/// Worth its own rule because the parentheses are load-bearing and mago's *formatter* removes
/// them: reprinting one real file turned `new (self::SERVICERS[$id])($config)` into a syntax
/// error, and the formatter's own guard is that its output parses.
fn instantiated_constants(program: &Program<'_>, source: &str) -> Vec<SyntaxError> {
    if !source.contains("new ") || !source.contains("::") {
        return Vec::new();
    }

    Node::Program(program).filter_map(|node| {
        let Node::Instantiation(instantiation) = node else {
            return None;
        };
        // Through any subscripts, because `H::C[0]` is an index over the constant and PHP refuses
        // the whole of it for the same reason -- the thing at the root is not a variable.
        let mut class = instantiation.class;
        while let Expression::ArrayAccess(access) = class {
            class = access.array;
        }
        matches!(class, Expression::Access(Access::ClassConstant(_))).then(|| SyntaxError {
            message: "A class constant cannot name the class of a `new`; write it in parentheses"
                .to_owned(),
            range: text_range(instantiation.class.span()),
        })
    })
}

/// The right-hand sides of `instanceof` that PHP takes only in parentheses, written without them.
///
/// PHP reads that side as a class reference: a name, `static`, or a variable and what `[...]`,
/// `->`, `?->` and `::$` reach from one. Anything else -- a cast, a call, a literal, a class
/// constant -- it refuses unless it is parenthesised, and mago parses it either way. Every row was
/// run against 8.5.10.
///
/// Public through the parsed file, because mago's *formatter* drops those parentheses for all but a
/// class constant -- `$job instanceof ((string) $jobToFake)` in Laravel came back as a syntax
/// error -- and the formatter puts them back from this list rather than from a copy of the rule.
pub(crate) fn bare_instanceof_operands(
    program: &Program<'_>,
    source: &str,
) -> Vec<phrost_model::TextRange> {
    if !source.contains("instanceof") {
        return Vec::new();
    }
    Node::Program(program).filter_map(|node| {
        let Node::Binary(binary) = node else {
            return None;
        };
        (matches!(binary.operator, BinaryOperator::Instanceof(_))
            && !is_class_reference(binary.rhs))
        .then(|| text_range(binary.rhs.span()))
    })
}

/// `class_name_reference` in PHP's grammar: a name, or `new_variable`, or a parenthesised
/// expression.
fn is_class_reference(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Parenthesized(_)
        | Expression::Identifier(_)
        | Expression::Static(_)
        | Expression::Self_(_)
        | Expression::Parent(_) => true,
        _ => is_new_variable(expression),
    }
}

/// `new_variable`: a variable, and what `[...]`, `->`, `?->` and `::$` reach from one -- the last
/// also from a class name.
fn is_new_variable(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Variable(_) => true,
        Expression::ArrayAccess(access) => is_new_variable(access.array),
        Expression::Access(Access::Property(access)) => is_new_variable(access.object),
        Expression::Access(Access::NullSafeProperty(access)) => is_new_variable(access.object),
        Expression::Access(Access::StaticProperty(access)) => {
            matches!(
                access.class,
                Expression::Identifier(_)
                    | Expression::Static(_)
                    | Expression::Self_(_)
                    | Expression::Parent(_)
            ) || is_new_variable(access.class)
        }
        _ => false,
    }
}

/// A heredoc or nowdoc whose closing marker is indented with tabs and spaces together.
///
/// PHP refuses the file -- "Invalid indentation - tabs and spaces cannot be mixed" -- and mago
/// records the mix as `DocumentIndentation::Mixed` and goes on. mago's formatter wrote one: an
/// unindented heredoc in WordPress's `network.php`, in an `else :` after a `?>`, indented with the
/// tab the template text ended on and spaces after it.
fn mixed_heredoc_indentation(program: &Program<'_>, source: &str) -> Vec<SyntaxError> {
    if !source.contains("<<<") {
        return Vec::new();
    }
    Node::Program(program).filter_map(|node| {
        let Node::DocumentString(document) = node else {
            return None;
        };
        matches!(document.indentation, DocumentIndentation::Mixed(..)).then(|| SyntaxError {
            message: "Invalid indentation - tabs and spaces cannot be mixed".to_owned(),
            range: text_range(document.close),
        })
    })
}

/// `$x |> fn ($v) => f($v)`, which PHP refuses and mago parses.
fn piped_arrow_functions(program: &Program<'_>, source: &str) -> Vec<SyntaxError> {
    // `|>` is two bytes no file can hide it behind.
    if !source.contains("|>") {
        return Vec::new();
    }

    Node::Program(program).filter_map(|node| {
        let Node::Pipe(pipe) = node else {
            return None;
        };
        // PHP parses `$x |> fn ($v) => f($v)` by binding the arrow function's body across the
        // pipe, which is never what the writer meant, so it refuses the whole file rather than
        // guess: `Arrow functions on the right hand side of |> must be parenthesized`. mago binds
        // it the way a reader expects -- its own formatter writes the parentheses in -- and says
        // nothing.
        //
        // A closure is not in it: `$x |> function ($v) { ... }` has a brace to end on and parses.
        // Both were run.
        matches!(pipe.callable, Expression::ArrowFunction(_)).then(|| SyntaxError {
            message: "Arrow functions on the right hand side of |> must be parenthesized"
                .to_owned(),
            range: text_range(pipe.callable.span()),
        })
    })
}

#[cfg(test)]
mod tests {
    use expect_test::{Expect, expect};

    use crate::ParseSession;

    /// Every complaint about the file, the parser's and this module's together, since what a caller
    /// gets is one list.
    fn check(source: &str, expect: Expect) {
        let session = ParseSession::new();
        let parsed = session.parse(source);
        let rendered = parsed
            .errors()
            .iter()
            .map(|error| format!("{:?} {}\n", error.range, error.message))
            .collect::<String>();
        expect.assert_eq(match rendered.is_empty() {
            true => "(nothing wrong with this file)\n",
            false => &rendered,
        });
    }

    /// The one shape PHP 8.5.10 refuses and mago accepts. Its own formatter writes the parentheses
    /// in, which is the tell that it read the code the way the author meant and the language does
    /// not.
    #[test]
    fn an_arrow_function_piped_into_without_parentheses_is_a_file_php_refuses() {
        check(
            "<?php\n\n$r = 1 |> fn ($x) => $x + 1;\n",
            expect![[r#"
                17..34 Arrow functions on the right hand side of |> must be parenthesized
            "#]],
        );
    }

    /// `static fn` is the same shape wearing a modifier, and PHP refuses it with the same words.
    #[test]
    fn a_static_arrow_function_is_refused_the_same_way() {
        check(
            "<?php\n\n$r = 1 |> static fn ($x) => $x + 1;\n",
            expect![[r#"
                17..41 Arrow functions on the right hand side of |> must be parenthesized
            "#]],
        );
    }

    /// And the four right-hand sides PHP accepts, which must stay quiet. A closure has a brace to
    /// end on, so it needs no parentheses; a first-class callable and a plain callable string are
    /// values already.
    #[test]
    fn every_right_hand_side_php_accepts_is_left_alone() {
        check(
            "<?php\n\n$a = 1 |> (fn ($x) => $x + 1);\n\
             $b = 1 |> function ($x) { return $x; };\n\
             $c = 1 |> static function ($x) { return $x; };\n\
             $d = 1 |> strval(...);\n",
            expect![[r#"
                (nothing wrong with this file)
            "#]],
        );
    }

    /// `new X::CONST(...)`, whose parentheses are load-bearing and which mago's own formatter
    /// removes -- turning one real file into a syntax error on save.
    #[test]
    fn a_class_constant_naming_the_class_of_a_new_is_refused() {
        check(
            "<?php\n\nclass H { const C = ['stdClass'];\n  function m() { return new self::C[0](); } }\n",
            expect![[r#"
                69..79 A class constant cannot name the class of a `new`; write it in parentheses
            "#]],
        );
    }

    /// And every class reference PHP accepts: a name, a variable, a property chain, a static
    /// property, a call, and the parenthesised form the one above is missing. All six were run.
    #[test]
    fn every_class_reference_php_accepts_is_left_alone() {
        check(
            "<?php\n\nclass H { const C = ['stdClass']; public array $m = []; public static array $s = [];\n             function m(string $c) {\n             $a = new H();\n             $b = new $c();\n             $d = new $this->m[0]();\n             $e = new H::$s[0]();\n             $f = new (self::C[0])();\n             $g = new (strval(1))();\n             } }\n",
            expect![[r#"
                (nothing wrong with this file)
            "#]],
        );
    }

    /// A cast, a call and a literal on the right of `instanceof` are refused without parentheses,
    /// as PHP refuses them; every class reference it accepts is left alone. All rows were run.
    #[test]
    fn an_instanceof_operand_php_reads_only_in_parentheses_is_refused() {
        check(
            "<?php\nclass H { static $s; }\n$a = $j instanceof (string) $c;\n$b = $j instanceof get_class($j);\n$c = $j instanceof 'Foo';\n$d = $j instanceof $c[0] || $j instanceof $o->c || $j instanceof $o?->c\n    || $j instanceof H::$s || $j instanceof $c::$s || $j instanceof static\n    || $j instanceof ((string) $c) || $j instanceof H;\n",
            expect![[r#"
                48..59 The right-hand side of `instanceof` must be a class name or a variable; write anything else in parentheses
                80..93 The right-hand side of `instanceof` must be a class name or a variable; write anything else in parentheses
                114..119 The right-hand side of `instanceof` must be a class name or a variable; write anything else in parentheses
            "#]],
        );
    }

    /// A closing marker indented with a tab and spaces together, which PHP refuses.
    #[test]
    fn a_heredoc_indented_with_tabs_and_spaces_is_refused() {
        check(
            "<?php\n$x = <<<EOF\n  \tline\n  \tEOF;\n$y = <<<EOF\n\t\tline\n\t\tEOF;\n",
            expect![[r#"
                26..32 Invalid indentation - tabs and spaces cannot be mixed
            "#]],
        );
    }

    /// A file with no `|>` in it never reaches the walk, which is what makes the rule affordable on
    /// the indexing path. Asserted through the answer rather than the gate, because the gate is an
    /// implementation detail and the silence is the contract.
    #[test]
    fn an_arrow_function_that_is_not_piped_into_is_not_a_pipe() {
        check(
            "<?php\n\n$f = fn ($x) => $x + 1;\n$r = $f(1);\n",
            expect![[r#"
                (nothing wrong with this file)
            "#]],
        );
    }
}
