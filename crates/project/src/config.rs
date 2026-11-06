//! What a project says about the choices its tools cannot make for it.
//!
//! Two sources say it. A `phrost.toml` at the project root is the committed one, and a client's
//! `initializationOptions` -- what `.zed/settings.json` puts under `lsp.phrost.initialization_options`
//! -- is the personal one. Every setting in either is a decision only the project can make, and every
//! one has a default that a framework or this crate supplies, so both are optional and neither is the
//! only source.
//!
//! # What it can say
//!
//! ```toml
//! # Switch a framework on that detection did not find. Added to what was detected, never
//! # replacing it; by the id a provider reports.
//! framework = ["drupal"]
//!
//! [files]
//! # Not this project's code. Added to what is skipped already -- a version store, a JavaScript
//! # dependency tree, PHP's own tool caches, and whatever the framework said it generates.
//! exclude = ["docroot/sites/legacy", "tools/fixtures"]
//! # Code, whatever else says otherwise: this list beats every exclusion above *and* the project's
//! # own ignore files, because a `.gitignore` line is what usually hides a real source tree.
//! include = ["docroot/core/lib"]
//!
//! [lint]
//! categories = ["correctness"]
//! on = ["doc-missing-tags"]
//! # Any code a report shows, from the syntax rules or the type checks alike.
//! off = ["doc-undefined-type"]
//!
//! [diagnostics]
//! project = false
//!
//! # A template a page includes by a computed name -- `{% include section %}` -- sees that page's
//! # variables, and nothing in either file says which page that is.
//! [[twig.context]]
//! templates = "@mine/sections/**"
//! from = "@mine/page.html.twig"
//! ```
//!
//! Paths are relative to the project root and matched by prefix, so naming a directory covers
//! everything under it. Everything above is optional.
//!
//! # Why there is one at all
//!
//! Measured on a real Drupal project: the syntax rules find 6 855 things with every category turned
//! on, and the largest single rule is `strict-types` at 987 -- a rule that says to write
//! `declare(strict_types=1)`, which Drupal core deliberately does not. Category alone cannot express
//! that: `strict-types` is a *correctness* rule, and correctness is the category a person most wants.
//! So a project has to be able to name one rule.
//!
//! # Why there are two
//!
//! `phrost check` and `phrost perf` have no editor, so the file has to keep working or the whole
//! measurement loop stops. A team that wants a committed, shared policy keeps the file; a person who
//! wants their own settings puts them in the editor and commits nothing. Both, together, are the
//! common case.
//!
//! # One construction path
//!
//! The effective settings are built once by [`ProjectConfig::resolve`], from the defaults a framework
//! contributed and then the file and then the editor -- each later source overriding the earlier one,
//! because the editor is the more specific statement of the two and a person must be able to
//! *contradict* the committed file rather than only agree with it harder.
//!
//! Overriding is per setting, not per source and not per table: a source that names a setting
//! replaces it outright, and a source that stays silent about one leaves whatever the earlier source
//! said. So an editor that names only `lint.off` keeps the file's `lint.categories`, and an editor
//! that says `off = []` really does mean *nothing is off*, which is the only way to switch a rule back
//! on that the file switched off. `on` and `off` behave identically, and so does everything else here.
//!
//! There is deliberately no second path where a caller consults a source itself: two places that
//! decide the same thing disagree, and the disagreement shows up as a rule that is off in the editor
//! and on in the checker. That is why [`ProjectConfig::resolve`] is the only public way to get one,
//! and why reading the file is private to this module.
//!
//! # A source that does not parse
//!
//! Reported, and then ignored -- for both sources and for the same reason. Refusing to start over a
//! typo would be worse than carrying on, and silently dropping the settings would be worse still: a
//! rule that is off because of a misspelling looks exactly like a rule that does not work. What is in
//! force stays in force, so a broken editor payload cannot switch a project's whole policy off.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The file's name, at the project root.
pub const CONFIG_FILE: &str = "phrost.toml";

