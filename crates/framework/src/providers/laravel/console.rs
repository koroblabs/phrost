//! What a Laravel command's `$signature` declares, input by input.
//!
//! `Illuminate\Console\Parser`, restated: every `{...}` is one input, an option when it begins with
//! two dashes, and what follows its name says what it holds. What each spelling holds was read off
//! the runtime -- Laravel's parser building a definition, Symfony's input reading it back -- and
//! agrees with Larastan's: `{--queue=default}` is still nullable, because `--queue` written with no
//! value hands back null whatever the default says.

use phrost_model::{BuiltinTy, DeclaredInput, TextRange, ValueShape};

const TEXT: ValueShape = ValueShape {
    builtin: BuiltinTy::String,
    nullable: false,
    list: false,
};
const MAYBE_TEXT: ValueShape = ValueShape {
    nullable: true,
    ..TEXT
};
const TEXTS: ValueShape = ValueShape { list: true, ..TEXT };
const MAYBE_TEXTS: ValueShape = ValueShape {
    list: true,
    ..MAYBE_TEXT
};
const FLAG: ValueShape = ValueShape {
    builtin: BuiltinTy::Bool,
    ..TEXT
};

/// What every command has beside its signature: the application's own argument and options, as
/// Laravel 12 on Symfony 7.4 lists them, and the `--isolated` an `Isolatable` command is given.
/// Read off `Illuminate\Console\Application::getDefinition()` rather than written from memory --
/// `--silent` arrived with Symfony 7.2, and `--no-ansi` is the negation of `--ansi`.
pub(super) const APPLICATION_INPUTS: &[&str] = &[
    "command",
    "--help",
    "--silent",
    "--quiet",
    "--verbose",
    "--version",
    "--ansi",
    "--no-ansi",
    "--no-interaction",
    "--env",
    "--isolated",
];

/// Every input a signature declares: an argument under its name, an option under `--` and its name,
/// which is how the signature writes each -- with where the name is written, and what it is for.
pub(super) fn inputs(signature: &str) -> Vec<DeclaredInput> {
    let mut found = Vec::new();
    let mut rest = signature;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            break;
        };
        // Laravel matches `\{\s*(.*?)\s*\}`, whose `.` stops at a line end: an input written across
        // two lines is not one it reads, and is not one here.
        let token = after[..close].trim();
        rest = &after[close + 1..];
        if token.contains('\n') {
            continue;
        }
        let (token, description) = described(token);
        let (name, shape, prefix) = match token.strip_prefix("--") {
            Some(option) => {
                let (name, shape) = self::option(option.trim_start_matches('-'));
                (name, shape, "--")
            }
            None => {
                let (name, shape) = argument(token);
                (name, shape, "")
            }
        };
        // Every slice above is of the signature itself, so where the name sits is where it was cut.
        let Some(at) = signature.substr_range(name) else {
            continue;
        };
        let (Ok(start), Ok(end)) = (u32::try_from(at.start), u32::try_from(at.end)) else {
            continue;
        };
        found.push(DeclaredInput {
            name: format!("{prefix}{name}"),
            shape,
            range: TextRange::new(start.into(), end.into()),
            description: description.map(str::to_owned),
        });
    }
    found
}

/// `user : The user to mail` as the input and its description, which follows the first colon with
/// space on both sides.
fn described(token: &str) -> (&str, Option<&str>) {
    let bytes = token.as_bytes();
    let separator = (1..bytes.len().saturating_sub(1)).find(|&at| {
        bytes[at] == b':'
            && bytes[at - 1].is_ascii_whitespace()
            && bytes[at + 1].is_ascii_whitespace()
    });
    match separator {
        Some(at) => {
            let description = token[at + 1..].trim();
            (
                token[..at].trim_end(),
                (!description.is_empty()).then_some(description),
            )
        }
        None => (token, None),
    }
}

/// `user`, `user?`, `ids*`, `ids?*`, `name=default`, `ids=*1,2`.
fn argument(token: &str) -> (&str, ValueShape) {
    if token.ends_with("?*") || token.ends_with('*') {
        return (token.trim_matches(['?', '*']), TEXTS);
    }
    if token.ends_with('?') {
        return (token.trim_matches('?'), MAYBE_TEXT);
    }
    if let Some((name, _)) = defaulted(token, "=*") {
        return (name, TEXTS);
    }
    // A default makes the argument optional and is a string itself, so a string comes back either way.
    if let Some((name, _)) = defaulted(token, "=") {
        return (name, TEXT);
    }
    (token, TEXT)
}

