//! The string literal a call reads a catalog key out of.
//!
//! `\Drupal::service('|')` -- the id between the quotes is what says which class comes back, and
//! answering "what is this id" and "what could this id be" both start by finding the literal and
//! the catalogs the call is bound to. Apart from its siblings because it is the one part of this
//! module that works in bytes: the moment that matters for completion is the one the tree cannot
//! answer, where the quote is still open and the literal does not parse.

use phrost_model::TextSize;

use crate::view::IndexView;

impl IndexView<'_> {
    /// Whether `offset` sits inside a string literal.
    ///
    /// Asked before offering names, because a class name is not what someone typing inside
    /// `service('...')` is reaching for -- and a list of 188 classes over a service id is noise that
    /// hides the editor's own history.
    ///
    /// From the lowered body rather than by counting quotes: the tree knows exactly where a literal
    /// is, including one spanning lines. A string being typed and still unterminated does not parse,
    /// so this says nothing about it -- a miss, not a wrong answer.
    pub fn inside_string_literal(&self, offset: TextSize) -> bool {
        let Some(open) = self.current() else {
            return false;
        };
        phrost_body::string_literal_at(open.hir(), offset)
    }

    /// The catalog keys a call accepts, when the cursor is inside its first argument.
    ///
    /// `\Drupal::service('|` -- the id is the only thing in the source that says what comes back, so
    /// it is also the only thing worth offering there. A class name is not what somebody typing an id
    /// is reaching for, which is why nothing else is offered inside a string.
    ///
    /// Located through the byte-scanned string ranges and not the argument node, because the moment
    /// that matters is the one the tree cannot answer: the quote is still open, so the literal does
    /// not parse.
    ///
    /// The range is what an accepted item replaces -- the text between the quotes, which for an
    /// unterminated literal runs to wherever the scan stopped.
    /// The literal a bound call is reading, and which catalogs it indexes.
    ///
    /// The shared half of "what is this id" and "what could this id be": both need the text between
    /// the quotes and the catalogs the call is bound to, and neither should work that out twice.
    ///
    /// Located through the byte-scanned string ranges and the call site signature help already uses,
    /// because the moment that matters for completion is the one the tree cannot answer -- the quote
    /// is still open, so the literal does not parse.
    fn bound_argument_at(&self, offset: TextSize) -> Option<(phrost_model::TextRange, Vec<&str>)> {
        let quoted = self.current()?.hir().string_at(offset)?;
        let inside = self.inside_the_quotes(quoted)?;

        let catalogs = self
            .call_catalogs_at(offset, quoted)
            .or_else(|| self.element_catalogs_at(offset))?;
        (!catalogs.is_empty()).then_some((inside, catalogs))
    }

    /// What an accepted completion replaces inside a string literal: its text, not its delimiters.
    ///
    /// The scanned range covers the whole literal including both quotes -- which is what makes
    /// `inside_string` right about a cursor sitting just after the opening one -- so replacing it
    /// as-is swallows the closing quote and leaves the string unterminated the moment an item is
    /// accepted.
    ///
    /// The closing quote is stepped back over only when there is one: an unterminated literal has
    /// none, and the delimiter is read from the text rather than assumed, because `"` opens one too.
    pub(crate) fn inside_the_quotes(
        &self,
        quoted: phrost_model::TextRange,
    ) -> Option<phrost_model::TextRange> {
        let text = self.current()?.text();
        let opening = text.as_bytes().get(usize::from(quoted.start())).copied();
        let after_open = quoted.start() + TextSize::from(1);
        let mut closes_at = quoted.end().max(after_open);
        if let Some(quote) = opening
            && closes_at > after_open
            && text.as_bytes().get(usize::from(closes_at) - 1) == Some(&quote)
        {
            closes_at -= TextSize::from(1);
        }
        Some(phrost_model::TextRange::new(
            after_open,
            closes_at.max(after_open),
        ))
    }

    /// The catalogs a call reads the argument under the cursor as a key into.
    ///
    /// *Which* argument is a key is the callee's business and each binding says: nearly all of them
    /// mean the first, and `allowedIfHasPermission($account, $permission)` means the second. So the
    /// cursor's own position is counted and only the bindings that name it answer -- a cursor in the
    /// account is reading a plain string, and so is a cursor in the second argument of `get()`.
    fn call_catalogs_at(
        &self,
        offset: TextSize,
        quoted: phrost_model::TextRange,
    ) -> Option<Vec<&str>> {
        let hir = self.current()?.hir();
        let call = phrost_body::call_site_at(hir, offset)?;

        // Counted by how many arguments end before the literal begins, rather than by looking the
        // literal up in the list: the string the cursor is in may be unterminated, which is the whole
        // reason completion goes through a byte scan, and an unterminated one is not in the list at
        // all. A named or spread argument before it makes counting a guess, and PHP allows both.
        let before = call
            .arguments
            .iter()
            .take_while(|argument| argument.range.end() < quoted.start());
        let mut at = 0u16;
        for argument in before {
            if argument.name.is_some() || argument.spread {
                return None;
            }
            at = at.checked_add(1)?;
        }

        let contracts = self.skeleton.returns();
        let (receiver, name) = match &call.target {
            phrost_body::CallTarget::Member { receiver, name } => (receiver, name),
            // A function's half of the same binding: `view('posts.show')`, `app('router')`. Kept
            // here rather than beside the class's, because which argument the cursor is in is
            // counted once for both.
            phrost_body::CallTarget::Function(written) => {
                let binding =
                    contracts.function_argument_binding(hir.name_ref(*written).path.as_str())?;
                return (binding.at == at).then(|| vec![&*binding.catalog]);
            }
            _ => return None,
        };

        let class = self.class_of_held(
            quoted.start(),
            hir,
            &self.resolve_path(quoted.start(), hir, receiver, crate::view::FOLLOW_LIMIT)?,
        )?;

        let catalogs = contracts
            .argument_bindings(name.as_str())
            .iter()
            .filter(|binding| binding.at == at)
            .filter(|binding| {
                class.as_str().eq_ignore_ascii_case(&binding.class)
                    || self.descends_from(
                        class.as_str(),
                        &binding.class,
                        crate::resolve::HIERARCHY_DEPTH,
                    )
            })
            .map(|binding| &*binding.catalog)
            .collect::<Vec<_>>();

        (!catalogs.is_empty()).then_some(catalogs)
    }

    /// The catalogs an array entry's key reads its value as, when the cursor is in that value.
    ///
    /// `['#type' => 'text|field']`. Nothing here resolves anything: the key is the whole contract,
    /// so there is no receiver to follow and no hierarchy to walk -- which is also why this is tried
    /// second and costs nothing when the first answered.
    fn element_catalogs_at(&self, offset: TextSize) -> Option<Vec<&str>> {
        let contracts = self.skeleton.returns();
        // The cheap gate first, as everywhere else here: a project with no framework has bound no
        // array key, and finding the entry means walking the file's expressions.
        if !contracts.binds_any_array_key() {
            return None;
        }
        let element = phrost_body::keyed_element_at(self.current()?.hir(), offset)?;
        let catalogs = contracts
            .array_key_catalogs(element.key.as_str())
            .iter()
            .map(|catalog| &**catalog)
            .collect::<Vec<_>>();

        (!catalogs.is_empty()).then_some(catalogs)
    }

    /// The catalog key the cursor is on, and the class it names.
    ///
    /// The exact id rather than a prefix: this answers "what is this", where the completion answers
    /// "what could this be". `None` when the string is not a key of any catalog the call is bound to,
    /// which is the honest answer for an id nothing registers.
    pub fn catalog_key_at(&self, offset: TextSize) -> Option<crate::CursorTarget> {
        let (written, bindings) = self.bound_argument_at(offset)?;
        let open = self.current()?;
        let key = open
            .text()
            .get(usize::from(written.start())..usize::from(written.end()))?;

        let contracts = self.skeleton.returns();
        // A class first, because that is the answer with more in it -- and no catalog holds both
        // kinds, so which is asked first cannot change an answer.
        if let Some(class) = bindings
            .iter()
            .find_map(|catalog| contracts.catalog_class(catalog, key))
        {
            return Some(crate::CursorTarget::CatalogKey {
                key: key.into(),
                class: self.class_location(class)?.fqn,
                range: written,
            });
        }

        // A key of one of the bound catalogs, whether or not anything registered a place for it: a
        // route generated from an entity type's link template has a path and no file of its own, and
        // hovering it must say what it is rather than nothing.
        let catalog = bindings
            .iter()
            .find(|catalog| contracts.catalog_holds(catalog, key))?;
        Some(crate::CursorTarget::CatalogPlace {
            key: key.into(),
            detail: contracts.catalog_detail(catalog, key).unwrap_or("").into(),
            place: contracts.catalog_placed_at(catalog, key).cloned(),
            range: written,
        })
    }

    pub fn catalog_keys_at(
        &self,
        offset: TextSize,
        limit: usize,
    ) -> Option<(
        phrost_model::TextRange,
        Vec<crate::completion::CatalogCandidate>,
    )> {
        let (written, catalogs) = self.bound_argument_at(offset)?;
        let open = self.current()?;
        // Taken from the literal itself and not from the lexical context, which stops at the first
        // character that cannot be in an identifier -- and a service id is full of dots.
        let prefix = open
            .text()
            .get(usize::from(written.start())..usize::from(offset))
            .unwrap_or_default();

        let contracts = self.skeleton.returns();
        let items = catalogs
            .into_iter()
            .flat_map(|catalog| {
                contracts
                    .catalog_keys(catalog, prefix, limit)
                    .into_iter()
                    .map(move |key| crate::completion::CatalogCandidate {
                        key: key.into(),
                        // What the key names, whichever kind it is: the class for a service id, the
                        // URL for a route, the title for a permission. A list of a hundred route
                        // names with nothing beside them is unreadable.
                        //
                        // Empty for a key nothing describes -- a service built by a factory -- which
                        // is still a key worth offering, so this must not be what drops it.
                        detail: contracts.catalog_detail(catalog, key).unwrap_or("").into(),
                    })
            })
            .collect::<Vec<_>>();

        Some((written, items))
    }
}