/// What a client's settings are called in a message about them.
///
/// The protocol's own name for the field, not an editor's name for the place it is written: every
/// client has its own spelling of the latter, and only the former is the same thing everywhere.
const EDITOR_SOURCE: &str = "initializationOptions";

/// Everything a project may say.
///
/// Every field is optional: a source exists to override, so a setting it does not mention has to
/// leave the earlier source alone rather than reset it. That is why nothing here is a plain value --
/// including the two lists, because `[]` and *unmentioned* are different answers and a merge that
/// cannot tell them apart cannot honour either.
///
/// The same field names serve TOML and JSON. `kebab-case` renaming is on for both and no name here
/// has two words, so a person moving a setting from one to the other retypes nothing.
#[derive(Clone, Default, PartialEq, Eq, Debug, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct ProjectConfig {
    pub lint: LintConfig,
    pub format: FormatConfig,
    pub catalog: CatalogConfig,
    pub diagnostics: DiagnosticsConfig,
    /// Frameworks to switch on whatever detection found.
    ///
    /// By the id a provider reports -- `drupal`, `laravel`, `symfony` -- and **added** to what was
    /// detected rather than replacing it. Detection probes the filesystem, so it answers no for a
    /// layout nobody anticipated: a Drupal docroot under a name the provider does not look for, an
    /// application assembled by hand, a package that holds a framework's classes without being an
    /// application. Naming one here is how a project says so.
    ///
    /// Added and not substituted, because a project naming `drupal` has said nothing about the
    /// `symfony` beside it, and turning that off would be answering a question nobody asked.
    pub framework: Option<Vec<String>>,
    pub files: FilesConfig,
    pub twig: TwigConfig,
}

/// What a project says about its Twig templates that no file of it says.
#[derive(Clone, Default, PartialEq, Eq, Debug, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct TwigConfig {
    /// Templates included by a name the page computes, and the page whose variables they see.
    pub context: Option<Vec<TwigContext>>,
}

/// Templates whose variables are another template's.
///
/// `{% include section %}` hands the included template every variable of the page including it,
/// and when the name is computed -- read from a list the PHP built -- no reading of either file can
/// say which templates that is. This says it.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct TwigContext {
    /// The included templates, by the name a template loads them by, with `*` for any part of one
    /// directory's name and `**` for any number of directories: `@mine/sections/**`.
    pub templates: String,
    /// The template including them, by name: `@mine/page.html.twig`.
    pub from: String,
}

/// Which directories a project takes out of the walk, and which it takes back.
///
/// Both are paths relative to the project root, matched by prefix -- naming a directory covers
/// everything under it.
#[derive(Clone, Default, PartialEq, Eq, Debug, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct FilesConfig {
    /// Directories that are not this project's code, **added** to what is skipped already.
    ///
    /// What is skipped already is a version store, a JavaScript dependency tree, the caches PHP's
    /// own tools keep, and whatever the detected framework said it generates. This is for the rest:
    /// a vendored copy of another project, a fixtures tree, a directory of generated PHP nobody
    /// reads.
    pub exclude: Option<Vec<String>>,
    /// Directories to index whatever else says otherwise.
    ///
    /// Beats every exclusion -- this list, a framework's, the built-in names, **and the project's
    /// own ignore files**. The last is the useful half: a `.gitignore` line is what keeps most
    /// generated PHP out of an index, and it is also what hides a real source tree in a checkout
    /// that keeps its dependencies out of git.
    ///
    /// Walked with ignore files off, the same way a composer root outside `vendor/` already is --
    /// see [`crate::discovery`]. So naming a directory here is a statement that it holds code,
    /// strong enough to outrank a rule that was written about something else.
    pub include: Option<Vec<String>>,
}

