//! Block directives that do not pair up.
//!
//! Blade compiles each directive on its own, so `@foreach (...) ... @endif` becomes a `foreach:`
//! ended by `endif;`, which PHP refuses to parse, and a block nobody closes leaves the compiled
//! template unparseable: the page fails, and the error names a line of the cached PHP rather than
//! the template. Paired by what each half compiles to rather than by name -- `@auth ... @endif` is
//! valid, both halves being an `if` -- so that only what PHP refuses is reported.

use std::ops::Range;

use super::Masker;

/// The control structure a block directive compiles to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Structure {
    If,
    Foreach,
    /// `@forelse`, whose loop `@empty` ends and whose `if` `@endforelse` does.
    Forelse,
    For,
    While,
    Switch,
}

/// One block open: what it compiles to, where its directive is, how it was written, and -- for a
/// `@forelse` -- whether its `@empty` has been read.
struct Open {
    structure: Structure,
    at: Range<usize>,
    name: String,
    emptied: bool,
}

/// What does not pair in the template: where, and what is wrong there.
pub(crate) fn problems(template: &str) -> Vec<(Range<usize>, String)> {
    let masker = Masker::masked(template);
    let mut found = masker.unended;
    let mut open: Vec<Open> = Vec::new();
    for (at, name, arguments) in masker.directives {
        if let Some(structure) = opens(&name, arguments) {
            open.push(Open {
                structure,
                at,
                name,
                emptied: false,
            });
            continue;
        }
        match name.as_str() {
            // Without arguments, the turn of a `@forelse` from its loop to its empty case.
            "empty" => {
                match open.last_mut() {
                    Some(block) if block.structure == Structure::Forelse && !block.emptied => {
                        block.emptied = true;
                    }
                    _ => found.push((at, "`@empty` is not inside a `@forelse`".to_owned())),
                }
                continue;
            }
            "else" | "elseif" | "elsecan" | "elsecannot" | "elsecanany" | "elseauth"
            | "elseguest" => {
                if open
                    .last()
                    .is_none_or(|block| block.structure != Structure::If)
                {
                    found.push((at, format!("`@{name}` is not inside an `@if`")));
                }
                continue;
            }
            _ => {}
        }
        let Some(structure) = closes(&name) else {
            continue;
        };
        // The block this closer is named for first, and only then any of the structure it
        // compiles to: `@endif` past an `@error` nobody closed ends the `@if` around it, and it is
        // the `@error` that is unclosed -- whatever PHP makes of the two.
        let Some(index) = open
            .iter()
            .rposition(|block| ends(&block.name) == name)
            .or_else(|| open.iter().rposition(|block| block.structure == structure))
        else {
            found.push((at, format!("`@{name}` ends no block that is open")));
            continue;
        };
        // What was opened inside it and not closed ends here unclosed: the closer reaches past it.
        for inner in open.drain(index + 1..) {
            found.push((inner.at, format!("`@{}` is never closed", inner.name)));
        }
        if let Some(block) = open.pop()
            && block.structure == Structure::Forelse
            && !block.emptied
        {
            found.push((block.at, "`@forelse` ends without an `@empty`".to_owned()));
        }
    }
    for block in open {
        found.push((block.at, format!("`@{}` is never closed", block.name)));
    }
    found.sort_by_key(|(at, _)| at.start);
    found
}

/// What a directive opens, if it opens a block: some only with a condition, some either way.
fn opens(name: &str, arguments: bool) -> Option<Structure> {
    Some(match name {
        "if" | "unless" | "isset" | "empty" | "hassection" | "sectionmissing" | "can"
        | "cannot" | "canany" | "env" | "error" | "session" | "context"
            if arguments =>
        {
            Structure::If
        }
        "auth" | "guest" | "production" | "once" => Structure::If,
        "foreach" if arguments => Structure::Foreach,
        "forelse" if arguments => Structure::Forelse,
        "for" if arguments => Structure::For,
        "while" if arguments => Structure::While,
        "switch" if arguments => Structure::Switch,
        _ => return None,
    })
}

/// The closer a block directive is written with.
fn ends(name: &str) -> String {
    match name {
        "hassection" | "sectionmissing" => "endif".to_owned(),
        _ => format!("end{name}"),
    }
}

/// What a directive ends, if it ends a block.
fn closes(name: &str) -> Option<Structure> {
    Some(match name {
        "endif" | "endunless" | "endisset" | "endempty" | "endcan" | "endcannot" | "endcanany"
        | "endenv" | "enderror" | "endsession" | "endcontext" | "endauth" | "endguest"
        | "endproduction" | "endonce" => Structure::If,
        "endforeach" => Structure::Foreach,
        "endforelse" => Structure::Forelse,
        "endfor" => Structure::For,
        "endwhile" => Structure::While,
        "endswitch" => Structure::Switch,
        _ => return None,
    })
}
