//! `strict-behavior` says "comparison" of a function that compares nothing.
//!
//! One sentence for four functions, and `base64_decode` is the one it does not fit: its `$strict`
//! is about characters outside the alphabet, not about `==`. See [`super::MAGO_MISSTATEMENTS`].

use expect_test::expect;

use super::*;

fn findings(source: &str) -> String {
    let issues = lint(
        source,
        "src/Runner.php",
        &[],
        None,
        LintSelection {
            on: &["strict-behavior".to_owned()],
            ..Default::default()
        },
    );
    if issues.is_empty() {
        return "no findings\n".to_owned();
    }
    issues
        .iter()
        .map(|issue| {
            format!(
                "{:?} {}: {}\n  help: {}\n",
                issue.severity,
                issue.code,
                issue.message,
                issue.help.as_deref().unwrap_or("(none)"),
            )
        })
        .collect()
}

/// The finding and its advice are mago's and are right; only the headline named the wrong thing.
#[test]
fn a_decode_is_asked_to_decode_strictly() {
    expect![[r#"
        Warning strict-behavior: Call to `base64_decode` must enforce strict decoding.
          help: Call the function `base64_decode` with the `$strict` parameter set to `true`.
    "#]]
    .assert_eq(&findings("<?php\n\n$raw = base64_decode($payload);\n"));
}

/// The three that do compare keep the sentence as mago wrote it.
#[test]
fn a_search_is_still_asked_to_compare_strictly() {
    expect![[r#"
        Warning strict-behavior: Call to `in_array` must enforce strict comparison.
          help: Call the function `in_array` with the `$strict` parameter set to `true`.
    "#]]
    .assert_eq(&findings(
        "<?php\n\n$found = in_array($needle, $haystack);\n",
    ));
}
