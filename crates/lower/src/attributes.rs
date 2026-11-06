//! Which attributes carry a key somebody above reads, and what that buys them in lowering.
//!
//! Two things follow from being named here, and neither is free enough to do for every attribute.
//!
//! A *named* attribute argument is kept unconditionally, because there are few enough of them that
//! deciding costs more than keeping: a thirty-thousand-file project writes about a thousand. A
//! positional one is not like that. The same project writes 8 402 leading positional string
//! literals, of which 77 name anything a reader would ever ask about -- the rest are test-runner
//! groups, data providers and hook names. Keeping all of them would put eight thousand strings into
//! the resident skeleton to answer nothing.
//!
//! And the *older spelling* of the same declaration -- a Doctrine-style annotation in the docblock,
//! `@ContentEntityType(id = "node")` -- is read only for a name in this set. Reading every docblock
//! that writes an `@Something(...)` would parse the docblock of every declaration in the project to
//! collect a few thousand strings nothing consults.
//!
//! So the list is handed *down*, the way autoload rules are: whoever knows which attributes carry an
//! id says so, and this crate only compares names. Nothing here knows what an attribute means, and
//! nothing here may learn.
//!
//! One set for both, because both answer the same question -- is this attribute one whose arguments
//! somebody reads -- and two sets derived from one list of rules would be two chances to add a rule
//! to one and not the other. The argument names in [`KeyedAttributes::entering`] ride along for the
//! same reason: they are the *array* arguments a rule reads the keys of, and an attribute list built
//! without them would silently read no entries.

/// Attribute names whose arguments lowering goes out of its way to keep.
///
/// Empty by default, which is the answer for a project no framework was detected in: the gate then
/// costs one length check per attribute written on a declaration, and one per docblock.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct KeyedAttributes {
    /// Compared by the last segment, so the list holds short names. Sorting or hashing would cost
    /// more to build than a scan of two or three short strings costs to walk.
    names: Vec<Box<str>>,
    /// Argument names whose value is an array whose *entries* somebody reads.
    ///
    /// A separate and much stronger gate than [`Self::names`], because entering an array argument
    /// keeps a string per entry rather than one per argument: `links` on an entity type is five, and
    /// `handlers` beside it is a dozen class names nothing asks about.
    entered: Vec<Box<str>>,
    /// Property names whose array default is kept -- see [`phrost_hir::KeptTableData`].
    ///
    /// Not an attribute, and in this set for the reason the set exists: it is what lowering keeps
    /// for a reader above, handed down by whoever knows which ones are read. Compared exactly, as a
    /// property is written.
    defaults: Vec<Box<str>>,
    /// Attributes, by last segment, whose string arguments are a list kept the same way: the
    /// attribute spelling of what a kept default says.
    lists: Vec<Box<str>>,
    /// Method names whose body `return X::class;` says the class forwards to an `X` -- a `@mixin`
    /// written in code rather than in a docblock. Laravel's facade states its root this way in
    /// `getFacadeAccessor()`. Compared exactly, as a method is written.
    mixing_in: Vec<Box<str>>,
    /// Method names whose body builds objects that declare entries, and the classes -- by last
    /// segment -- whose construction is one: Twig's `getFilters()` building `TwigFilter`s. Read in
    /// every file, a dependency's included, and only in these methods.
    constructing_in: Vec<Box<str>>,
    constructed: Vec<Box<str>>,
}

impl KeyedAttributes {
    /// The set naming each of `names`, which may be written qualified or short.
    pub fn of(names: impl IntoIterator<Item: AsRef<str>>) -> Self {
        let mut set = Self::default();
        for name in names {
            let short = last_segment(name.as_ref());
            if !short.is_empty() && !set.names.iter().any(|held| &**held == short) {
                set.names.push(short.into());
            }
        }
        set
    }

    /// The same set, also entering the array arguments named here.
    pub fn entering(mut self, arguments: impl IntoIterator<Item: AsRef<str>>) -> Self {
        for argument in arguments {
            let argument = argument.as_ref();
            if !argument.is_empty() && !self.entered.iter().any(|held| &**held == argument) {
                self.entered.push(argument.into());
            }
        }
        self
    }

    /// The same set, also keeping the array default of the properties named here.
    pub fn keeping_defaults_of(mut self, properties: impl IntoIterator<Item: AsRef<str>>) -> Self {
        for property in properties {
            let property = property.as_ref();
            if !property.is_empty() && !self.defaults.iter().any(|held| &**held == property) {
                self.defaults.push(property.into());
            }
        }
        self
    }

    /// The same set, also keeping the string list the attributes named here write.
    pub fn keeping_lists_of(mut self, attributes: impl IntoIterator<Item: AsRef<str>>) -> Self {
        for attribute in attributes {
            let short = last_segment(attribute.as_ref());
            if !short.is_empty() && !self.lists.iter().any(|held| &**held == short) {
                self.lists.push(short.into());
            }
        }
        self
    }

    /// The same set, also reading the class the methods named here return as a mixin.
    pub fn mixing_in_returns_of(mut self, methods: impl IntoIterator<Item: AsRef<str>>) -> Self {
        for method in methods {
            let method = method.as_ref();
            if !method.is_empty() && !self.mixing_in.iter().any(|held| &**held == method) {
                self.mixing_in.push(method.into());
            }
        }
        self
    }

