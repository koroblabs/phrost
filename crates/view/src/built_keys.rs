//! A key a call reads that names an element the enclosing class builds.
//!
//! Drupal's `$form_state->getValue('email')` reads what was submitted for the element its form wrote
//! as `$form['email'] = [...]` -- or as `'email' => [...]` inside another element, since a value is
//! keyed by the element's own key at whatever depth. The string names nothing PHP declares, and the
//! arrays the class builds are the only statement of which keys there are, so that is where goto
//! goes and what completion offers.
//!
//! # Every method of the class, and why not only the builder
//!
//! A form hands its array around: `buildForm()` returns `$this->buildEditForm($form)`, a base class's
//! `vendorFields()` builds `$elements['company_name']` for a subclass to merge in, a helper returns
//! `['type' => [...]]`, `'#parents' => ['emails', $k]` files a value under a key of its own. On the
//! project this was measured on, reading `buildForm()`'s own body answered 175 of 217 reads and the
//! class's every method answers 201; of the other 16, nine are values the form keeps without an
//! element (`action`, Drupal's own `op`) or entity fields an alter adds, and the rest are written by a
//! parent or a sibling class in another file. So an element is recognised by what it *is* -- an array
//! holding a property key, `#type`, `#title` -- and not by which method it was written in.
//!
//! Read from the open file's class, and only there: a key a parent class in another file adds, or a
//! `form_alter` elsewhere, is a key this cannot see -- which is why nothing is reported for a key it
//! does not find.

use phrost_hir::ExprId;
use phrost_hir::FileHir;
use phrost_hir::body::{ArrayElementKind, ExprData, LiteralKind};
use phrost_model::{Name, TextRange, TextSize};

use crate::{
    CursorTarget,
    view::{FOLLOW_LIMIT, IndexView},
};

/// One key an element is written under, and where.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BuiltKeyEntry {
    pub key: String,
    /// The key as written, quotes included.
    pub range: TextRange,
    /// The element's `#type`, when it is written as a literal: what a list of keys is worth reading
    /// beside.
    pub detail: String,
}

