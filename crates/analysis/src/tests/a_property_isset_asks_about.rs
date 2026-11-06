//! A property `isset()`, `empty()` or `??` asks about is the existence check, not a use.
//!
//! PHP answers `false` for an undeclared property there, and for a private one read from outside,
//! without a warning -- run against 8.5.10 -- so a report on it lands on the line that proves the
//! author knew. A plain read and a call inside the same `isset()` are still reported: the read warns
//! and the call is evaluated.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn the_access_a_presence_test_asks_about_is_not_reported() {
    check(
        r#"
//- /src/Job.php
<?php
namespace App;

class Job
{
    private int $secret = 1;

    public function attempts(): int { return 1; }
}

function retry(Job $job): mixed
{
    if (isset($job->retryUntil) || !empty($job->delay)) {
        return $job->retryUntil ?? $job->secret ?? isset($job->secret);
    }
    if (isset($job->missing()->value)) {
        return 1;
    }
    return $job->retryUntil;
}
"#,
        expect![[r#"
            16:21-16:28 error [undefined-method] Undefined method App\Job::missing()
            19:18-19:28 error [undefined-property] Undefined property App\Job::$retryUntil
        "#]],
    );
}