/// Which files are checked and reported.
#[derive(Clone, Default, PartialEq, Eq, Debug, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct DiagnosticsConfig {
    /// Whether every file the project owns is checked in the background after indexing and
    /// whenever one changes on disk, so the problems panel shows the whole project and not only the
    /// open tabs. On unless a project says otherwise; `false` keeps checking to what is open.
    pub project: Option<bool>,
}

/// Which syntax rules a project wants to hear about.
#[derive(Clone, Default, PartialEq, Eq, Debug, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct LintConfig {
    /// Categories reported without being asked. `None` leaves whatever the earlier source said.
    pub categories: Option<Vec<String>>,
    /// Rules never reported, whatever their category. By the code a report shows -- a syntax rule's
    /// or a type check's, since a person naming one does not care which engine implements it.
    pub off: Option<Vec<String>>,
    /// Rules always reported, whatever their category. The other half of naming one rule: a project
    /// that wants `no-eval` and nothing else from safety says so here rather than by taking the
    /// category and excluding forty.
    ///
    /// It also names the index's own checks that are off by default -- `doc-missing-tags` and
    /// `doc-param-name`, which are house standards rather than facts about the code. One list, and
    /// deliberately: a person naming a rule does not care which engine implements it.
    pub on: Option<Vec<String>>,
}

/// Where a project's own methods take a name a catalog already holds.
///
/// A framework names its own accessors: `$node->get('field_amount')` is bound to the field catalog by
/// the Drupal provider, so the name completes and a typo in it is visible. A project that wraps those
/// accessors loses both, and wrapping them is the norm rather than the exception -- one real checkout
/// routes 171 literal field names through four methods of one trait, against 509 written against
/// Drupal's own. Nothing below the seam may name a project's class, and nothing in the seam may name a
/// customer's, so the only place the fact can be stated is here.
///
/// A binding may name a **trait**, which is what keeps this short: `descends_from` walks `uses`, so
/// one entry per wrapper method covers every class that uses the trait.
///
/// ```toml
/// [[catalog.argument]]
/// class = "Drupal\\my_app\\EntityHelperTrait"
/// method = "_field"
/// catalog = "field"
/// ```
///
/// # What it cannot do
///
/// Bind to a catalog the project does not have -- an unknown name simply matches nothing, which is
/// the same silence as not writing the entry. And it must not be pointed at a **closed** catalog
/// unless the argument is *always* a key of it: a closed catalog reports what it does not hold, so
/// "sometimes a key" becomes an error on correct code. The open catalogs offer and never report,
/// which is why field names are the safe and useful case.
#[derive(Clone, Default, PartialEq, Eq, Debug, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct CatalogConfig {
    /// One entry per method whose argument is a catalog key. `None` leaves whatever the earlier
    /// source said, as every setting here does.
    pub argument: Option<Vec<CatalogArgument>>,
}

/// One method argument that is a key of one catalog.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct CatalogArgument {
    /// The class, interface or trait declaring the method. Anything below it is covered.
    ///
    /// Fully qualified and **without** a leading separator, which is how every name the index holds
    /// is spelled -- one written `\\App\\Foo` matches nothing, silently.
    pub class: String,
    /// The method's own name.
    pub method: String,
    /// Which argument, counting from zero.
    #[serde(default)]
    pub at: u16,
    /// The catalog the argument is a key into, by the name the framework gave it.
    pub catalog: String,
    /// What the method appends to the argument before looking it up. Almost always empty -- Drupal's
    /// `alter($type)` running `hook_{$type}_alter` is the shape it exists for.
    #[serde(default)]
    pub suffix: String,
}

/// How a project wants its code printed.
#[derive(Clone, Default, PartialEq, Eq, Debug, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct FormatConfig {
    /// A house style by name. `None` leaves the one the earlier source named.
    pub style: Option<String>,
}

