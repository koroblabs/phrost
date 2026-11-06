//! A record handed where a shape is declared, missing a key the shape requires.
//!
//! Silent while a record held only what its literal wrote, because the commonest way to build one
//! is a key at a time and refusing it for a key added later reported correct code. The writes are
//! folded in since, so a key the record lacks is one nothing put there: Symfony's own test helper
//! declares `scrollOffset` for an array the renderer and every caller spell `scroll_offset`.
//!
//! The last file holds every case, because the harness reports that file: a literal missing a
//! required key, one missing only an optional key, one completed a key at a time, and one whose
//! missing key is written in a branch -- which counts as there, keeping the doubt on the quiet side.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_required_key_nothing_wrote_is_reported() {
    check(
        r#"
//- /src/Renderer.php
<?php

class Renderer
{
    /**
     * @param array{scrollOffset: int, lines: int, title?: string} $viewport
     */
    public function render(array $viewport): void
    {
    }
}

//- /src/app.php
<?php

function run(Renderer $renderer, bool $flag): void
{
    $renderer->render(['scroll_offset' => 0, 'lines' => 2]);
    $renderer->render(['scrollOffset' => 0, 'lines' => 2]);

    $built = ['scrollOffset' => 0];
    $built['lines'] = 2;
    $renderer->render($built);

    $maybe = ['scrollOffset' => 0];
    if ($flag) {
        $maybe['lines'] = 2;
    }
    $renderer->render($maybe);
}
"#,
        expect![[r#"
            5:23-5:59 error [incompatible-argument] $viewport takes array{scrollOffset: int, lines: int, title?: string}, and this is array{scroll_offset: int, lines: int}
        "#]],
    );
}
