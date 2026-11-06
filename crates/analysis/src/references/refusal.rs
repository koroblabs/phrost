//! When a rename must not go ahead, and what to tell the person who asked.
//!
//! Rename is the one request where a wrong answer destroys code rather than merely disappointing
//! somebody, so the interesting output is not the edit -- it is the decline. A rename that quietly
//! misses one site leaves a project that no longer runs and an editor's undo that is one step per
//! file; a rename that declines costs a sentence.
//!
//! Two declines live here, and they are different in kind. One is about the *name*: PHP will not
//! let a class be called `class`, and the lexical check next door cannot see it. The other is about
//! the *hierarchy*: a method declared in more than one place is renamed in all of them, and a
//! hierarchy too large to enumerate is one nobody can prove safe -- see `family`, which used to be
//! a blanket refusal here and is now the feature.

use phrost_view::{IndexView, MemberKind};

use super::{ReferenceTarget, rename::is_reserved_word};

/// Why this rename cannot be done, or `None` when nothing stands in its way.
///
/// Asked by both halves of the protocol: `prepareRename` with no name yet, so the dialogue does not
/// open on a rename that cannot finish, and `rename` with the name a person typed.
///
/// Cheap: the work of deciding which declarations move together was done by `rename_target` when it
/// built the family, and this reads what that walk recorded.
pub fn rename_refusal(
    view: &IndexView<'_>,
    target: &ReferenceTarget,
    new_name: Option<&str>,
) -> Option<String> {
    match target {
        // A search target, which nothing renames: `rename_target` always answers with the narrower
        // reading below, so a class arriving here under its every-spelling name is a caller asking
        // the wrong question rather than a rename with nothing in its way.
        ReferenceTarget::Class(_) => None,
        ReferenceTarget::ClassUnderItsOwnName(fqn) => {
            let new_name = new_name?;
            is_reserved_word(new_name).then(|| {
                format!(
                    "`{new_name}` is a word PHP reserves, so no class can be called it -- \
                     `{}` would stop parsing in every file this touched",
                    phrost_model::short_name(fqn.as_str())
                )
            })
        }
        // The same refusal a class gets, and asked of PHP rather than assumed: `function list()`,
        // `function print()`, `function match()` and `const list = 1;` are all parse errors on
        // 8.5, exactly as `class list` is. A method is the one place the language relaxed this,
        // which is why the family below does not ask.
        ReferenceTarget::Function(fqn) | ReferenceTarget::Constant(fqn) => {
            let new_name = new_name?;
            let what = match target {
                ReferenceTarget::Constant(_) => "constant",
                _ => "function",
            };
            is_reserved_word(new_name).then(|| {
                format!(
                    "`{new_name}` is a word PHP reserves, so no {what} can be called it -- \
                     `{}` would stop parsing in every file this touched",
                    phrost_model::short_name(fqn.as_str())
                )
            })
        }
        // A search target, which nothing renames: `rename_target` always answers with the family.
        ReferenceTarget::Member { .. } => None,
        // The family, which is renamed whole. Two refusals are left: a method the *language*
        // calls, where no reference walk can see the call site at all, and a family this could not
        // finish enumerating -- a hierarchy too large to walk is one nobody can prove safe.
        ReferenceTarget::MemberFamily {
            declarers,
            kind,
            name,
            complete,
        } => {
            if *kind == MemberKind::Method
                && let Some(why) = magic_method_refusal(name.as_str(), new_name)
            {
                return Some(why);
            }
            if *kind == MemberKind::Property
                && let Some(owner) = declarers
                    .iter()
                    .find(|owner| view.declared_only_by_a_table(owner.as_str(), name.as_str()))
            {
                return Some(format!(
                    "`{name}` is a key of the table `{}` states, not a property PHP declares -- the same name is read through every value of that kind and stored under it, and a rename can reach neither",
                    phrost_model::short_name(owner.as_str())
                ));
            }
            (!complete).then(|| {
                format!(
                    "`{name}` is declared in more classes than this can enumerate -- at least {}, and the walk gave up before it found them all. Renaming some of them leaves the rest answering the old name, which is a project that no longer runs.",
                    declarers.len()
                )
            })
        }
        // Declared in a framework's configuration, which the reference walk does not edit: renaming
        // the PHP that asks for a key leaves the YAML still declaring the old one, and every call
        // then asks for something nothing registers.
        ReferenceTarget::CatalogKey { key, .. } => Some(format!(
            "`{key}` is declared in the framework's configuration, which a rename here does not edit -- rename it there and at its uses together"
        )),
        // The class is where this can see the key, and not where it is written: a `#states`
        // selector spells it inside a string, an alter in another module reads it, and so does the
        // test submitting the form. A rename of what is listed here leaves every one of those behind.
        ReferenceTarget::ElementKey { key, .. } => Some(format!(
            "`{key}` is also named where this cannot see -- `#states` selectors, form alters, tests that submit the form -- so a rename here would leave those reading the old key"
        )),
        // The signature and the reads are only where PHP names it. Whoever runs the command names it
        // too -- a crontab, a scheduler's `command('mail:send --queue=high')`, `Artisan::call()`, a
        // test -- and none of those is a read this can find.
        ReferenceTarget::DeclaredInput { name, .. } => Some(format!(
            "`{name}` is also named by whatever runs the command -- the scheduler, `Artisan::call()`, a test, a crontab -- so a rename here would leave those passing the old name"
        )),
        // A local and an alias reach one file, and nothing else in the language can be declared
        // twice under either name. PHP has let a method be called `list` or `print` since 7.0 and a
        // variable always could, so the reserved words above are not asked about here.
        ReferenceTarget::Local { .. } | ReferenceTarget::Alias { .. } => None,
        // The clause and the calls are every place PHP reads the name -- except an override: the
        // alias is a method of the class, and a subclass declaring one of that name replaces it.
        // A local's rules: the name is checked by the protocol layer like any variable's.
        ReferenceTarget::Parameter(_) => None,
        ReferenceTarget::TraitAlias(alias) => view.trait_alias_overridden_by(alias).map(|by| {
            format!(
                "`{}` is overridden by `{}`, which declares a method of that name -- renaming the alias alone would stop it overriding",
                alias.alias,
                phrost_model::short_name(by.as_str())
            )
        }),
        // The name lives wherever the accessor compares against it -- a `match`, an array key, a
        // YAML file -- and no reference walk sees any of those. Renaming the accesses alone leaves
        // them asking for a name the accessor no longer answers.
        ReferenceTarget::MagicMember {
            answered_by, name, ..
        } => Some(format!(
            "`{name}` is declared nowhere -- `{}`'s magic accessor answers it, and the names that accessor recognises are not something a rename can find",
            phrost_model::short_name(answered_by.as_str())
        )),
    }
}

