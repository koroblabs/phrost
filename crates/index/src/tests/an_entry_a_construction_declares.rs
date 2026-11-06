//! What an object a named method builds declares: the key its first argument writes and what its
//! second runs -- read from a dependency's body, which is otherwise not lowered, as from the
//! project's own.

use expect_test::expect;

use super::check_index;

#[test]
fn a_construction_in_a_named_method_declares_its_key_and_what_runs_it() {
    check_index(
        r#"
//- /vendor/twig/twig/src/Extension/CoreExtension.php
<?php
namespace Twig\Extension;

use Twig\TwigFilter;
use Twig\TwigFunction;

class CoreExtension
{
    public function getFilters(): array
    {
        return [
            new TwigFilter('upper', [self::class, 'upper']),
            new \Twig\TwigFilter('raw', null),
            new TwigFilter('t', 't'),
            new TwigFilter('fmt', 'Foo\\Bar::fmt'),
            new TwigFunction('not_a_filter', 'f'),
        ];
    }

    public function notAskedAbout(): array
    {
        return [new TwigFilter('ignored', 'f')];
    }
}

//- /src/AppExtension.php
<?php
namespace App;

use Twig\TwigFilter;
use App\Format\Other;

class AppExtension
{
    public function getFilters(): array
    {
        $filters = [new TwigFilter('mine', [$this, 'mine'])];
        $filters[] = new TwigFilter('theirs', [Other::class, 'theirs']);
        return array_merge($filters, [new TwigFilter('closed', fn ($x) => $x)]);
    }
}
"#,
        expect![[r#"
            state ready
            file src/AppExtension.php [User]
              App\AppExtension declares filter `mine` @ 169..173 running App\AppExtension::mine()
              App\AppExtension declares filter `theirs` @ 232..238 running App\Format\Other::theirs()
              App\AppExtension declares filter `closed` @ 322..328 running (unnamed)
              class App\AppExtension @ 7:7-7:19
              use Twig\TwigFilter as TwigFilter  [class]
              use App\Format\Other as Other  [class]
              App\AppExtension is class
              App\AppExtension::method getFilters(): array  public  @ 9:21-9:31 (full 9:5-14:6)
            file vendor/twig/twig/src/Extension/CoreExtension.php [Vendor]
              Twig\Extension\CoreExtension declares filter `upper` @ 191..196 running Twig\Extension\CoreExtension::upper()
              Twig\Extension\CoreExtension declares filter `raw` @ 258..261 running (unnamed)
              Twig\Extension\CoreExtension declares filter `t` @ 299..300 running t()
              Twig\Extension\CoreExtension declares filter `fmt` @ 337..340 running Foo\Bar::fmt()
              class Twig\Extension\CoreExtension @ 7:7-7:20
              Twig\Extension\CoreExtension is class
              Twig\Extension\CoreExtension::method getFilters(): array  public  @ 9:21-9:31 (full 9:5-18:6)
              Twig\Extension\CoreExtension::method notAskedAbout(): array  public  @ 20:21-20:34 (full 20:5-23:6)
            catalog
              [class] app\appextension -> src/AppExtension.php (App\AppExtension)
              [class] twig\extension\coreextension -> vendor/twig/twig/src/Extension/CoreExtension.php (Twig\Extension\CoreExtension)
            namespaces
              app -> App x1
              twig -> Twig x1
              twig\extension -> Twig\Extension x1
            declared keys
            dispatched keys
        "#]],
    );
}
