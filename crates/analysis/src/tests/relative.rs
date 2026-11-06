//! `self`, `static` and `parent` written where a class goes.
//!
//! A keyword rather than a name, which is why nothing found it: it is not a name reference the
//! def-map can resolve — asking for a class called "self" finds nothing — and not a member site
//! either, so both searches a cursor runs went straight past it.

use expect_test::expect;

use crate::tests::{Query, check};

#[test]
fn goto_on_a_relative_keyword_reaches_the_class_it_means() {
    check(
        r#"
//- /src/Base.php
<?php
namespace App;

class Base
{
    public function __construct() {}
}

//- /src/Config.php
<?php
namespace App;

class Config extends Base
{
    public const NAME = 'c';

    public static function make(): static
    {
        return new se@new_self@lf();
    }

    public function copy(): void
    {
        par@parent@ent::__construct();
        $name = se@static_const@lf::NAME;
        $made = stat@static_call@ic::make();
        $is = $this instanceof se@instanceof@lf;
        $other = new stat@new_static@ic();
    }
}
"#,
        &[
            Query::goto("new self", "new_self"),
            Query::goto("parent::__construct", "parent"),
            Query::goto("self::NAME", "static_const"),
            Query::goto("static::make", "static_call"),
            Query::goto("instanceof self", "instanceof"),
            Query::goto("new static", "new_static"),
        ],
        expect![[r#"
            new self
            - class App\Config @ src/Config.php 4:7-4:13 (full 4:1-21:2)
            parent::__construct
            - class App\Base @ src/Base.php 4:7-4:11 (full 4:1-7:2)
            self::NAME
            - class App\Config @ src/Config.php 4:7-4:13 (full 4:1-21:2)
            static::make
            - class App\Config @ src/Config.php 4:7-4:13 (full 4:1-21:2)
            instanceof self
            - class App\Config @ src/Config.php 4:7-4:13 (full 4:1-21:2)
            new static
            - class App\Config @ src/Config.php 4:7-4:13 (full 4:1-21:2)
        "#]],
    );
}

#[test]
fn hover_on_a_relative_keyword_describes_the_class() {
    check(
        r#"
//- /src/Config.php
<?php
namespace App;

/** The configuration. */
final class Config
{
    public static function make(): static
    {
        return new se@here@lf();
    }
}
"#,
        &[Query::hover("self", "here")],
        expect![[r#"
            self
            - code: <?php
            namespace App;

            final class Config
            - ---
            - doc: The configuration.
        "#]],
    );
}
