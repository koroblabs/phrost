//! The checks inside a trait read `$this` as the class the trait is composed into, which is the
//! code that runs: its private members are reachable, and a member no host declares is missing.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_hosts_private_members_are_its_own_and_a_missing_one_is_reported() {
    check(
        r#"
//- /src/Counts.php
<?php
namespace App;

class Base
{
    protected int $count = 0;
}

class Host extends Base
{
    use Counts;

    private string $secret = '';
}

trait Counts
{
    private int $own = 0;

    public function bump(): int
    {
        $this->nowhere();
        return $this->count + $this->own + ($this->secret === '' ? 0 : 1);
    }
}
"#,
        expect![[r#"
            22:16-22:23 error [undefined-method] Undefined method App\Host::nowhere()
        "#]],
    );
}

#[test]
fn a_call_to_the_traits_own_method_is_checked_with_several_hosts() {
    // Each host composes the method from the trait, so the call has one declaration whichever host
    // runs it, and passing a null where it takes a string is reported as it would be in a class.
    check(
        r#"
//- /src/Filters.php
<?php
namespace App;

class Rows
{
    use Filters;
}

class LazyRows
{
    use Filters;
}

trait Filters
{
    public function whereStrict(string $key): static { return $this; }

    public function whereNull(?string $key = null): static
    {
        return $this->whereStrict($key);
    }
}
"#,
        expect![[r#"
            20:35-20:39 warning [possibly-null-argument] $key takes string, and this may be null
        "#]],
    );
}
