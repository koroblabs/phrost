//! One file per behaviour, each rendering the whole answer for a template.

mod a_computed_name_is_not_one;
mod names_a_template_writes;
mod tags_the_lexer_sees;
mod what_a_template_reads;

use crate::{BindingKind, Value, lexer::TokenKind, template_names};

/// Every name `text` writes, one per line: the construct, the name, where it is written and whether
/// the template allows it to be missing.
fn render_names(text: &str) -> String {
    template_names(text)
        .iter()
        .map(|found| {
            format!(
                "{:?} {:?} at {:?}{}\n",
                found.kind,
                found.name,
                found.written,
                match found.may_be_missing {
                    true => " (may be missing)",
                    false => "",
                }
            )
        })
        .collect()
}

/// Every token `text` holds, one per line, as its kind and its text.
fn render_tokens(text: &str) -> String {
    crate::lexer::tokens(text)
        .iter()
        .map(|token| {
            let kind = match token.kind {
                TokenKind::String(quoted) => format!(
                    "String{}{}",
                    if quoted.interpolated {
                        " interpolated"
                    } else {
                        ""
                    },
                    if quoted.terminated {
                        ""
                    } else {
                        " unterminated"
                    }
                ),
                other => format!("{other:?}"),
            };
            format!("{kind} {:?}\n", &text[token.range.clone()])
        })
        .collect()
}

/// Every chain and binding `text` holds, in the order they were read, then the macro bodies.
fn render_reading(text: &str) -> String {
    let reading = crate::read(text);
    let chain = |at: usize| {
        let chain = &reading.chains[at];
        let mut out = text[chain.root.clone()].to_owned();
        for step in &chain.steps {
            let name = &text[step.name.clone()];
            out.push_str(&match step.kind {
                crate::StepKind::Attribute => format!(" .{name}"),
                crate::StepKind::Method => format!(" .{name}()"),
                crate::StepKind::Key => format!(
                    " [{}]",
                    if name.parse::<i64>().is_ok() {
                        name.to_owned()
                    } else {
                        format!("'{name}'")
                    }
                ),
                crate::StepKind::Index => " [..]".to_owned(),
            });
        }
        out
    };
    let value = |value: &Value| match value {
        Value::Chain { chain: at, filters } => {
            let mut out = format!("Chain {}", chain(*at));
            for filter in filters {
                out.push_str(&format!(" |{filter}"));
            }
            out
        }
        other => format!("{other:?}"),
    };
    // Bindings and chains interleaved by where they are written, which is the order a reader meets
    // them in.
    let mut lines: Vec<(usize, String)> = Vec::new();
    for (at, found) in reading.chains.iter().enumerate() {
        lines.push((found.root.start, format!("chain {}\n", chain(at))));
    }
    for binding in &reading.bindings {
        let kind = match &binding.kind {
            BindingKind::Element(found) => format!("Element({})", value(found)),
            BindingKind::Key(found) => format!("Key({})", value(found)),
            BindingKind::Assigned(found) => format!("Assigned({})", value(found)),
            other => format!("{other:?}"),
        };
        lines.push((
            binding.range.start,
            format!(
                "binding {} {kind} visible {:?}\n",
                binding.name, binding.visible
            ),
        ));
    }
    lines.sort_by_key(|(at, _)| *at);
    let mut out: String = lines.into_iter().map(|(_, line)| line).collect();
    for body in &reading.isolated {
        out.push_str(&format!("isolated {body:?}\n"));
    }
    out
}
