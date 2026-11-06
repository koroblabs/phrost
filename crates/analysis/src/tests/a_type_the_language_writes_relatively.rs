//! `self` and `parent` in a card, which name a class rather than being one.
//!
//! A class name in a type is the author's spelling and is shown as written. A relative keyword is
//! not: it is the language's way of saying "the class this is in", and a reader asking what a value
//! *is* learns nothing from being told `self`. The stored half of the type table already resolved
//! them; the written half did not, so one card could say `self` where go-to-type-definition landed
//! on `Profile` -- Symfony declares `Profile::addChild(self $child)` and `CurlResponse` takes
//! `self $response`, which is where the divergence was found.

use expect_test::expect;

use super::{Query, check};

const FIXTURE: &str = r#"
//- /src/app.php
<?php

class Node
{
    public function addChild(self $chi@child@ld): void
    {
        $ch@used@ild;
    }
}

class Leaf extends Node
{
    public function detach(parent $ow@owner@ner): void
    {
    }

    public function orphan(?self $ma@nullable@ybe, self|int $eit@union@her): void
    {
    }
}

function loose(self $no@loose@where): void
{
}
"#;

#[test]
fn a_relative_keyword_in_a_parameter_names_the_class_it_means() {
    check(
        FIXTURE,
        &[
            Query::hover("self at its declaration", "child"),
            Query::hover("self at a use", "used"),
            Query::hover("parent", "owner"),
            Query::hover("?self keeps its marker", "nullable"),
            Query::hover("self inside a union", "union"),
            Query::hover("self with no class around it", "loose"),
        ],
        expect![[r#"
            self at its declaration
            - code: Node $child
            self at a use
            - code: Node $child
            - note: parameter
            parent
            - code: Node $owner
            ?self keeps its marker
            - code: ?Leaf $maybe
            self inside a union
            - code: Leaf|int $either
            self with no class around it
            - code: self $nowhere
        "#]],
    );
}
