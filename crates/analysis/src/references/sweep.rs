use phrost_view::IndexView;

/// What a project-wide sweep reads a file's text for, before it pays to analyse the file.
///
/// A prefilter and nothing more: a file it admits is analysed and may turn out to hold nothing,
/// and a file it refuses is never looked at again. So what it says has to be a superset of where a
/// use can be, and every rule that makes it narrower has to be a rule about where a use *cannot*
/// be.
///
/// Data, not a matcher: this crate does not scan text, and the layer reading the files does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SweepQuery {
    /// Words any occurrence of which may be a use.
    pub words: Vec<String>,
    /// How a constructor is reached, when the subject is one.
    pub construction: Option<ConstructionQuery>,
}

/// The ways into a file a constructor has, none of which may need to write the word `__construct`.
///
/// PHP runs one on `new Repo()`, on `new Child()` for every subclass declaring none of its own,
/// and on an explicit `parent::__construct()` -- so the sweep has to look for every class that
/// runs it. That is the whole subclass tree, two thousand names under a test base class, and
/// every one of those classes' own files writes its name in its declaration. Read as words, that
/// made each of them a candidate: a caller list for `TestCase::__construct` analysed 5 700 files
/// to find one call. The positions that cannot construct are told apart where the text is read --
/// see the reader for the rules.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConstructionQuery {
    /// Short names of the classes declaring the constructor, or standing in for it where the
    /// hierarchy declares none: a file declaring one of these is where it is written.
    pub declared_by: Vec<String>,
    /// Short names of the subclasses that run it because they declare none of their own.
    pub inherited_by: Vec<String>,
}

impl ConstructionQuery {
    /// The member's own name, which a file reaches it by only with `->__construct()` or
    /// `::__construct()`.
    pub const MEMBER: &'static str = "__construct";
}

impl SweepQuery {
    /// One word, for a subject every use of which writes it.
    pub(crate) fn word(word: String) -> Self {
        Self {
            words: vec![word],
            construction: None,
        }
    }

    /// The constructor `classes` declare, with every subclass that runs one of them.
    pub(crate) fn construction<'a>(
        view: &IndexView<'_>,
        classes: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        let mut construction = ConstructionQuery::default();
        for class in classes {
            construction
                .declared_by
                .push(phrost_model::short_name(class).to_owned());
            construction.inherited_by.extend(
                view.classes_reaching_constructor(class)
                    .iter()
                    .map(|named| phrost_model::short_name(named.as_str()).to_owned()),
            );
        }
        for names in [
            &mut construction.declared_by,
            &mut construction.inherited_by,
        ] {
            names.sort();
            names.dedup();
        }
        Self {
            words: Vec::new(),
            construction: Some(construction),
        }
    }

    /// Whether there is nothing to sweep for -- a subject no other file can hold.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty() && self.construction.is_none()
    }

    /// Every word a file has to hold one of to be read at all, sorted.
    pub fn every_word(&self) -> Vec<&str> {
        let mut words = self.words.iter().map(String::as_str).collect::<Vec<_>>();
        if let Some(construction) = &self.construction {
            words.push(ConstructionQuery::MEMBER);
            words.extend(construction.declared_by.iter().map(String::as_str));
            words.extend(construction.inherited_by.iter().map(String::as_str));
        }
        words.sort_unstable();
        words.dedup();
        words
    }
}