/// `force`, `queue=`, `ids=*`, `queue=default`, `ids=*1,2`, each optionally after a `Q|` shortcut --
/// the name without its dashes.
fn option(token: &str) -> (&str, ValueShape) {
    let token = match token.split_once('|') {
        Some((_, name)) => name.trim_start(),
        None => token,
    };
    if token.ends_with('=') {
        return (token.trim_matches('='), MAYBE_TEXT);
    }
    if token.ends_with("=*") {
        return (token.trim_matches(['=', '*']), MAYBE_TEXTS);
    }
    if let Some((name, _)) = defaulted(token, "=*") {
        return (name, MAYBE_TEXTS);
    }
    if let Some((name, _)) = defaulted(token, "=") {
        return (name, MAYBE_TEXT);
    }
    (token, FLAG)
}

/// `(.+)<separator>(.+)` as PHP matches it: the first group greedy, so the split is at the last
/// separator that leaves something on both sides.
fn defaulted<'t>(token: &'t str, separator: &str) -> Option<(&'t str, &'t str)> {
    let at = token.rfind(separator)?;
    let (name, default) = (&token[..at], &token[at + separator.len()..]);
    (!name.is_empty() && !default.is_empty()).then_some((name, default))
}

#[cfg(test)]
mod tests {
    use expect_test::{Expect, expect};

    fn check(signature: &str, expected: Expect) {
        let rendered = super::inputs(signature)
            .into_iter()
            .map(|input| format!("{}: {}\n", input.name, input.shape))
            .collect::<String>();
        expected.assert_eq(&rendered);
    }

    #[test]
    fn every_spelling_of_an_argument_and_an_option_holds_what_the_runtime_hands_back() {
        // Each line checked against Laravel's parser and Symfony's input on PHP 8.5.10: `--dval`
        // written alone hands back null although a default is declared, and `--arr` alone appends
        // a null to the list.
        check(
            "demo {req} {opt?} {def=fallback} {many*} {maybe?*} {listed=*a,b} {--flag} {--val=} \
             {--dval=fallback} {--arr=*} {--darr=*a,b} {--Q|queue=}",
            expect![[r#"
                req: string
                opt: ?string
                def: string
                many: list<string>
                maybe: list<string>
                listed: list<string>
                --flag: bool
                --val: ?string
                --dval: ?string
                --arr: list<?string>
                --darr: list<?string>
                --queue: ?string
            "#]],
        );
    }

    #[test]
    fn a_description_and_the_space_around_a_token_are_not_part_of_its_name() {
        check(
            "mail:send
                {user : The ID of the user}
                { --queue= : Whether the job should be queued }
                {--id=* : The ids: several}",
            expect![[r#"
                user: string
                --queue: ?string
                --id: list<?string>
            "#]],
        );
    }

    /// Where each name is written, which is what a read of it goes to and a rename moves: the name
    /// alone, without the dashes, the shortcut, the modifiers or the default.
    #[test]
    fn each_name_is_found_where_the_signature_writes_it() {
        let signature = "mail:send {user : The ID of the user} {ids?*} {--Q|queue=default : Where}
            {--force} {--id=*1,2}";
        let rendered = super::inputs(signature)
            .into_iter()
            .map(|input| {
                let at = std::ops::Range::<usize>::from(input.range);
                format!(
                    "{} at {at:?} `{}`{}\n",
                    input.name,
                    &signature[at.clone()],
                    input
                        .description
                        .map(|said| format!(" -- {said}"))
                        .unwrap_or_default()
                )
            })
            .collect::<String>();
        expect![[r#"
            user at 11..15 `user` -- The ID of the user
            ids at 39..42 `ids`
            --queue at 51..56 `queue` -- Where
            --force at 89..94 `force`
            --id at 99..101 `id`
        "#]]
        .assert_eq(&rendered);
    }

    #[test]
    fn a_signature_with_no_inputs_declares_none() {
        check("inspire", expect![""]);
    }
}
