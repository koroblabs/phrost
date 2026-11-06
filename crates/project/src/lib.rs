//! Composer autoload configuration and project layout.
//!
//! This crate answers one question: given a fully-qualified PHP name, which file *could* declare
//! it? That answer comes entirely from composer's own configuration — `composer.json` and the
//! generated `vendor/composer/autoload_*.php` files — never from parsing PHP. A name here is a
//! string with a namespace-separator convention, nothing more; this crate has no idea what a
//! class, an interface, or a `use` statement is, and it must stay that way. Teaching it PHP
//! semantics would duplicate work `phrost-lower` and `phrost-defmap` already own, and the two
//! copies would drift apart the first time either one changed.
//!
//! Two configuration sources feed one [`AutoloadMap`]:
//! - [`ComposerJson`] reads the root `composer.json`'s own `autoload` / `autoload-dev` sections.
//! - [`load_generated_autoload`] reads composer's generated files under `vendor/composer/`, the
//!   merged result across every installed package and the authoritative source for vendor code.
//!
//! [`ProjectConfig`] is the one thing here that is not composer's: the settings a project makes for
//! itself, in a `phrost.toml` beside `composer.json`. It lives here because it is project
//! configuration and this crate is where a project's configuration is read.
//!
//! [`ProjectLayout`] carries just enough to tell user code from vendor code apart, and
//! [`discover_php_files`] walks it. Neither detects a docroot or a framework — those belong to the
//! framework layer, which contributes extra roots and file extensions through its own seam.

mod autoload;
mod composer_json;
mod config;
mod discovery;
mod generated;
mod layout;
pub mod paths;
mod version_constraint;

pub use self::{
    autoload::{AutoloadMap, AutoloadStats},
    composer_json::{ComposerJson, ComposerJsonError},
    config::{
        CONFIG_FILE, CatalogArgument, CatalogConfig, FormatConfig, LintConfig, ProjectConfig,
    },
    discovery::{
        DiscoveredFile, PHP_EXTENSIONS, TEMPLATE_EXTENSIONS, discover_php_files, discover_templates,
    },
    generated::load_generated_autoload,
    layout::ProjectLayout,
    version_constraint::VersionConstraint,
};