impl ProjectConfig {
    /// The settings in force at a project root: the file, then whatever the editor said.
    ///
    /// The only way to build one, so that nothing can decide these questions twice and come to two
    /// answers. `editor` is the client's `initializationOptions` verbatim; `None` and JSON `null` both
    /// mean the client said nothing, which is what every client that has never heard of this server
    /// sends.
    ///
    /// The reasons a source was ignored come back alongside, one per source, in the order the sources
    /// are read. An empty list means both were understood.
    pub fn resolve(root: &Path, editor: Option<&serde_json::Value>) -> (Self, Vec<String>) {
        let mut reasons = Vec::new();

        let (mut config, file_error) = Self::from_file(root);
        reasons.extend(file_error);

        match Self::from_editor(editor) {
            Ok(Some(editor)) => config = config.overridden_by(editor),
            // Nothing said, which is not the same as saying nothing is set: the file stands.
            Ok(None) => {}
            Err(reason) => reasons.push(reason),
        }

        (config, reasons)
    }

    /// Reads the file at a project root, or the defaults when there is none.
    fn from_file(root: &Path) -> (Self, Option<String>) {
        let path = root.join(CONFIG_FILE);
        let Ok(bytes) = std::fs::read(&path) else {
            return (Self::default(), None);
        };
        // Lossy, for the reason every other configuration read here is: a hand-edited file can carry
        // a stray byte, and failing on it would drop every setting the file makes.
        let text = String::from_utf8_lossy(&bytes);
        match toml::from_str::<Self>(&text) {
            Ok(config) => (config, None),
            Err(error) => (
                Self::default(),
                Some(format!("{CONFIG_FILE}: {}", one_line(&error.to_string()))),
            ),
        }
    }

    /// Reads what the client sent, or nothing when it sent nothing.
    ///
    /// An error rather than a silent default, because the whole failure this guards against is a
    /// misspelled key in a settings file switching a project's lint policy off and looking like the
    /// feature not working. `deny_unknown_fields` is what makes the misspelling visible, and it is on
    /// for the same reason here as it is for the file.
    fn from_editor(editor: Option<&serde_json::Value>) -> Result<Option<Self>, String> {
        let Some(value) = editor.filter(|value| !value.is_null()) else {
            return Ok(None);
        };
        Self::deserialize(value)
            .map(Some)
            .map_err(|error| format!("{EDITOR_SOURCE}: {}", one_line(&error.to_string())))
    }

    /// This configuration with every setting the later source names replaced by it.
    ///
    /// Per setting, so a source that names one thing does not silently reset the rest -- and outright,
    /// so a named list is the whole answer. Extending instead would make it impossible to un-name a
    /// rule the earlier source named, which is exactly what "the editor is more specific" has to mean.
    fn overridden_by(self, over: Self) -> Self {
        Self {
            lint: LintConfig {
                categories: over.lint.categories.or(self.lint.categories),
                off: over.lint.off.or(self.lint.off),
                on: over.lint.on.or(self.lint.on),
            },
            format: FormatConfig {
                style: over.format.style.or(self.format.style),
            },
            catalog: CatalogConfig {
                argument: over.catalog.argument.or(self.catalog.argument),
            },
            diagnostics: DiagnosticsConfig {
                project: over.diagnostics.project.or(self.diagnostics.project),
            },
            framework: over.framework.or(self.framework),
            files: FilesConfig {
                exclude: over.files.exclude.or(self.files.exclude),
                include: over.files.include.or(self.files.include),
            },
            twig: TwigConfig {
                context: over.twig.context.or(self.twig.context),
            },
        }
    }

    /// The directories the project takes out of the walk, resolved against its root.
    ///
    /// Absolute, because that is what every reader compares against: a discovered path and an
    /// opened buffer are both absolute, and the two comparisons have to agree or a directory is
    /// skipped by the walk and accepted as a buffer.
    pub fn excluded_paths(&self, root: &Path) -> Vec<PathBuf> {
        Self::under(root, self.files.exclude.as_deref())
    }

    /// The directories the project takes back, resolved the same way.
    pub fn included_paths(&self, root: &Path) -> Vec<PathBuf> {
        Self::under(root, self.files.include.as_deref())
    }

