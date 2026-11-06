//! A `@method` tag whose return type is settled at the call.
//!
//! A real method spreads the claim over `@template`, `@param` and `@return`; a `@method` tag is the
//! whole declaration in one line, and the two claims a call can settle are read off its own parts.
//! Laravel writes both on every facade, which is what the corpus fixtures are modelled on: a finder
//! whose result depends on whether it was given an id or a list of them, and a container whose
//! factory hands back the class it was named.

use expect_test::expect;

use super::{Query, check_view};

const CLASSES: &str = r#"
//- /src/classes.php
<?php
class User
{
    public function getName(): string { return ''; }
}
class UserCollection
{
    public function first(): User { return new User(); }
}
class MyService
{
    public function doSomething(): void {}
}
"#;

#[test]
fn a_conditional_return_on_a_tag_is_settled_by_the_argument() {
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
/**
 * @method static ($id is int ? User : UserCollection) find($id)
 */
class UserFacade
{{
}}

function run(): void
{{
    $one = UserFacade::find(1);
    $on@one@e;
    $many = UserFacade::find('a,b');
    $ma@many@ny;
}}
"#
        ),
        &[
            Query::variable_class("given an int", "one", "one"),
            Query::variable_class("given a string", "many", "many"),
        ],
        expect![[r#"
            given an int
            - User
            given a string
            - UserCollection
        "#]],
    );
}

#[test]
fn a_template_bound_through_a_class_string_parameter_is_the_class_the_call_names() {
    // On an instance and on a static call, and through a union: `class-string<T>|string` is how a
    // container's factory takes a service id and a class name in one parameter.
    check_view(
        &format!(
            r#"{CLASSES}
//- /src/app.php
<?php
/**
 * @method T make<T of object>(class-string<T> $abstract)
 */
class Container
{{
}}

/**
 * @method static T make<T of object>(class-string<T>|string $abstract, array $parameters = [])
 */
class App
{{
}}

function run(Container $c): void
{{
    $x = $c->make(MyService::class);
    $@instance@x;
    $y = App::make(MyService::class);
    $@static@y;
}}
"#
        ),
        &[
            Query::variable_class("through an instance", "instance", "x"),
            Query::variable_class("through a static call", "static", "y"),
        ],
        expect![[r#"
            through an instance
            - MyService
            through a static call
            - MyService
        "#]],
    );
}
