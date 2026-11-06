//! A form's element keys, read where the form state is asked for what was submitted.
//!
//! `buildForm(array $form, FormStateInterface $form_state)` writes `$form['email'] = [...]`, and the
//! same class's `submitForm()` and `validateForm()` read `$form_state->getValue('email')` -- by the
//! element's own key, at whatever depth it was written, which is Drupal's default and the only one
//! a string argument can express. 217 such reads in the measured project's own forms, and until this
//! each was a string with nowhere to go from it; 201 of them now go to their element.

use crate::rule::BuiltKey;

/// The form state's accessors whose first argument is an element's key.
///
/// `setErrorByName()` is not one: it takes the element's *name*, which nests with `][` and is a
/// different spelling of the same thing.
pub(super) static BUILT_KEYS: &[BuiltKey] = &[
    reading("getValue"),
    reading("setValue"),
    reading("hasValue"),
    reading("unsetValue"),
    reading("isValueEmpty"),
];

/// A form state accessor reading an element of a render array: an array with a `#` property, whose
/// `#type` says what it is and whose `#parents` can file its value under another key.
const fn reading(method: &'static str) -> BuiltKey {
    BuiltKey {
        class: "Drupal\\Core\\Form\\FormStateInterface",
        method,
        property: "#",
        described_by: "#type",
        filed_under: "#parents",
    }
}
