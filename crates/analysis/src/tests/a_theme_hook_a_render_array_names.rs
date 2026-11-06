//! The theme hook a render array names, and the one thing that can be said about a wrong one.
//!
//! `'#theme' => 'item_list'` says what draws a render array, and a typo there renders *nothing*: no
//! exception, no log line, an empty region on the page. 67 of them are written in one real project's
//! own modules, and it is the commonest half-day of Drupal debugging there is.
//!
//! # Why "unknown" is impossible here, not merely unwise
//!
//! `#theme` also takes **suggestions**: `table__file_formatter_table` asks for the `table` hook drawn
//! by a more specific template if one exists. They are composed at the call, so no catalog will ever
//! hold them -- and **31 of that project's 67 sites are one**. A catalog that reported an absence
//! would light up half the render arrays in the codebase.
//!
//! So the only claim is the near-miss one, and what makes it safe is measured rather than argued: of
//! those 31 uncovered values, none is within one edit of any of the 458 names the catalog holds. A
//! suggestion is a real hook plus `__` plus more, which puts it far from every registered name by
//! construction.

use expect_test::expect;

use super::diagnostics::check_framework;

const FIXTURE: &str = r#"
//- /src/Render.php
<?php
namespace App;

class Render
{
    public function build(): array
    {
        return [
            'rows' => ['#theme' => 'admin_block_content'],
            'typo' => ['#theme' => 'admin_block_contnt'],
            'suggestion' => ['#theme' => 'admin_block_content__item_summary'],
            'unheard_of' => ['#theme' => 'zzz_no_such_theme_anywhere'],
            'computed' => ['#theme' => self::CHOSEN],
        ];
    }

    private const CHOSEN = 'admin_block_content';
}
"#;

fn registered(world: &mut phrost_harness::World) {
    world.contracts_mut().bind_array_key("#theme", "theme");
    world
        .contracts_mut()
        .catalog_entry("theme", "admin_block_content", None);
    world.contracts_mut().catalog_entry("theme", "table", None);
}

#[test]
fn a_misspelling_reports_and_a_suggestion_and_an_unheard_of_name_do_not() {
    // The last three lines are the whole reason this catalog stays open. A suggestion is correct code
    // no catalog can hold; a name nothing registers may be a `hook_theme()` entry whose template
    // lives somewhere this cannot see -- that project puts 21 of them under
    // `src/Generated/.../TemplateFiles/twig/`; and a computed value is not a name written wrong.
    check_framework(
        FIXTURE,
        registered,
        expect![[r#"
        10:36-10:56 warning [unknown-catalog-key] No theme is registered as `admin_block_contnt`. Did you mean `admin_block_content`?
    "#]],
    );
}