impl IndexView<'_> {
    /// What a string under the cursor names when a call reads it as a key its class's builder wrote.
    pub(crate) fn built_key_at(&self, offset: TextSize) -> Option<CursorTarget> {
        let reading = self.reading_call_at(offset)?;
        let entry = self
            .keys_built_for(offset, &reading)?
            .into_iter()
            .find(|entry| entry.key == reading.written)?;
        let open = self.current()?;
        let file = self.skeleton().file(open.hir().file())?;
        // At the key itself rather than its opening quote, so that a client selecting the key's
        // length from here selects the key.
        let at = open.line_index().line_col(
            self.inside_the_quotes(entry.range)
                .map_or(entry.range.start(), |inside| inside.start()),
        );
        Some(CursorTarget::CatalogPlace {
            key: reading.written.into(),
            detail: entry.detail.clone().into(),
            place: Some(phrost_index::Place {
                path: std::sync::Arc::from(file.path()),
                line: at.line,
                character: at.col,
                detail: entry.detail.into(),
            }),
            range: reading.inside,
        })
    }

    /// Every key the string under the cursor could name, for completion.
    pub fn built_keys_at(&self, offset: TextSize) -> Option<Vec<BuiltKeyEntry>> {
        let reading = self.reading_call_at(offset)?;
        self.keys_built_for(offset, &reading)
    }

    /// The element key under the cursor, in a class that reads its elements' keys: a reading call's
    /// string, or a key the class writes an element under.
    ///
    /// The second only where the first exists somewhere in the class -- an element nothing reads by
    /// its key is an array key like any other, and searching for it means nothing more.
    pub fn built_key_use_at(&self, offset: TextSize) -> Option<Box<str>> {
        if let Some(reading) = self.reading_call_at(offset) {
            return Some(reading.written.into());
        }
        self.current()?.hir().string_at(offset)?;
        let reading = self.readings_in(offset).into_iter().next()?;
        self.elements_written(offset, &reading)?
            .into_iter()
            .find(|entry| entry.range.contains_inclusive(offset))
            .map(|entry| entry.key.into())
    }

    /// Every place the class enclosing `at` names the element key `key`: where it writes an element
    /// under it, and where it reads one by it. What is between the quotes, which is the key.
    pub fn built_key_occurrences(&self, at: TextSize, key: &str) -> Vec<TextRange> {
        let readings = self.readings_in(at);
        let Some(first) = readings.first() else {
            return Vec::new();
        };
        let written = self
            .elements_written(at, first)
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| entry.key == key)
            .filter_map(|entry| self.inside_the_quotes(entry.range));
        let read = readings
            .iter()
            .filter(|reading| reading.written == key)
            .map(|reading| reading.inside);
        let mut found = written.chain(read).collect::<Vec<_>>();
        found.sort_by_key(|range| range.start());
        found.dedup();
        found
    }

    /// The reading call the string under the cursor is the key argument of.
    fn reading_call_at(&self, offset: TextSize) -> Option<Reading> {
        let rules = self.skeleton().built_keys();
        if rules.is_empty() {
            return None;
        }
        let open = self.current()?;
        let hir = open.hir();
        let quoted = hir.string_at(offset)?;
        // What stands before the opening quote, cheaply first: the call a string is the first
        // argument of is written right there, and nearly no string is one of these.
        let before = open.text().get(..usize::from(quoted.start()))?.trim_end();
        let called = before.strip_suffix('(')?.trim_end();
        let word = &called[called
            .rfind(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .map_or(0, |at| at + 1)..];
        let rule = rules
            .iter()
            .find(|rule| word.eq_ignore_ascii_case(&rule.method))?;

        self.file_calls().iter().find_map(|call| {
            let argument = call.arguments.first()?;
            match hir.expr(argument.value) {
                ExprData::Literal {
                    kind: LiteralKind::String,
                    range,
                } if *range == quoted => self.reading_of(call, rule),
                _ => None,
            }
        })
    }

    /// Every reading call in the class enclosing `offset`, in source order.
    fn readings_in(&self, offset: TextSize) -> Vec<Reading> {
        let rules = self.skeleton().built_keys();
        let Some(open) = self.current() else {
            return Vec::new();
        };
        let Some((_, class)) = open
            .hir()
            .classes()
            .filter(|(_, class)| class.body_range.contains_inclusive(offset))
            .last()
        else {
            return Vec::new();
        };
        self.file_calls()
            .iter()
            .filter(|call| class.body_range.contains_range(call.callee_range))
            .filter_map(|call| {
                let phrost_body::CallTarget::Member { name, .. } = &call.target else {
                    return None;
                };
                let rule = rules
                    .iter()
                    .find(|rule| name.as_str().eq_ignore_ascii_case(&rule.method))?;
                self.reading_of(call, rule)
            })
            .collect()
    }

    /// `call` as a reading of `rule`: its method, a literal key, and a receiver the rule names.
    fn reading_of(
        &self,
        call: &phrost_body::CallSite,
        rule: &phrost_index::BuiltKey,
    ) -> Option<Reading> {
        let phrost_body::CallTarget::Member { receiver, name } = &call.target else {
            return None;
        };
        if !name.as_str().eq_ignore_ascii_case(&rule.method) {
            return None;
        }
        let open = self.current()?;
        let hir = open.hir();
        let ExprData::Literal {
            kind: LiteralKind::String,
            range,
        } = hir.expr(call.arguments.first()?.value)
        else {
            return None;
        };
        let at = call.receiver_at();
        let held = self.resolve_path(at, hir, receiver, FOLLOW_LIMIT)?;
        let class = self.class_of_held(at, hir, &held)?;
        if !class.as_str().eq_ignore_ascii_case(&rule.class)
            && !self.descends_from(class.as_str(), &rule.class, 8)
        {
            return None;
        }

        let inside = self.inside_the_quotes(*range)?;
        Some(Reading {
            written: open
                .text()
                .get(usize::from(inside.start())..usize::from(inside.end()))?
                .to_owned(),
            inside,
            rule: rule.clone(),
        })
    }

    /// The element keys the class enclosing `offset` writes, each once, in the order written.
    fn keys_built_for(&self, offset: TextSize, reading: &Reading) -> Option<Vec<BuiltKeyEntry>> {
        let mut found = self.elements_written(offset, reading)?;
        // The first time a key is written is where it is declared; a later write adds to it.
        let mut seen = std::collections::HashSet::new();
        found.retain(|entry| seen.insert(entry.key.clone()));
        Some(found)
    }

    /// Every place the class enclosing `offset` writes an element under a key, in source order.
    fn elements_written(&self, offset: TextSize, reading: &Reading) -> Option<Vec<BuiltKeyEntry>> {
        let open = self.current()?;
        let hir = open.hir();
        let (_, class) = hir
            .classes()
            .filter(|(_, class)| class.body_range.contains_inclusive(offset))
            .last()?;
        let elements = Elements {
            hir,
            text: open.text(),
            rule: &reading.rule,
        };
        // A write into a variable says it is a render array only by what it writes, so a write of a
        // call's result -- `$elements['district'] = $this->districtElement()` -- counts once another
        // write in the same method has said so. Methods by range, since a variable is per method.
        let methods = hir
            .member_list(class.members)
            .iter()
            .map(|id| hir.member(*id))
            .filter(|member| member.kind == phrost_hir::MemberKind::Method)
            .map(|member| member.full_range)
            .collect::<Vec<_>>();
        let method_of = |range: TextRange| {
            methods
                .iter()
                .position(|method| method.contains_range(range))
        };

        let mut found = Vec::new();
        let mut renders = Vec::<(usize, Name)>::new();
        let mut undecided = Vec::new();
        for (_, data) in hir.expressions() {
            if !class.body_range.contains_range(data.range()) {
                continue;
            }
            match data {
                ExprData::ArrayLiteral { elements: list, .. } => {
                    elements.literal_keys(hir.expr_list(*list), &mut found);
                }
                ExprData::Assign { target, value, .. } => {
                    let Some(write) = elements.write(*target, *value) else {
                        continue;
                    };
                    if write.says_element {
                        if let (Some(root), Some(method)) = (&write.root, method_of(data.range())) {
                            renders.push((method, root.clone()));
                        }
                        found.extend(write.keys);
                    } else {
                        undecided.push((method_of(data.range()), write));
                    }
                }
                _ => {}
            }
        }
        for (method, write) in undecided {
            let into_a_render_array =
                method
                    .zip(write.root.as_ref())
                    .is_some_and(|(method, root)| {
                        renders
                            .iter()
                            .any(|(at, name)| *at == method && name == root)
                    });
            if into_a_render_array {
                found.extend(write.keys);
            }
        }

        found.sort_by_key(|entry| entry.range.start());
        Some(found)
    }
}

