//! The theme hook a render array names, read off the template files that implement it.
//!
//! `'#theme' => 'item_list'` is how a render array says what draws it, and a typo there renders
//! *nothing*: no exception, no log line, an empty region on the page. It is the commonest half-day of
//! Drupal debugging there is, and 67 of them are written in one real project's own modules.
//!
//! # Read from the filenames, and why that is enough
//!
//! A theme hook is registered by `hook_theme()`, whose keys are entries of an array returned from a
//! function body -- not something a per-file claim can withdraw. But the convention that goes with it
//! is that each hook has a template named after it with hyphens for underscores, in the extension's
//! `templates/` directory, and the filenames *are* readable: 1 263 of them in that checkout under 458
//! distinct names, in 20 ms.
//!
//! Partial by construction, and that is the whole reason this catalog is **open**. `hook_theme()` can
//! register a name whose template lives elsewhere -- that project puts 21 of them under
//! `src/Generated/.../TemplateFiles/twig/` -- or with no template at all, and a name this cannot see
//! is a name it must say nothing about.
//!
//! # The suggestion form, which is why it can never be closed
//!
//! `#theme` also takes *suggestions*: `table__file_formatter_table` asks for the `table` hook drawn by
//! a more specific template if one exists. No catalog will ever hold those -- they are composed at the
//! call -- and **31 of that project's 67 sites are one**. So the only thing this can say is that a name
//! is a plausible misspelling of one it holds, which the near-miss reader already does for every open
//! catalog.
//!
//! Measured before it was built, because a wrong suggestion is worse than silence: of those 31
//! uncovered values, **none** is within one edit of any of the 458 registered names. The suggestion
//! form is far from every real hook by construction -- it is a real hook plus `__` plus more -- which
//! is what makes the length gate do the work here.

use super::templates;
use crate::rule::{ArrayKeyBinding, PlaceEntry, RenderArray, TemplateFileName};

/// A name this provider chose. Nothing below the seam knows what it means.
pub(super) const THEMES: &str = "theme";

/// The one array key that names a theme hook.
pub(super) static ARRAY_KEYS: &[ArrayKeyBinding] = &[ArrayKeyBinding {
    key: "#theme",
    catalog: THEMES,
}];

/// A render array naming a theme hook, and handing its template the values under `#` keys.
///
/// The keys listed are the array's own -- how it is cached, wrapped, weighted and attached -- and are
/// never one of the hook's variables, so listing them as variables would offer `cache` in a template
/// that has none.
pub(super) static RENDER_ARRAYS: &[RenderArray] = &[RenderArray {
    key: "#theme",
    catalog: THEMES,
    prefix: "#",
    own: &[
        "#theme",
        "#attached",
        "#cache",
        "#prefix",
        "#suffix",
        "#weight",
        "#access",
        "#access_callback",
        "#type",
        "#markup",
        "#plain_text",
        "#allowed_tags",
        "#theme_wrappers",
        "#pre_render",
        "#post_render",
        "#printed",
        "#lazy_builder",
        "#create_placeholder",
        "#children",
        "#sorted",
        "#states",
        "#render_children",
        "#defaults_loaded",
    ],
    file_name: Some(TemplateFileName {
        suffix: SUFFIX,
        written: '-',
        named: '_',
    }),
}];

/// What a template file is called.
const SUFFIX: &str = ".html.twig";

/// Every theme hook a template file under an extension's `templates/` directory implements.
///
/// The same per-extension shape the route and permission reader uses, and the same directories: core
/// declares most of the hooks a project writes, and `item_list`, `table` and `container` are in every
/// other render array. The files are found once, by [`templates::template_files`], for this and for
/// the names Twig loads them by.
///
/// A name several files implement -- a theme overriding a module's template is Drupal's own mechanism
/// -- is one key, and which file goto opens is whichever was read last. The key set is the same either
/// way, which is what this is for.
pub(super) fn catalog_places(
    extensions: &[templates::Extension],
    files: &[templates::TemplateFile],
) -> Vec<PlaceEntry> {
    files
        .iter()
        .filter_map(|file| {
            let name = file.path.file_name()?.to_str()?;
            let stem = name.strip_suffix(SUFFIX)?;
            Some(PlaceEntry {
                catalog: THEMES,
                // Drupal's own rule: the hook is the filename with hyphens turned into underscores.
                key: stem.replace('-', "_"),
                path: file.path.clone(),
                // The file *is* the declaration, so there is no line inside it to point at -- and
                // landing at the top of the template is what a reader following `#theme` wants.
                line: 0,
                character: 0,
                // Which extension provides the template, which is what a reader wants beside the
                // name: the filename is already what goto opens, so repeating it would say nothing
                // twice.
                detail: extensions[file.extension].owner().to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use phrost_testkit::MaterializedFixture;

    use super::THEMES;
    use crate::{FrameworkProvider as _, providers::drupal::Drupal};

    /// Every theme hook a template file names, and which extension provides it.
    ///
    /// The whole set, because an extra key matters as much as a missing one: this catalog is what a
    /// misspelling is measured against, and a name nobody registered would be something to suggest.
    #[test]
    fn every_template_under_an_extension_names_the_hook_it_draws() {
        let fixture = MaterializedFixture::parse(
            r#"
//- /docroot/core/lib/Drupal.php
<?php class Drupal {}

//- /docroot/core/modules/system/system.info.yml
name: System

//- /docroot/core/modules/system/templates/table.html.twig
<table></table>

//- /docroot/core/modules/system/templates/item-list.html.twig
<ul></ul>

//- /docroot/modules/custom/mine/mine.info.yml
name: Mine

//- /docroot/modules/custom/mine/templates/mail/item-summary.html.twig
<p></p>

//- /docroot/modules/custom/mine/templates/notes.md
not a template

//- /docroot/themes/custom/skin/skin.info.yml
name: Skin

//- /docroot/themes/custom/skin/templates/table.html.twig
<table class="skinned"></table>
"#,
        );

        let mut rendered = Drupal
            .project(fixture.root())
            .catalog_places()
            .into_iter()
            .filter(|place| place.catalog == THEMES)
            .map(|place| format!("{} -- {}\n", place.key, place.detail))
            .collect::<Vec<_>>();
        rendered.sort();

        expect![[r#"
            item_list -- system
            item_summary -- mine
            table -- skin
            table -- system
        "#]]
        .assert_eq(&rendered.concat());
    }
}
