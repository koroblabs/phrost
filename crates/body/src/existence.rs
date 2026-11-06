//! Which names the file itself tests the existence of.
//!
//! `function_exists('ssh2_connect')`, `class_exists(Relay::class)`, `interface_exists`,
//! `enum_exists`, `trait_exists`, `defined('Normalizer::NFKC_CF')`: a call whose whole purpose is to
//! ask whether a name is there. A file that writes one has *said* the name may be absent and has
//! written the branch for it, so reporting the name as undefined tells its author what they already
//! knew and handled.
//!
//! `defined` is the one that names a *class constant* as well as a global one, and it names it in a
//! single string -- `'Normalizer::NFKC_CF'` -- so both halves are compared and neither is guessed.
//! Symfony's `AbstractUnicodeString` writes exactly that guard, because the constant exists in the
//! intl extension and not in the polyfill class that stands in for it.
//!
//! # Per file, and not per position
//!
//! The narrower reading -- the name is present only inside the branch the test guards -- is the
//! true one, and it is refused because it answers the wrong question. The guarded call is often
//! nowhere near the test: a PECL class is named in the type hints of a whole trait and constructed
//! once behind a `class_exists`, and WordPress tests for `ssh2_connect` in one method and calls
//! seven of its neighbours across the file. Requiring the test to enclose the call would report
//! most of exactly the code this exists to stop reporting.
//!
//! What it gives up is a file that tests a name in one place and calls it unguarded in another,
//! where the call really is a defect. That is a trade this check can afford: the value of
//! `undefined-function` is catching a *typo*, and a typo is never the name somebody wrote a
//! `function_exists` for.

use std::collections::HashSet;

use phrost_hir::{
    FileHir,
    body::{ArgumentLiteral, ExprData},
};
use phrost_model::Name;

use crate::scope::Scope;

/// The names a file tests for, split by what kind of name each test asks about.
#[derive(Default, Debug)]
pub struct ExistenceTests {
    functions: HashSet<Name>,
    classes: HashSet<Name>,
    /// The constants the file tests for with `defined('X')`, as written.
    ///
    /// Global and class constants in one set, because `defined` spells both and the reader asks
    /// about one of the two -- a class constant with the `::` in it, a global one without.
    constants: HashSet<Name>,
    /// The extensions the file tests for with `extension_loaded('ssh2')`.
    ///
    /// An extension names no function, so what connects the two is the convention every PHP
    /// extension follows: its functions are prefixed with its own name and an underscore --
    /// `ssh2_connect`, `imap_open`, `curl_init`. That is the whole of the rule, and it is why this
    /// is kept apart from the names above rather than expanded into them: a prefix match is a
    /// weaker claim than a name, and a reader should be able to see which one answered.
    extensions: HashSet<Name>,
    /// The member names the file tests for with `method_exists($x, 'm')` or `property_exists`.
    ///
    /// Only the name, because the first argument is a value and the class it holds is a question
    /// for resolution -- and what this answers does not need it. A file that asks whether *some*
    /// value has `headers()` has said the method may be declared somewhere nothing here can see,
    /// which is what a magic accessor answering for the name would contradict.
    members: HashSet<Name>,
}

impl ExistenceTests {
    /// Whether the file asks anywhere whether this function is declared.
    ///
    /// Case-insensitively, as PHP compares function names, and with any leading separator off:
    /// `\ssh2_connect` and `ssh2_connect` are one name.
    pub fn tests_function(&self, path: &str) -> bool {
        let wanted = normalised(path);
        self.functions
            .iter()
            .any(|name| normalised(name.as_str()) == wanted)
            || self.extensions.iter().any(|extension| {
                wanted.starts_with(&format!("{}_", normalised(extension.as_str())))
            })
    }

    /// Whether the file asks anywhere whether this class constant is declared.
    ///
    /// Both halves compared, which `defined` makes possible where `method_exists` does not: the
    /// class and the name arrive in one string, so a guard written about another class licenses
    /// nothing here. Class names are compared case-insensitively as PHP compares them, and the
    /// constant's own name exactly, as PHP compares that.
    pub fn tests_class_constant(&self, class: &str, name: &str) -> bool {
        self.constants.iter().any(|written| {
            let Some((wrote_class, wrote_name)) = written.as_str().split_once("::") else {
                return false;
            };
            wrote_name == name
                && normalised(wrote_class) == normalised(class.trim_start_matches('\\'))
        })
    }

    /// Whether the file asks anywhere whether some value has a member of this name.
    ///
    /// Case-insensitively, which is how PHP compares a method name; a property name compared the
    /// same way over-answers only for two properties differing in case, which nobody writes.
    pub fn tests_member(&self, name: &str) -> bool {
        self.members
            .iter()
            .any(|written| written.as_str().eq_ignore_ascii_case(name))
    }

    /// Whether the file asks anywhere whether this class-like name is declared.
    pub fn tests_class(&self, path: &str) -> bool {
        let wanted = normalised(path);
        self.classes
            .iter()
            .any(|name| normalised(name.as_str()) == wanted)
    }
}

/// Every existence test written in the file.
pub fn existence_tests(hir: &FileHir) -> ExistenceTests {
    let mut found = ExistenceTests::default();

    Scope::visit_every_expression(hir, &mut |hir, expr| {
        let ExprData::FunctionCall {
            name,
            literal_arguments,
            ..
        } = hir.expr(expr)
        else {
            return;
        };
        let called = hir.name_ref(*name).path.as_str().trim_start_matches('\\');
        let tested = if called.eq_ignore_ascii_case("function_exists") {
            Tested::Function
        } else if [
            "class_exists",
            "interface_exists",
            "enum_exists",
            "trait_exists",
        ]
        .iter()
        .any(|known| called.eq_ignore_ascii_case(known))
        {
            Tested::Class
        } else if called.eq_ignore_ascii_case("extension_loaded") {
            Tested::Extension
        } else if called.eq_ignore_ascii_case("defined") {
            Tested::Constant
        } else if called.eq_ignore_ascii_case("method_exists")
            || called.eq_ignore_ascii_case("property_exists")
        {
            Tested::Member
        } else {
            return;
        };
        // The name the call wrote, which lowering already carries for a literal argument -- as a
        // string, and as the class a `::class` named, which is the spelling a modern class test is
        // written in.
        // The member tests name the member second; every other test names its subject first.
        let position = match tested {
            Tested::Member => 1,
            Tested::Function | Tested::Class | Tested::Extension | Tested::Constant => 0,
        };
        let Some(named) = literal_arguments
            .iter()
            .find(|argument| {
                argument.position == position && argument.kind == ArgumentLiteral::Text
            })
            .map(|argument| argument.value.clone())
        else {
            return;
        };
        match tested {
            Tested::Class => found.classes.insert(named),
            Tested::Function => found.functions.insert(named),
            Tested::Extension => found.extensions.insert(named),
            Tested::Constant => found.constants.insert(named),
            Tested::Member => found.members.insert(named),
        };
    });

    found
}

/// What one existence test asks about.
enum Tested {
    Function,
    Class,
    Extension,
    Constant,
    Member,
}

fn normalised(path: &str) -> String {
    path.trim_start_matches('\\').to_ascii_lowercase()
}
