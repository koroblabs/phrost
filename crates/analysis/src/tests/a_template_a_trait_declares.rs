//! `@use Holder<Widget>` on a class that composes a templated trait.
//!
//! The identical `@extends Base<Widget>` answered and this did not, which is the divergence that
//! names the cause: one field carried two questions. What `self` means inside a trait's member is
//! the *composing* class -- PHP's rule, and why a trait member has no declaring class of its own --
//! but whose `@template` a name refers to is the **trait**, because that is where the tag is
//! written. Read off one name, a templated trait member answered nothing at all.
//!
//! The tag's prefixed spelling had a second hole of its own: the reader's prefilter looked for
//! `@use` with its `@` where it looks for `extends` without one, so `@template-use` -- which is
//! what Psalm's own documentation writes -- was invisible.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_trait_template_is_supplied_by_the_composing_class() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Widget
{
    public function widgetOnly(): string
    {
        return 'w';
    }
}

/** @template T */
trait Holder
{
    /** @var T */
    private mixed $held;

    /** @return T */
    public function held(): mixed
    {
        return $this->held;
    }
}

/** @use Holder<Widget> */
class WidgetBox
{
    use Holder;
}

function probe(WidgetBox $box): string
{
    return $box->held()->widgetOnly();
}
"#,
        expect![""],
    );
}

/// The prefixed spelling is the same tag, and the prefilter has to see it.
#[test]
fn the_prefixed_spelling_is_read_too() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Widget
{
    public function widgetOnly(): string
    {
        return 'w';
    }
}

/** @template T */
trait Holder
{
    /** @var T */
    private mixed $held;

    /** @return T */
    public function held(): mixed
    {
        return $this->held;
    }
}

/** @template-use Holder<Widget> */
class WidgetBox
{
    use Holder;
}

function probe(WidgetBox $box): string
{
    return $box->held()->widgetOnly();
}
"#,
        expect![""],
    );
}

/// The control, and it has to be read the other way round: with no tag the template resolves to
/// nothing, and a member access on nothing is silence rather than a report -- so a *passing* first
/// test proves only that nothing was reported, which is also true when nothing was typed. What
/// separates the two is a member the supplied class does **not** have: with the tag read, the
/// receiver is a `Widget` and the call is reported; without it, there is no class to report against.
#[test]
fn a_member_the_supplied_class_lacks_is_reported() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Widget
{
    public function widgetOnly(): string
    {
        return 'w';
    }
}

/** @template T */
trait Holder
{
    /** @var T */
    private mixed $held;

    /** @return T */
    public function held(): mixed
    {
        return $this->held;
    }
}

/** @use Holder<Widget> */
class WidgetBox
{
    use Holder;
}

function probe(WidgetBox $box): string
{
    return $box->held()->notOnWidget();
}
"#,
        expect![[r#"
            33:26-33:37 error [undefined-method] Undefined method App\Widget::notOnWidget()
        "#]],
    );
}

/// And with no tag, nothing supplies the template and there is no class to report against.
#[test]
fn a_trait_template_nothing_supplies_answers_nothing() {
    check(
        r#"
//- /src/app.php
<?php
namespace App;

class Widget
{
    public function widgetOnly(): string
    {
        return 'w';
    }
}

/** @template T */
trait Holder
{
    /** @var T */
    private mixed $held;

    /** @return T */
    public function held(): mixed
    {
        return $this->held;
    }
}

class WidgetBox
{
    use Holder;
}

function probe(WidgetBox $box): string
{
    return $box->held()->notOnWidget();
}
"#,
        expect![""],
    );
}
