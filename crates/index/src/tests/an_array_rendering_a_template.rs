//! An array naming the template it renders under a framework's key is a render site, with the keys
//! that hand variables over -- the array's own keys left out, and an array naming no template, or
//! naming it with something other than a literal, not one at all.

use expect_test::expect;

use super::check_index;

#[test]
fn an_array_naming_its_template_hands_over_its_prefixed_keys() {
    check_index(
        r#"
//- /src/Report.php
<?php
namespace App;

class Report
{
    public function build(string $hook): array
    {
        $items = [
            'nested' => ['#template' => 'item_list', '#items' => [1, 2], '#cache' => []],
        ];
        $dynamic = ['#template' => $hook, '#items' => []];
        $plain = ['#items' => []];
        return ['#template' => 'report__pdf', '#data' => ['name' => 'x'], '#title' => 'T', 'skipped' => 1];
    }
}
"#,
        expect![[r#"
            state ready
            file src/Report.php [User]
              renders item_list at 133 with #items in App\Report
              renders report__pdf at 319 with #data, #title in App\Report
              class App\Report @ 4:7-4:13
              App\Report is class
              App\Report::method build(string $hook): array  public  @ 6:21-6:26 (full 6:5-14:6)
            catalog
              [class] app\report -> src/Report.php (App\Report)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}

/// The same array built one key at a time: the writes on that local in the same body, and none on
/// another local or in a closure of its own.
#[test]
fn an_array_built_by_writes_hands_over_the_keys_written_beside_it() {
    check_index(
        r#"
//- /src/Notice.php
<?php
namespace App;

class Notice
{
    public function build(object $application): array
    {
        $build['#template'] = 'notice';
        $build['#date'] = $application->date();
        if ($application->shared) {
            $build['#shared'] = true;
        }
        $build['#cache'] = [];
        $other['#unrelated'] = 1;
        $later = function () use ($build) {
            $build['#inside'] = 2;
        };
        return $build;
    }
}
"#,
        expect![[r#"
            state ready
            file src/Notice.php [User]
              renders notice at 105 with #date, #shared (writes) in App\Notice
              class App\Notice @ 4:7-4:13
              App\Notice is class
              App\Notice::method build(object $application): array  public  @ 6:21-6:26 (full 6:5-19:6)
            catalog
              [class] app\notice -> src/Notice.php (App\Notice)
            namespaces
              app -> App x1
            declared keys
            dispatched keys
        "#]],
    );
}
