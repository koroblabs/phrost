//! Every input a class reads by a name its own declaration does not write.
//!
//! `$this->argument('usr')` on a Laravel command whose `$signature` writes `{user}` is Symfony's
//! `InvalidArgumentException: The "usr" argument does not exist.` on the run that reaches the line,
//! and like a misspelt service id it is a string no compiler, linter or type checker says anything
//! about. The declaration is the whole list once what every command has is added to it: a command
//! with a signature reads nothing else -- Laravel asks `getArguments()` only without one. Where it
//! may not be the whole list nothing is judged: a class declaring or calling a method that adds
//! inputs (`configure()`, `addOption()`), and a class not extending the rule's class directly, since
//! a class between the two may add some in its constructor -- Laravel's `GeneratorCommand` gives
//! every `make:` command using `CreatesMatchingTest` a `--test`, a `--pest` and a `--phpunit`.
//!
//! Read from the file being analysed, as the type of each input is: the reads sit beside the
//! declaration, and a class declared elsewhere is no question here.

use phrost_model::TextRange;

use crate::view::IndexView;

/// One read of an input its class's declaration does not write.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UndeclaredInput {
    /// The literal, quotes and all, which is what a report underlines.
    pub range: TextRange,
    /// The name as the call writes it.
    pub name: Box<str>,
    /// The method that read it: Laravel's `argument` or `option`.
    pub reader: Box<str>,
    /// A declared name the method reads, one edit from this one, in the call's spelling. `None`
    /// where nothing is that near, or two are.
    pub nearest: Option<Box<str>>,
}

impl IndexView<'_> {
    /// Every read in the current file of an input the reading class's declaration does not write,
    /// in source order.
    pub fn undeclared_inputs(&self) -> Vec<UndeclaredInput> {
        let declaring = self.declaring_classes();
        if !declaring.iter().any(|class| class.whole) {
            return Vec::new();
        }
        // Each declaration that is the whole list, with every name on it.
        let names = declaring
            .iter()
            .map(|class| {
                let mut names = class
                    .inputs()
                    .into_iter()
                    .map(|input| input.name)
                    .collect::<Vec<_>>();
                names.extend(class.rule.always.iter().map(|name| name.to_string()));
                names
            })
            .collect::<Vec<_>>();

        let calls = self.file_calls();
        let mut out = Vec::new();
        for call in calls.iter() {
            let Some(read) = self.input_read(&declaring, call) else {
                continue;
            };
            if !read.declaring.whole {
                continue;
            }
            let Some(names) = declaring
                .iter()
                .position(|class| std::ptr::eq(class, read.declaring))
                .map(|at| &names[at])
            else {
                continue;
            };
            if names.contains(&read.declared_name()) {
                continue;
            }
            out.push(UndeclaredInput {
                range: read.range,
                name: read.written.into(),
                reader: read.reader.as_str().into(),
                nearest: Self::nearest_input(names, read.prefix, read.written),
            });
        }
        out
    }

    /// The one declared name this method reads that is a single edit from `written`, in the call's
    /// spelling. The same bound a catalog key's suggestion uses, and for its reason: a confidently
    /// wrong suggestion is worse than none.
    fn nearest_input(names: &[String], prefix: &str, written: &str) -> Option<Box<str>> {
        let mut near = names.iter().filter_map(|declared| {
            // An argument is written with no prefix and an option with one, so each method's names
            // are the ones its prefix leaves something of -- and an argument's are not an option's.
            let bare = match prefix.is_empty() {
                true => (!declared.starts_with('-')).then_some(declared.as_str())?,
                false => declared.strip_prefix(prefix)?,
            };
            phrost_index::edits_within(written, bare, phrost_index::NEAR_MISS_EDITS)
                .map(|distance| (distance, bare))
        });
        let (distance, best) = near.next()?;
        let mut best = (distance, best);
        let mut tied = false;
        for (distance, bare) in near {
            match distance.cmp(&best.0) {
                std::cmp::Ordering::Less => {
                    best = (distance, bare);
                    tied = false;
                }
                std::cmp::Ordering::Equal if bare != best.1 => tied = true,
                _ => {}
            }
        }
        (!tied).then(|| best.1.into())
    }
}
