/// PHP member visibility.
///
/// Ordered from most to least visible so that "at least as visible as" is a comparison rather
/// than a match arm that someone will eventually get backwards.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum Visibility {
    #[default]
    Public,
    Protected,
    Private,
}

impl Visibility {
    /// Whether a member with this visibility is reachable from `context`.
    ///
    /// `context` is the visibility level the accessing scope has earned: `Private` inside the
    /// declaring class, `Protected` inside a subclass, `Public` from outside.
    pub fn is_visible_from(self, context: Visibility) -> bool {
        context >= self
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Protected => "protected",
            Self::Private => "private",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Visibility::{self, Private, Protected, Public};

    #[test]
    fn access_context_decides_what_is_reachable() {
        // From outside the hierarchy only public members are reachable.
        assert!(Public.is_visible_from(Public));
        assert!(!Protected.is_visible_from(Public));
        assert!(!Private.is_visible_from(Public));

        // A subclass adds protected, but not private.
        assert!(Protected.is_visible_from(Protected));
        assert!(!Private.is_visible_from(Protected));

        // The declaring class sees everything.
        assert!(Private.is_visible_from(Private));
        assert!(Public.is_visible_from(Private));
    }

    #[test]
    fn ordering_runs_from_most_to_least_visible() {
        let mut levels = vec![Private, Public, Protected];
        levels.sort();
        assert_eq!(levels, vec![Public, Protected, Private]);
        assert_eq!(Visibility::default(), Public);
    }
}