/// Every method name PHP gives a meaning of its own.
///
/// A closed list rather than the `__` prefix the manual reserves in prose: a project writing
/// `__helper()` is ignoring advice and nothing more, and refusing to rename that would be this
/// server inventing a rule. These seventeen are the ones the *language* calls.
const MAGIC_METHODS: [&str; 17] = [
    "__construct",
    "__destruct",
    "__call",
    "__callStatic",
    "__get",
    "__set",
    "__isset",
    "__unset",
    "__sleep",
    "__wakeup",
    "__serialize",
    "__unserialize",
    "__toString",
    "__invoke",
    "__set_state",
    "__clone",
    "__debugInfo",
];

/// The refusal for a method PHP calls itself, in either direction.
///
/// Renaming one is never what anybody means and always breaks code no reference walk can see: the
/// call site of `__construct` is a `new`, of `__toString` a cast, of `__clone` a `clone`. Nothing
/// writes the name, so the rename edits the declaration, reports success, and leaves a class whose
/// constructor no longer runs.
///
/// The other direction is the same mistake arriving from the dialogue: renaming `build()` to
/// `__construct` makes every existing call reach the language's own meaning instead.
fn magic_method_refusal(name: &str, new_name: Option<&str>) -> Option<String> {
    if let Some(magic) = MAGIC_METHODS
        .iter()
        .find(|magic| magic.eq_ignore_ascii_case(name))
    {
        return Some(format!(
            "`{magic}` is a method PHP calls itself -- nothing in the project writes the name, so renaming it would edit the declaration and leave every place the language reaches it calling something that is gone"
        ));
    }
    let new_name = new_name?;
    MAGIC_METHODS
        .iter()
        .find(|magic| magic.eq_ignore_ascii_case(new_name))
        .map(|magic| {
            format!(
                "`{magic}` is a method PHP calls itself, so renaming `{name}` to it would make every existing call reach the language's own meaning instead"
            )
        })
}
