//! What a plugin's docblock annotation becomes when it is written as an attribute.
//!
//! Drupal is midway through a move it started in 10.2: every plugin type that had an annotation
//! class under `Drupal\<module>\Annotation\` now has an attribute class beside it under
//! `Drupal\<module>\Attribute\`, with the same short name and the same arguments. So the segment is
//! the whole rule -- `@RestResource(...)` becomes `#[RestResource(...)]` and the class to import is
//! whichever indexed class is called `RestResource` and sits one segment along.
//!
//! What the segment cannot answer is the annotation written as an argument's *value*.
//! `label = @Translation("Region Notice.")` does not become an attribute: `@Translation` was never a
//! plugin, it was an object spelled in a docblock, and the attribute form writes that object out --
//! `label: new TranslatableMarkup('Region Notice.')`. There are four of those and they are listed
//! here, which is also what makes an unknown nested annotation a reason to *refuse* the conversion
//! rather than to guess at one.

use crate::rule::{AnnotationRewrite, AnnotationValueClass};

pub(super) static REWRITE: AnnotationRewrite = AnnotationRewrite {
    attributes_in: "Attribute",
    values: VALUES,
};

/// The annotations that stand for an object rather than for an attribute.
///
/// `Translation` is nearly all of them by count -- every plugin has a `label` -- and the context
/// definitions are the rest. A fifth would show up as this action declining to convert a class,
/// which is the failure that costs nothing.
static VALUES: &[AnnotationValueClass] = &[
    AnnotationValueClass {
        annotation: "Translation",
        constructs: "Drupal\\Core\\StringTranslation\\TranslatableMarkup",
    },
    AnnotationValueClass {
        annotation: "PluralTranslation",
        constructs: "Drupal\\Core\\StringTranslation\\PluralTranslatableMarkup",
    },
    AnnotationValueClass {
        annotation: "ContextDefinition",
        constructs: "Drupal\\Core\\Plugin\\Context\\ContextDefinition",
    },
    AnnotationValueClass {
        annotation: "EntityContextDefinition",
        constructs: "Drupal\\Core\\Plugin\\Context\\EntityContextDefinition",
    },
];

#[cfg(test)]
mod tests {
    use super::{REWRITE, VALUES};

    /// Every value class is named in full, and the plugin attributes deliberately are not.
    ///
    /// The asymmetry is the whole design and it fails silently in one direction: a value written as
    /// a bare short name would be searched for in the attribute namespace, find nothing, and refuse
    /// the conversion of whatever class happened to carry it -- an offer that quietly stops being
    /// made for one plugin type out of a hundred.
    #[test]
    fn an_annotation_that_stands_for_an_object_names_its_class_in_full() {
        for value in VALUES {
            assert!(
                value.constructs.contains('\\'),
                "{} constructs `{}`, which is not a qualified name",
                value.annotation,
                value.constructs
            );
        }
    }

    /// The segment is a segment and not a namespace.
    ///
    /// It is compared against one segment of a candidate's qualified name, so anything with a
    /// separator in it can never match and the conversion would simply never be offered.
    #[test]
    fn the_attribute_namespace_is_one_segment() {
        assert!(!REWRITE.attributes_in.contains('\\'));
        assert!(!REWRITE.attributes_in.is_empty());
    }
}
