//! Which of the two settings sources wins, and what happens when one of them is nonsense.
//!
//! One behaviour per test and one file for the behaviour, because this is the question two people
//! will change for two different reasons: somebody adding a setting, and somebody changing what an
//! editor is allowed to say about one.

use phrost_project::ProjectConfig;
use phrost_testkit::MaterializedFixture;
use serde_json::json;

/// The whole resolved configuration, rendered so a test asserts on all of it rather than on the one
/// field it was thinking about.
///
/// A substring assertion here would let a merge that quietly dropped `format.style` pass, which is
/// exactly the class of regression this is for.
fn rendered(config: &ProjectConfig) -> String {
    fn list(name: &str, value: &Option<Vec<String>>) -> String {
        match value {
            None => format!("{name}: unset\n"),
            Some(items) => format!("{name}: [{}]\n", items.join(", ")),
        }
    }

    let mut out = String::new();
    out.push_str(&list("lint.categories", &config.lint.categories));
    out.push_str(&list("lint.off", &config.lint.off));
    out.push_str(&list("lint.on", &config.lint.on));
    out.push_str(&match &config.format.style {
        None => "format.style: unset\n".to_owned(),
        Some(style) => format!("format.style: {style}\n"),
    });
    out
}

const FILE_ONLY: &str = r#"
//- /phrost.toml
[lint]
categories = ["safety", "security"]
off = ["strict-types"]

[format]
style = "drupal"
"#;

#[test]
fn a_project_with_only_a_file_gets_what_the_file_says() {
    let fixture = MaterializedFixture::parse(FILE_ONLY);
    let (config, reasons) = ProjectConfig::resolve(fixture.root(), None);

    assert_eq!(reasons, Vec::<String>::new());
    assert_eq!(
        rendered(&config),
        "lint.categories: [safety, security]\n\
         lint.off: [strict-types]\n\
         lint.on: unset\n\
         format.style: drupal\n"
    );
}

#[test]
fn a_project_with_only_an_editor_gets_what_the_editor_says() {
    // No `phrost.toml` at all, which is the case this whole feature exists for: a person who wants
    // their own settings and commits nothing.
    let fixture = MaterializedFixture::parse("//- /composer.json\n{}\n");
    let editor = json!({
        "lint": {
            "categories": ["correctness"],
            "on": ["doc-missing-tags"],
        },
    });
    let (config, reasons) = ProjectConfig::resolve(fixture.root(), Some(&editor));

    assert_eq!(reasons, Vec::<String>::new());
    assert_eq!(
        rendered(&config),
        "lint.categories: [correctness]\n\
         lint.off: unset\n\
         lint.on: [doc-missing-tags]\n\
         format.style: unset\n"
    );
}

#[test]
fn the_editor_overrides_the_file_setting_by_setting() {
    // `categories` and `off` are named by both and the editor's answer stands. `on` is named by
    // neither, and `format.style` only by the file -- so the file's survives, which is what makes
    // this an override rather than a replacement of the whole configuration.
    let fixture = MaterializedFixture::parse(FILE_ONLY);
    let editor = json!({
        "lint": {
            "categories": ["correctness", "deprecation"],
            "off": [],
        },
    });
    let (config, reasons) = ProjectConfig::resolve(fixture.root(), Some(&editor));

    assert_eq!(reasons, Vec::<String>::new());
    assert_eq!(
        rendered(&config),
        "lint.categories: [correctness, deprecation]\n\
         lint.off: []\n\
         lint.on: unset\n\
         format.style: drupal\n"
    );
}

#[test]
fn a_misspelled_editor_setting_is_reported_and_the_file_stands() {
    // The failure this guards against: a typo in `.zed/settings.json` silently switching a project's
    // whole lint policy off, which reads as the feature not working rather than as the setting being
    // wrong. What was in force has to stay in force, and the misspelling has to be in the message.
    let fixture = MaterializedFixture::parse(FILE_ONLY);
    let editor = json!({ "lint": { "of": ["strict-types"] } });
    let (config, reasons) = ProjectConfig::resolve(fixture.root(), Some(&editor));

    assert_eq!(
        rendered(&config),
        "lint.categories: [safety, security]\n\
         lint.off: [strict-types]\n\
         lint.on: unset\n\
         format.style: drupal\n"
    );
    assert_eq!(
        reasons,
        ["initializationOptions: unknown field `of`, expected one of `categories`, `off`, `on`"]
    );
}