/// The reading call a string is the key of, and the rule that made it one.
struct Reading {
    written: String,
    inside: TextRange,
    rule: phrost_index::BuiltKey,
}

/// What the rule says an element is, applied to one file.
struct Elements<'a> {
    hir: &'a FileHir,
    text: &'a str,
    rule: &'a phrost_index::BuiltKey,
}

/// One `$array['a']['b'] = ...`, read for the element keys it writes.
struct Write {
    /// The variable written into, when it is one.
    root: Option<Name>,
    /// Whether the write itself shows its array is a render array: it stores an element, or sets a
    /// property of one.
    says_element: bool,
    keys: Vec<BuiltKeyEntry>,
}

impl Elements<'_> {
    /// The keys of an array literal whose values are elements, and the key a `#parents` files its
    /// element's value under.
    fn literal_keys(&self, list: &[ExprId], found: &mut Vec<BuiltKeyEntry>) {
        for element in list {
            let ExprData::ArrayElement {
                kind: ArrayElementKind::LiteralKey { name, range },
                value,
                ..
            } = self.hir.expr(*element)
            else {
                continue;
            };
            if name.as_str() == &*self.rule.filed_under {
                found.extend(self.filed_under(*value));
            } else if !self.is_property(name.as_str()) && self.read_by_its_key(*value) {
                found.push(BuiltKeyEntry {
                    key: name.as_str().to_owned(),
                    range: *range,
                    detail: self.described(*value),
                });
            }
        }
    }

    /// `'#parents' => ['emails', $k, 'name']` files the value under `emails`.
    fn filed_under(&self, value: ExprId) -> Option<BuiltKeyEntry> {
        let ExprData::ArrayLiteral { elements, .. } = self.hir.expr(value) else {
            return None;
        };
        let first = *self.hir.expr_list(*elements).first()?;
        let ExprData::Literal {
            kind: LiteralKind::String,
            range,
        } = self.hir.expr(first)
        else {
            return None;
        };
        let written = self
            .text
            .get(usize::from(range.start())..usize::from(range.end()))?;
        Some(BuiltKeyEntry {
            key: written.get(1..written.len().checked_sub(1)?)?.to_owned(),
            range: *range,
            detail: String::new(),
        })
    }

    /// The keys a write into an array names, outermost first, up to the first property: in
    /// `$form['name']['#options']['a']`, `a` is an option and not an element.
    fn write(&self, target: ExprId, value: ExprId) -> Option<Write> {
        let mut chain = Vec::new();
        let mut at = target;
        let root = loop {
            match self.hir.expr(at) {
                ExprData::Index {
                    base, key, index, ..
                } => {
                    if let (Some(key), Some(index)) = (key, index) {
                        chain.push((key.clone(), self.hir.expr(*index).range()));
                    }
                    at = *base;
                }
                ExprData::Variable { name, .. } => break Some(name.clone()),
                _ => break None,
            }
        };
        if chain.is_empty() {
            return None;
        }
        chain.reverse();

        let property = chain
            .iter()
            .position(|(key, _)| self.is_property(key.as_str()));
        let stored = property.is_none() && self.is_element(value);
        if stored && !self.read_by_its_key(value) {
            return None;
        }
        let named = &chain[..property.unwrap_or(chain.len())];
        // `$form['state']['#type'] = 'select'` says what `state` is as plainly as a literal would.
        let detail = match property {
            Some(at)
                if at + 1 == chain.len() && chain[at].0.as_str() == &*self.rule.described_by =>
            {
                self.string(value).unwrap_or_default()
            }
            None if stored => self.described(value),
            _ => String::new(),
        };
        let last = named.len().saturating_sub(1);
        Some(Write {
            root,
            says_element: stored || property.is_some(),
            keys: named
                .iter()
                .enumerate()
                .map(|(nth, (key, range))| BuiltKeyEntry {
                    key: key.as_str().to_owned(),
                    range: *range,
                    detail: if nth == last {
                        detail.clone()
                    } else {
                        String::new()
                    },
                })
                .collect(),
        })
    }

    fn is_property(&self, key: &str) -> bool {
        key.starts_with(&*self.rule.property)
    }

    /// An array literal holding a property key.
    fn is_element(&self, value: ExprId) -> bool {
        self.property_value(value, None).is_some()
    }

    /// An element whose value is read by the key it is written under, which `#parents` overrides.
    fn read_by_its_key(&self, value: ExprId) -> bool {
        self.is_element(value)
            && self
                .property_value(value, Some(&self.rule.filed_under))
                .is_none()
    }

    /// The element's literal description -- its `#type` -- when it says one.
    fn described(&self, value: ExprId) -> String {
        self.property_value(value, Some(&self.rule.described_by))
            .and_then(|value| self.string(value))
            .unwrap_or_default()
    }

    /// The value of the property `named` -- or of any property, for `None` -- in an array literal.
    fn property_value(&self, value: ExprId, named: Option<&str>) -> Option<ExprId> {
        let ExprData::ArrayLiteral { elements, .. } = self.hir.expr(value) else {
            return None;
        };
        self.hir.expr_list(*elements).iter().find_map(|element| {
            let ExprData::ArrayElement {
                kind: ArrayElementKind::LiteralKey { name, .. },
                value,
                ..
            } = self.hir.expr(*element)
            else {
                return None;
            };
            let wanted = match named {
                Some(named) => name.as_str() == named,
                None => self.is_property(name.as_str()),
            };
            wanted.then_some(*value)
        })
    }

    fn string(&self, value: ExprId) -> Option<String> {
        let ExprData::Literal {
            kind: LiteralKind::String,
            range,
        } = self.hir.expr(value)
        else {
            return None;
        };
        let written = self
            .text
            .get(usize::from(range.start())..usize::from(range.end()))?;
        Some(written.trim_matches(['\'', '"']).to_owned())
    }
}