    /// A configured list against a root, dropping what cannot be a path under it.
    ///
    /// An absolute entry is kept as written -- somebody naming `/srv/shared/lib` means that -- and a
    /// relative one is joined. `..` is not resolved: a path that climbs out of the root would be
    /// compared by prefix against files that are all inside it and match nothing, which is the same
    /// answer as refusing it and needs no rule of its own.
    fn under(root: &Path, named: Option<&[String]>) -> Vec<PathBuf> {
        named
            .unwrap_or_default()
            .iter()
            .filter(|entry| !entry.trim().is_empty())
            .map(|entry| {
                let path = Path::new(entry);
                match path.is_absolute() {
                    true => path.to_path_buf(),
                    false => root.join(path),
                }
            })
            .collect()
    }
}

/// A parse error on one line, for a log that has one line to give it.
///
/// The whole message and not its first line: `toml` puts the position first and *what is wrong* last,
/// so keeping the beginning keeps the least useful half. A person who misspelled a setting needs to
/// see the misspelling.
fn one_line(message: &str) -> String {
    message
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && *line != "|")
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use phrost_testkit::MaterializedFixture;
    use serde_json::json;

    /// The shape a project writes to say which page a template included by a computed name sees
    /// the variables of -- in either source, with the editor's replacing the file's.
    #[test]
    fn a_twig_context_names_the_templates_and_the_page() {
        let fixture = MaterializedFixture::parse(
            "//- /phrost.toml\n[[twig.context]]\ntemplates = \"@mine/sections/**\"\nfrom = \"@mine/page.html.twig\"\n",
        );
        let (config, reasons) = ProjectConfig::resolve(fixture.root(), None);
        assert_eq!(reasons, Vec::<String>::new());
        assert_eq!(
            config.twig.context,
            Some(vec![TwigContext {
                templates: "@mine/sections/**".to_owned(),
                from: "@mine/page.html.twig".to_owned(),
            }])
        );
        let (config, _) =
            ProjectConfig::resolve(fixture.root(), Some(&json!({ "twig": { "context": [] } })));
        assert_eq!(config.twig.context, Some(Vec::new()));
    }

    #[test]
    fn a_project_with_no_file_and_no_editor_gets_the_defaults() {
        let fixture = MaterializedFixture::parse("//- /composer.json\n{}\n");
        let (config, reasons) = ProjectConfig::resolve(fixture.root(), None);
        assert_eq!(config, ProjectConfig::default());
        assert_eq!(reasons, Vec::<String>::new());
    }

    #[test]
    fn a_client_that_sends_null_is_a_client_that_said_nothing() {
        // What every client that has never heard of this server sends, and it must not read as
        // "everything is set to nothing".
        let fixture =
            MaterializedFixture::parse("//- /phrost.toml\n[lint]\noff = [\"strict-types\"]\n");
        let (config, reasons) = ProjectConfig::resolve(fixture.root(), Some(&json!(null)));
        assert_eq!(reasons, Vec::<String>::new());
        assert_eq!(
            config.lint.off.as_deref(),
            Some(["strict-types".to_owned()].as_slice())
        );
    }

    /// The shape a project writes to bind its own wrapper to a framework's catalog.
    ///
    /// Pinned as a *file* rather than only as a struct, because the whole value of the setting is
    /// that a person can write it: `deny_unknown_fields` means a misspelt key is an error rather than
    /// a silently ignored line, and a table array is the one TOML shape where getting the brackets
    /// wrong parses as something else entirely.
    ///
    /// The class here is a **trait**, which is the case that makes the setting short: one entry per
    /// wrapper method covers every class that uses it, because ancestry walks `uses`.
    #[test]
    fn a_project_binds_its_own_wrapper_to_a_catalog_the_framework_filled() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /phrost.toml
[[catalog.argument]]
class = 'Drupal\App\EntityHelperTrait'
method = '_field'
catalog = 'field'

