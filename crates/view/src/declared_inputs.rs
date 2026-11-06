//! The inputs a class declares in a property's string, and every read of one by name.
//!
//! Laravel's `protected $signature = 'mail:send {user} {--queue=}'` declares two inputs, and
//! `$this->argument('user')` and `$this->option('queue')` read them. The string is the only place
//! either is declared, so it is where a read goes to, what completion inside `option('|')` offers,
//! and one of the places a search for an input lists. What each input holds is read the same way --
//! see the contract reader's `returned_from_declared_input` -- and what a read of an undeclared one
//! means is `declared_input_uses`.
//!
//! Read from the file being analysed: a command's `handle()` sits beside its signature, and a class
//! declared elsewhere is no question here.

use phrost_body::{CallSite, CallTarget, PathRoot};
use phrost_defmap::DeclarationSource;
use phrost_hir::{KeptTextData, MemberKind};
use phrost_index::DeclaredInputsRule;
use phrost_model::{DeclaredInput, Name, TextRange, TextSize, matches_loosely};

use crate::{CursorTarget, catalog_uses::written_key, resolve::HIERARCHY_DEPTH, view::IndexView};

/// A class in the current file whose property declares its inputs.
pub(crate) struct Declaring<'a> {
    pub(crate) body: TextRange,
    pub(crate) rule: &'a DeclaredInputsRule,
    kept: &'a KeptTextData,
    /// Whether the declaration is every input the class has, beside what every class has: nothing
    /// between it and the rule's class may add one, and nothing in it adds one either.
    pub(crate) whole: bool,
}

impl Declaring<'_> {
    /// Every input the declaration writes, under the name it is written with.
    pub(crate) fn inputs(&self) -> Vec<DeclaredInput> {
        (self.rule.parse)(&self.kept.text)
    }

    /// Where in the file `input` is written -- `None` where the literal's text is not its value,
    /// which an escape inside it makes so: an offset into the one is then not an offset into the
    /// other.
    fn written_at(&self, text: &str, input: &DeclaredInput) -> Option<TextRange> {
        let range = self.kept.range;
        let source = text.get(usize::from(range.start())..usize::from(range.end()))?;
        (source == &*self.kept.text).then(|| input.range + range.start())
    }
}

/// One read of an input by name: `$this->option('queue')`.
pub(crate) struct InputRead<'a> {
    pub(crate) declaring: &'a Declaring<'a>,
    /// What the declaration writes before a name this reader is given: `--` for an option.
    pub(crate) prefix: &'a str,
    /// The method: Laravel's `argument` or `option`.
    pub(crate) reader: &'a Name,
    /// The name as the call writes it.
    pub(crate) written: &'a str,
    /// The literal, quotes and all.
    pub(crate) range: TextRange,
}

impl InputRead<'_> {
    /// The name as the declaration would write it: `--queue` for `option('queue')`.
    pub(crate) fn declared_name(&self) -> String {
        format!("{}{}", self.prefix, self.written)
    }

    /// Whether `input` is one this reader can be given: an argument's name has no prefix and an
    /// option's has one, so `option('user')` never reads the argument `user`.
    fn reads(&self, input: &str) -> Option<String> {
        match self.prefix.is_empty() {
            true => (!input.starts_with('-')).then(|| input.to_owned()),
            false => input.strip_prefix(self.prefix).map(str::to_owned),
        }
    }
}

/// One input completion offers for a read: its name as the call writes it, and what it holds.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeclaredInputEntry {
    pub name: String,
    /// What a read of it hands back: `?string`, `list<string>`, `bool`.
    pub holds: String,
    pub description: Option<String>,
}

