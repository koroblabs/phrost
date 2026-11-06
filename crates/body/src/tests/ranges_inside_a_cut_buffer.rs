//! Every expression range lowering hands out lies inside the text it was lowered from.
//!
//! A buffer mid-edit is cut anywhere, and the parser recovers by supplying what is missing -- a
//! closing parenthesis, a semicolon -- at positions past the end. A range reaching there is a
//! slice a reader cannot take: inlay hints sliced the value of the last assignment of a truncated
//! WordPress file and the request failed with "end byte index 4688 is out of bounds".

use phrost_model::{FileId, Interner, TextSize};

const SOURCE: &str = r#"<?php
function wp_apply_dimensions_support( $block_type, $block_attributes ) {
	$dimensions_block_styles = array();
	if ( isset( $block_attributes['minHeight'] ) ) {
		$dimensions_block_styles['aspectRatio'] = 'unset';
	}
	$styles = wp_style_engine_get_styles( array( 'dimensions' => $dimensions_block_styles ) );
	return $styles['css'] ?? fn ( $x ) => [ $x, $block_type->name ];
}
"#;

#[test]
fn no_range_reaches_past_the_end_of_a_cut_buffer() {
    let mut past = Vec::new();
    for cut in (1..SOURCE.len()).filter(|&cut| SOURCE.is_char_boundary(cut)) {
        let text = &SOURCE[..cut];
        let end = TextSize::try_from(text.len()).expect("fixture fits in u32");
        let mut interner = Interner::new();
        let lowered = phrost_lower::lower_text_with_bodies(
            &mut interner,
            FileId::from_raw(0),
            text,
            &phrost_lower::KeyedAttributes::default(),
        );
        let hir = &lowered.hir;
        for (_, body) in hir.bodies() {
            crate::walk::visit_statements(hir, body.statements, &mut |hir, expr| {
                let range = hir.expr(expr).range();
                if range.end() > end {
                    past.push(format!(
                        "cut at {cut}: {range:?} past {end:?} {:.60}",
                        format!("{:?}", hir.expr(expr))
                    ));
                }
            });
        }
    }
    assert!(past.is_empty(), "{}", past[..past.len().min(8)].join("\n"));
}
