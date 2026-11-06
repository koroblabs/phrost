//! Reading a fully-qualified PHP name as text.
//!
//! Splitting on `\` is not worth a type, but it is worth one implementation: three layers were
//! doing it, and a fourth would eventually disagree about a leading separator or an empty segment.

/// The last segment of a fully-qualified name.
///
/// What a person types, and therefore what completion matches against: nobody types
/// `Drupal\Core\Entity\` before the name they are after.
pub fn short_name(fqn: &str) -> &str {
    match fqn.rsplit_once('\\') {
        Some((_, short)) => short,
        None => fqn,
    }
}

/// The namespace part of a fully-qualified name, or `None` when it is in the global namespace.
pub fn namespace_of(fqn: &str) -> Option<&str> {
    fqn.rsplit_once('\\').map(|(namespace, _)| namespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_qualified_name_splits_at_the_last_separator() {
        assert_eq!(short_name("Drupal\\Core\\Entity\\Node"), "Node");
        assert_eq!(
            namespace_of("Drupal\\Core\\Entity\\Node"),
            Some("Drupal\\Core\\Entity")
        );
    }

    #[test]
    fn a_global_name_is_its_own_short_name() {
        assert_eq!(short_name("Drupal"), "Drupal");
        assert_eq!(namespace_of("Drupal"), None);
    }

    #[test]
    fn a_leading_separator_is_not_stripped_here() {
        // Callers store paths without one; this does not silently repair a caller that does not,
        // because a name that arrives with a separator is a bug worth seeing rather than hiding.
        assert_eq!(short_name("\\Drupal"), "Drupal");
        assert_eq!(namespace_of("\\Drupal"), Some(""));
    }
}