impl IndexView<'_> {
    /// Every class in the current file whose property declares its inputs, below the class the rule
    /// names.
    pub(crate) fn declaring_classes(&self) -> Vec<Declaring<'_>> {
        let rules = self.skeleton.returns().declared_inputs();
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let hir = open.hir();
        if rules.is_empty() || hir.kept_texts().is_empty() {
            return Vec::new();
        }
        let calls = self.file_calls();
        let mut found = Vec::new();
        for (id, class) in hir.classes() {
            for rule in rules {
                let Some(kept) = hir
                    .kept_texts()
                    .iter()
                    .find(|kept| kept.class == id && kept.property.as_str() == &*rule.property)
                else {
                    continue;
                };
                let Some(fqn) = open
                    .scope()
                    .symbol_of(DeclarationSource::Class(id))
                    .map(|symbol| symbol.fqn.clone())
                else {
                    continue;
                };
                let direct = self
                    .class_header(fqn.as_str())
                    .and_then(|header| header.extends.first().cloned())
                    .is_some_and(|parent| parent.as_str().eq_ignore_ascii_case(&rule.class));
                if !direct && !self.descends_from(fqn.as_str(), &rule.class, HIERARCHY_DEPTH) {
                    continue;
                }
                let adds = |name: &str| {
                    rule.unless
                        .iter()
                        .any(|method| name.eq_ignore_ascii_case(method))
                };
                let declares_an_adder = hir.member_list(class.members).iter().any(|&member| {
                    let member = hir.member(member);
                    member.kind == MemberKind::Method && adds(member.name.as_str())
                });
                let calls_an_adder = calls.iter().any(|call| {
                    class.body_range.contains_range(call.range)
                        && matches!(&call.target, CallTarget::Member { name, .. } if adds(name.as_str()))
                });
                found.push(Declaring {
                    body: class.body_range,
                    rule,
                    kept,
                    whole: direct && !declares_an_adder && !calls_an_adder,
                });
            }
        }
        found
    }

    /// `call` as a read of an input one of `declaring` declares: a reading method called on `$this`
    /// with a literal name, inside the innermost class declaring inputs that method reads.
    pub(crate) fn input_read<'d>(
        &'d self,
        declaring: &'d [Declaring<'d>],
        call: &'d CallSite,
    ) -> Option<InputRead<'d>> {
        let CallTarget::Member { receiver, name } = &call.target else {
            return None;
        };
        // The instance the method runs on, and nothing reached through it: the declaration is about
        // `$this`.
        if !matches!(receiver.root, PathRoot::This) || !receiver.steps.is_empty() {
            return None;
        }
        // The class `$this` is: the innermost around the call, which an anonymous class inside a
        // command is, and declares nothing.
        let open = self.current()?;
        let innermost = open
            .hir()
            .classes()
            .map(|(_, class)| class.body_range)
            .filter(|body| body.contains_range(call.range))
            .min_by_key(|body| body.len())?;
        let (declaring, prefix) = declaring
            .iter()
            .filter(|class| class.body == innermost)
            .find_map(|class| Some((class, class.rule.prefix_read_by(name.as_str())?)))?;
        let (range, written) = written_key(open.text(), &call.arguments, 0)?;
        Some(InputRead {
            declaring,
            prefix,
            reader: name,
            written,
            range,
        })
    }

    /// What a read of a declared input names, for hover and goto: the input, what it holds, and
    /// where the declaration writes it.
    pub(crate) fn declared_input_at(&self, offset: TextSize) -> Option<CursorTarget> {
        // Asked at every call's callee while checking, so what settles it cheapest goes first.
        self.may_name_an_input(offset)?;
        let declaring = self.declaring_classes();
        let calls = self.file_calls();
        let read = self.input_read_at(&declaring, &calls, offset)?;
        let wanted = read.declared_name();
        let open = self.current()?;
        let inside = self.inside_the_quotes(read.range)?;
        let kind = match read.prefix.is_empty() {
            true => "argument",
            false => "option",
        };
        let Some(input) = read
            .declaring
            .inputs()
            .into_iter()
            .find(|input| input.name == wanted)
        else {
            // Every command has these, and nothing in this project writes them.
            let always = read
                .declaring
                .rule
                .always
                .iter()
                .any(|name| **name == *wanted);
            return always.then(|| CursorTarget::CatalogPlace {
                key: read.written.into(),
                detail: format!("{kind} {wanted}, which every command has").into(),
                place: None,
                range: inside,
            });
        };
        let mut detail = format!("{kind} {wanted}: {}", input.shape);
        if let Some(said) = &input.description {
            detail = format!("{detail} -- {said}");
        }
        let place = read
            .declaring
            .written_at(open.text(), &input)
            .and_then(|at| {
                let file = self.skeleton().file(open.hir().file())?;
                let at = open.line_index().line_col(at.start());
                Some(phrost_index::Place {
                    path: std::sync::Arc::from(file.path()),
                    line: at.line,
                    character: at.col,
                    detail: detail.clone().into(),
                })
            });
        Some(CursorTarget::CatalogPlace {
            key: read.written.into(),
            detail: detail.into(),
            place,
            range: inside,
        })
    }

    /// Every input the read under the cursor could name, in the order declared, for completion --
    /// with the range between its quotes, which accepting one replaces.
    ///
    /// Filtered by everything typed between the opening quote and the cursor rather than by the word
    /// before it, and the whole literal replaced: `dry-run` is one name, and a word ends at its dash.
    ///
    /// Only what the declaration writes. What every command has beside it -- `--verbose`, `--env` --
    /// is read through methods of its own (`$this->output->isVerbose()`), and nine of them ahead of a
    /// command's own two would bury the answer.
    pub fn declared_inputs_at(
        &self,
        offset: TextSize,
    ) -> Option<(TextRange, Vec<DeclaredInputEntry>)> {
        self.may_name_an_input(offset)?;
        let declaring = self.declaring_classes();
        let calls = self.file_calls();
        let read = self.input_read_at(&declaring, &calls, offset)?;
        let inside = self.inside_the_quotes(read.range)?;
        let typed = self.current()?.text().get(
            usize::from(inside.start())..usize::from(offset.clamp(inside.start(), inside.end())),
        )?;
        let entries = read
            .declaring
            .inputs()
            .into_iter()
            .filter_map(|input| {
                let name = read.reads(&input.name)?;
                matches_loosely(&name, typed).then(|| DeclaredInputEntry {
                    name,
                    holds: input.shape.to_string(),
                    description: input.description,
                })
            })
            .collect();
        Some((inside, entries))
    }

    /// The input a search or a rename at `offset` is about, as the declaration writes its name: a
    /// read's literal, or the name inside the declaration itself.
    pub fn declared_input_use_at(&self, offset: TextSize) -> Option<Box<str>> {
        self.may_name_an_input(offset)?;
        let declaring = self.declaring_classes();
        let calls = self.file_calls();
        if let Some(read) = self.input_read_at(&declaring, &calls, offset) {
            return Some(read.declared_name().into());
        }
        let text = self.current()?.text();
        declaring.iter().find_map(|class| {
            class.inputs().into_iter().find_map(|input| {
                class
                    .written_at(text, &input)
                    .filter(|at| at.contains_inclusive(offset))
                    .map(|_| input.name.into())
            })
        })
    }

    /// Every place the class enclosing `at` names its input `name`, which is written the way the
    /// declaration writes it: where it is declared and every read of it, each without its quotes.
    pub fn declared_input_occurrences(&self, at: TextSize, name: &str) -> Vec<TextRange> {
        let declaring = self.declaring_classes();
        let Some(class) = declaring
            .iter()
            .filter(|class| class.body.contains_inclusive(at))
            .min_by_key(|class| class.body.len())
        else {
            return Vec::new();
        };
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let declared = class
            .inputs()
            .into_iter()
            .filter(|input| input.name == name)
            .filter_map(|input| class.written_at(open.text(), &input));
        let calls = self.file_calls();
        let read = calls
            .iter()
            .filter_map(|call| self.input_read(&declaring, call))
            .filter(|read| std::ptr::eq(read.declaring, class) && read.declared_name() == name)
            .filter_map(|read| self.inside_the_quotes(read.range));
        let mut found = declared.chain(read).collect::<Vec<_>>();
        found.sort_by_key(|range| range.start());
        found.dedup();
        found
    }

    /// Whether `offset` can be in a string naming an input at all: a rule to read one by, a
    /// declaration kept in this file, and a string under the cursor -- in the order they are cheap.
    fn may_name_an_input(&self, offset: TextSize) -> Option<()> {
        if self.skeleton.returns().declared_inputs().is_empty() {
            return None;
        }
        let hir = self.current()?.hir();
        if hir.kept_texts().is_empty() {
            return None;
        }
        hir.string_at(offset).map(|_| ())
    }

    /// The read whose literal the cursor is in.
    fn input_read_at<'d>(
        &'d self,
        declaring: &'d [Declaring<'d>],
        calls: &'d [CallSite],
        offset: TextSize,
    ) -> Option<InputRead<'d>> {
        if declaring.is_empty() {
            return None;
        }
        let quoted = self.current()?.hir().string_at(offset)?;
        calls
            .iter()
            .filter(|call| {
                call.arguments
                    .first()
                    .is_some_and(|argument| argument.range == quoted)
            })
            .find_map(|call| self.input_read(declaring, call))
    }
}
