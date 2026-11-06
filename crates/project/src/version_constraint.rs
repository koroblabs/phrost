use phrost_model::PhpVersion;

/// A raw composer version constraint string, as written under `require`.
///
/// Composer constraints are a small DSL for resolving package versions against a full release
/// registry — caret and tilde ranges, wildcards, AND-lists, OR-alternatives. Answering "what is
/// the floor of this range" in general needs that resolver. This type only answers the one
/// question phrost actually needs — does this constraint unambiguously *name* a minimum version —
/// and refuses rather than guesses whenever the shape does not say. A wrong "detected" minimum
/// would make every version-gated diagnostic built on top of it wrong silently, which is worse
/// than admitting we do not know.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VersionConstraint<'a>(&'a str);

impl<'a> VersionConstraint<'a> {
    pub fn new(text: &'a str) -> Self {
        Self(text)
    }

    /// The minimum PHP version this constraint expresses, or `None` when it does not name one.
    pub fn minimum_php_version(&self) -> Option<PhpVersion> {
        let constraint = self.0.trim();

        // An OR-alternative set (`^7.4 || ^8.0`) needs full range arithmetic to find the true
        // floor across every alternative; refuse rather than naively taking the first one, which
        // could easily be the *higher* floor of the two.
        if constraint.contains("||") {
            return None;
        }

        // An AND-list (`>=8.1,<9.0`) is a common way to spell "this supported range". Exactly one
        // segment may name a lower bound; every other segment is either a pure upper bound
        // (`<9.0`) or an exclusion, and contributes nothing to the floor.
        let mut lower_bound = None;
        for segment in constraint.split(',') {
            let segment = segment.trim();
            if segment.is_empty() || is_upper_bound_or_exclusion(segment) {
                continue;
            }
            if lower_bound.is_some() {
                // Two lower-bound-shaped segments in one AND-list is not a shape composer
                // constraints actually produce in practice; refuse rather than pick one.
                return None;
            }
            lower_bound = Some(segment);
        }

        parse_lower_bound(lower_bound?)
    }
}

fn is_upper_bound_or_exclusion(segment: &str) -> bool {
    segment.starts_with('<') || segment.starts_with('!') || segment == "*"
}

/// Reads the floor out of one constraint segment already known to name a lower bound, or `None`
/// if it turns out not to (a bare `>8.1` is an *exclusive* lower bound with no exact floor to
/// name, so it is refused here rather than upstream, where it looks lower-bound-shaped).
fn parse_lower_bound(segment: &str) -> Option<PhpVersion> {
    let numeral = if let Some(rest) = segment.strip_prefix(">=") {
        rest
    } else if let Some(rest) = segment.strip_prefix('^') {
        rest
    } else if let Some(rest) = segment.strip_prefix('~') {
        rest
    } else if let Some(rest) = segment.strip_prefix('=') {
        rest
    } else if segment.starts_with(|ch: char| ch.is_ascii_digit()) {
        segment
    } else {
        return None;
    };

    let numeral = numeral.trim();
    let numeral = numeral.strip_suffix(".*").unwrap_or(numeral);
    let mut parts = numeral.split('.');
    let major: u8 = parts.next()?.parse().ok()?;
    let minor: u8 = match parts.next() {
        Some(text) => text.parse().ok()?,
        None => 0,
    };
    // A third, patch component is dropped rather than rejected: `PhpVersion` only models
    // major.minor by design, because no PHP syntax or type rule has ever turned on a patch
    // version, and a composer constraint naming one (`^8.1.2`) still expresses a clear
    // major.minor floor once the patch is set aside.
    Some(PhpVersion::new(major, minor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_caret_constraint_names_its_version_as_the_floor() {
        assert_eq!(
            VersionConstraint::new("^8.3").minimum_php_version(),
            Some(PhpVersion::new(8, 3))
        );
    }

    #[test]
    fn a_greater_or_equal_constraint_names_its_version_as_the_floor() {
        assert_eq!(
            VersionConstraint::new(">=8.1").minimum_php_version(),
            Some(PhpVersion::new(8, 1))
        );
    }

    #[test]
    fn a_wildcard_constraint_names_its_fixed_prefix_as_the_floor() {
        assert_eq!(
            VersionConstraint::new("8.3.*").minimum_php_version(),
            Some(PhpVersion::new(8, 3))
        );
    }

    #[test]
    fn a_tilde_constraint_names_its_version_as_the_floor() {
        assert_eq!(
            VersionConstraint::new("~8.1").minimum_php_version(),
            Some(PhpVersion::new(8, 1))
        );
    }

    #[test]
    fn a_patch_component_is_dropped_rather_than_rejected() {
        // `PhpVersion` has no patch field; truncating here matches that type's own documented
        // reasoning rather than inventing a new rule.
        assert_eq!(
            VersionConstraint::new("^8.1.2").minimum_php_version(),
            Some(PhpVersion::new(8, 1))
        );
    }

    #[test]
    fn an_and_list_takes_its_floor_from_the_one_lower_bound_segment() {
        assert_eq!(
            VersionConstraint::new(">=8.1,<9.0").minimum_php_version(),
            Some(PhpVersion::new(8, 1))
        );
    }

    #[test]
    fn an_or_alternative_set_is_refused_rather_than_guessed() {
        // The true floor across `^7.4 || ^8.0` is 7.4, but naively taking the first alternative
        // would happen to get that right for the wrong reason; a set ordered the other way
        // (`^8.0 || ^7.4`) would silently produce the wrong floor. Refuse the whole shape.
        assert_eq!(
            VersionConstraint::new("^7.4 || ^8.0").minimum_php_version(),
            None
        );
    }

    #[test]
    fn an_exclusive_lower_bound_is_refused_because_it_names_no_exact_floor() {
        // `>8.1` excludes 8.1 itself; the true floor is "the version right after 8.1", which does
        // not exist as a concrete value to report.
        assert_eq!(VersionConstraint::new(">8.1").minimum_php_version(), None);
    }

    #[test]
    fn a_bare_wildcard_is_refused() {
        assert_eq!(VersionConstraint::new("*").minimum_php_version(), None);
    }

    #[test]
    fn blank_and_garbage_input_is_refused_without_panicking() {
        assert_eq!(VersionConstraint::new("").minimum_php_version(), None);
        assert_eq!(
            VersionConstraint::new("not-a-version").minimum_php_version(),
            None
        );
    }
}