[[catalog.argument]]
class = 'Drupal\App\EntityHelperTrait'
method = '_value'
catalog = 'field'
"#,
        );
        let (config, reasons) = ProjectConfig::resolve(fixture.root(), None);
        assert_eq!(reasons, Vec::<String>::new());
        let bound = config.catalog.argument.expect("the file names two");
        assert_eq!(bound.len(), 2);
        assert_eq!(bound[0].class, "Drupal\\App\\EntityHelperTrait");
        assert_eq!(bound[0].method, "_field");
        assert_eq!(bound[0].catalog, "field");
        assert_eq!(
            bound[0].at, 0,
            "the first argument, unless it says otherwise"
        );
        assert_eq!(
            bound[0].suffix, "",
            "nothing appended, unless it says otherwise"
        );
        assert_eq!(bound[1].method, "_value");
        // Naming one table must not disturb the others, as everywhere else here.
        assert_eq!(config.lint.off, None);
        assert_eq!(config.format.style, None);
    }

    /// A misspelt key is a reported error and not a silently dropped line.
    #[test]
    fn a_catalog_entry_with_a_misspelt_key_is_reported() {
        let fixture = MaterializedFixture::parse(
            "//- /phrost.toml
[[catalog.argument]]
clas = \"App\"\nmethod = \"f\"\ncatalog = \"field\"\n",
        );
        let (config, reasons) = ProjectConfig::resolve(fixture.root(), None);
        assert_eq!(reasons.len(), 1, "one source, one reason");
        assert!(
            reasons[0].contains("clas"),
            "the misspelling itself: {}",
            reasons[0]
        );
        assert_eq!(
            config,
            ProjectConfig::default(),
            "a file that does not parse leaves the defaults, as the module doc says"
        );
    }

    #[test]
    fn a_setting_the_file_does_not_mention_is_left_alone() {
        // The whole reason nothing here is a plain value: a file that names only `off` must not also
        // reset the categories to an empty list.
        let fixture =
            MaterializedFixture::parse("//- /phrost.toml\n[lint]\noff = [\"strict-types\"]\n");
        let (config, reasons) = ProjectConfig::resolve(fixture.root(), None);
        assert_eq!(reasons, Vec::<String>::new());
        assert_eq!(
            config.lint.off.as_deref(),
            Some(["strict-types".to_owned()].as_slice())
        );
        assert_eq!(
            config.lint.categories, None,
            "not mentioned, so not changed"
        );
        assert_eq!(config.lint.on, None);
        assert_eq!(config.format.style, None);
    }

    #[test]
    fn a_whole_configuration_round_trips() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /phrost.toml
[lint]
categories = ["correctness", "safety"]
off = ["strict-types", "literal-named-argument"]
on = ["no-eval"]

[format]
style = "drupal"
"#,
        );
        let (config, reasons) = ProjectConfig::resolve(fixture.root(), None);
        assert_eq!(reasons, Vec::<String>::new());
        assert_eq!(
            config.lint.categories.as_deref(),
            Some(["correctness".to_owned(), "safety".to_owned()].as_slice())
        );
        assert_eq!(
            config.lint.on.as_deref(),
            Some(["no-eval".to_owned()].as_slice())
        );
        assert_eq!(config.format.style.as_deref(), Some("drupal"));
    }

    #[test]
    fn a_misspelled_setting_is_reported_and_the_defaults_stand() {
        // Refusing to start would be worse, and silently ignoring it would be worse still: a rule
        // that is off because of a typo looks like a rule that does not work.
        let fixture =
            MaterializedFixture::parse("//- /phrost.toml\n[lint]\ncatagories = [\"safety\"]\n");
        let (config, reasons) = ProjectConfig::resolve(fixture.root(), None);
        assert_eq!(config, ProjectConfig::default());
        assert!(
            reasons.iter().any(|reason| reason.contains("catagories")),
            "the misspelling itself has to be in the message, got {reasons:?}"
        );
    }
}
