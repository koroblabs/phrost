//! `$doc->createElement('p')` cannot be `false`.
//!
//! The declaration says `DOMElement|false`, and the `false` is what the method answers for a tag
//! name XML would not accept. When the name is written at the call site there is nothing left to be
//! uncertain about -- and a name read from a variable really can fail, which is what keeps the rule
//! a reading of the literal rather than a blanket exemption.
//!
//! The same shape `literal_returns` states for a free function, and it came up for the same reason:
//! the stub set's DOM tags were being dropped on the floor, and recovering them put 236 reports
//! about a `DOMElement|false` receiver on Symfony -- of which 166 were a `createElement` with a
//! literal tag.

use expect_test::expect;

use super::{Query, check_view};

#[test]
fn a_well_formed_tag_name_takes_the_failure_branch_off() {
    check_view(
        r#"
//- /src/stubs.php
<?php

class DOMNode {}

class DOMElement extends DOMNode
{
    public function setAttribute(string $name, string $value): void {}
}

class DOMDocument extends DOMNode
{
    /** @return DOMElement|false */
    public function createElement(string $localName, string $value = '') {}
}

//- /src/Writer.php
<?php
namespace App;

class Writer
{
    public function go(\DOMDocument $doc, string $tag): void
    {
        $l@literal@iteral = $doc->createElement('p');
        $q@qualified@ualified = $doc->createElement('xsl:template');
        $v@variable@ariable = $doc->createElement($tag);
        $m@malformed@alformed = $doc->createElement('not a name');
    }
}
"#,
        &[
            Query::variable_type("a literal XML name", "literal"),
            Query::variable_type("a qualified one", "qualified"),
            Query::variable_type("a name nothing here knows", "variable"),
            Query::variable_type("a literal XML would refuse", "malformed"),
        ],
        expect![[r#"
            a literal XML name
            - DOMElement
            a qualified one
            - DOMElement
            a name nothing here knows
            - DOMElement|false
            a literal XML would refuse
            - DOMElement|false
        "#]],
    );
}