    /// The same set, also keeping what the methods named here construct of the classes named here.
    pub fn keeping_constructions(
        mut self,
        methods: impl IntoIterator<Item: AsRef<str>>,
        classes: impl IntoIterator<Item: AsRef<str>>,
    ) -> Self {
        for method in methods {
            let method = method.as_ref();
            if !method.is_empty() && !self.constructing_in.iter().any(|held| &**held == method) {
                self.constructing_in.push(method.into());
            }
        }
        for class in classes {
            let short = last_segment(class.as_ref());
            if !short.is_empty() && !self.constructed.iter().any(|held| &**held == short) {
                self.constructed.push(short.into());
            }
        }
        self
    }

    /// Whether a method named `method` builds objects whose construction is kept.
    pub(crate) fn keeps_constructions_in(&self, method: &str) -> bool {
        self.constructing_in
            .iter()
            .any(|name| name.eq_ignore_ascii_case(method))
    }

    /// Whether constructing the class written `class` is kept.
    pub(crate) fn keeps_construction_of(&self, class: &str) -> bool {
        let written = last_segment(class);
        self.constructed
            .iter()
            .any(|name| name.eq_ignore_ascii_case(written))
    }

    /// Whether the class a method named `method` returns is a mixin of the class declaring it.
    pub(crate) fn mixes_in_return_of(&self, method: &str) -> bool {
        self.mixing_in.iter().any(|name| &**name == method)
    }

    /// The kept-list attribute a written name is, by last segment and without regard to case.
    pub(crate) fn kept_list(&self, attribute: &str) -> Option<&str> {
        let written = last_segment(attribute);
        self.lists
            .iter()
            .find(|name| name.eq_ignore_ascii_case(written))
            .map(|name| &**name)
    }

    /// Whether a property's array default is worth keeping.
    pub(crate) fn keeps_default_of(&self, property: &str) -> bool {
        self.defaults.iter().any(|name| &**name == property)
    }

    /// Whether the entries of an array written at `argument` are worth keeping.
    ///
    /// Compared exactly: an argument name is written by the framework's own source, not resolved
    /// the way an attribute's path is.
    pub(crate) fn wants_entries_of(&self, argument: &str) -> bool {
        self.entered.iter().any(|name| &**name == argument)
    }

    /// The array arguments to enter, for a walk that cannot call back into this.
    pub(crate) fn entered(&self) -> &[Box<str>] {
        &self.entered
    }

    /// Whether a literal written on `attribute` is worth keeping.
    ///
    /// By the last segment and without regard to case, which is how PHP resolves an attribute and
    /// how it is written: `#[FormElement('x')]` and `#[\Drupal\Core\Render\Attribute\FormElement('x')]`
    /// are the same attribute, and both appear in real code.
    pub(crate) fn wants(&self, attribute: &str) -> bool {
        if self.names.is_empty() {
            return false;
        }
        let written = last_segment(attribute);
        self.names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(written))
    }

    /// Whether any attribute in these lists is one this set names: a declaration a framework
    /// marks, like Laravel's `#[Scope]` on a method.
    pub(crate) fn marks(
        &self,
        lists: &mago_syntax::cst::Sequence<'_, mago_syntax::cst::AttributeList<'_>>,
    ) -> bool {
        !self.names.is_empty()
            && lists.iter().any(|list| {
                list.attributes.iter().any(|attribute| {
                    self.wants(&phrost_syntax::identifier_text(attribute.name.value()))
                })
            })
    }

    /// Whether a docblock is worth parsing for an annotation this set names.
    ///
    /// The prefilter in front of the annotation reader, and the reason reading the older spelling
    /// costs a project nothing until it uses one: a docblock that does not write the name cannot
    /// carry the annotation, and nearly none of them do. Case-sensitive, unlike [`Self::wants`],
    /// because this only decides whether to look -- the tag the parser finds is compared properly,
    /// and a docblock in the wrong case would be rejected there rather than mis-read.
    pub(crate) fn mentioned_in(&self, docblock: &str) -> bool {
        self.names.iter().any(|name| docblock.contains(&**name))
    }
}

/// The last `\`-separated piece of a written name.
fn last_segment(name: &str) -> &str {
    name.rsplit('\\').next().unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_attribute_is_wanted_by_its_last_segment_however_it_is_written() {
        let wanted = KeyedAttributes::of(["FormElement", "Drupal\\Core\\Render\\RenderElement"]);

        assert!(wanted.wants("FormElement"));
        assert!(wanted.wants("Drupal\\Core\\Render\\Attribute\\FormElement"));
        assert!(wanted.wants("formelement"), "PHP resolves it either way");
        assert!(wanted.wants("RenderElement"));
        assert!(!wanted.wants("Group"));
        assert!(!wanted.wants("Element"));
    }

    #[test]
    fn a_project_with_no_framework_wants_nothing() {
        assert!(!KeyedAttributes::default().wants("FormElement"));
        // An empty name is not a name, and would otherwise match every unqualified attribute.
        assert!(!KeyedAttributes::of([""]).wants("FormElement"));
    }

    #[test]
    fn a_docblock_is_worth_reading_only_when_it_writes_one_of_the_names() {
        let wanted = KeyedAttributes::of(["ContentEntityType"]);

        assert!(wanted.mentioned_in("/**\n * @ContentEntityType(id = \"node\")\n */"));
        assert!(!wanted.mentioned_in("/**\n * @param int $id\n */"));
        // The default, and the whole reason the prefilter is cheap: a project with no framework
        // never looks at a docblock for this at all.
        assert!(!KeyedAttributes::default().mentioned_in("@ContentEntityType(id = \"node\")"));
    }
}
