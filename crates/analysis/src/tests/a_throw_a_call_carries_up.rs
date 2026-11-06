//! `doc-missing-throws`: what a body throws by *calling* something that declares it throws.
//!
//! The rule read only `throw new` written in the body being checked, for a stated reason -- "a
//! callee's own throws are its docblock's business, and reading them here would document the whole
//! call tree". That reason answers a different question than the one a person asks: `new
//! DateTime($input)` throws, they wrote no handler and no tag, and the check said nothing.
//!
//! What a callee **declares**, never what its body does -- across files. A `@throws` is the promise
//! a caller may rely on, and the callee's body is in a file this view cannot see at all -- the
//! index keeps declarations, not statements. So across files the chain is one level deep by
//! construction: a method that documents nothing propagates nothing, which is how PhpStorm and
//! PHPStan read the tag too. A callee in the *same* file is read for what it throws: its body is
//! here, and reading only its tag made fixing the file a series of saves rather than one.
//!
//! Both halves read the same body, which is the point of testing them together: closures are out
//! of it, a surrounding `catch` covers a call as it covers a throw, and one class reported once.

use expect_test::expect;

use super::diagnostics::check_asked_for;

#[test]
fn a_call_that_declares_what_it_throws_is_reported_in_the_caller_that_hides_it() {
    check_asked_for(
        r#"
//- /spl.php
<?php
class Exception {}
class RuntimeException extends Exception {}
class ContractFailure extends Exception {}

//- /lib.php
<?php
namespace App;

class Boom
{
    /**
     * @throws \RuntimeException
     */
    public function go(): void {}

    /**
     * Says nothing, so nothing propagates from it.
     */
    public function quiet(): void {}
}

interface Contract
{
    /**
     * @throws \ContractFailure
     */
    public function run(): void;
}

/**
 * @throws \RuntimeException
 */
function detonate(): void {}

//- /app.php
<?php
namespace App;

class Repo
{
    /**
     * Calls one, documents nothing.
     */
    public function bare(Boom $boom): void
    {
        $boom->go();
    }

    /**
     * Documents it, so there is nothing to report.
     *
     * @throws \RuntimeException
     */
    public function documented(Boom $boom): void
    {
        $boom->go();
    }

    /**
     * Summarised with a base class, which covers it the way it covers a written throw.
     *
     * @throws \Exception
     */
    public function summarised(Boom $boom): void
    {
        $boom->go();
    }

    /**
     * Handled where it is called, so nothing leaves.
     */
    public function handled(Boom $boom): void
    {
        try {
            $boom->go();
        }
        catch (\RuntimeException $e) {
        }
    }

    /**
     * Four calls, one class, one finding -- on the first.
     */
    public function repeatedly(Boom $boom): void
    {
        $boom->go();
        $boom->go();
        $boom->go();
        $boom->go();
    }

    /**
     * A callee that documents nothing says nothing about what it throws.
     */
    public function silent(Boom $boom): void
    {
        $boom->quiet();
    }

    /**
     * A free function's tag carries the same way a method's does.
     */
    public function viaFunction(): void
    {
        detonate();
    }

    /**
     * Through the interface the receiver is typed as, which is where the tag is written.
     */
    public function viaContract(Contract $contract): void
    {
        $contract->run();
    }

    /**
     * Inside a closure, which is the closure's business -- the same rule a written throw follows.
     */
    public function inAClosure(Boom $boom): void
    {
        $run = function () use ($boom): void {
            $boom->go();
        };
        $run();
    }
}
"#,
        &["doc-missing-throws"],
        expect![[r#"
            11:16-11:18 warning [doc-missing-throws] throws \RuntimeException, which App\Boom::go() declares and the docblock does not
            51:16-51:18 warning [doc-missing-throws] throws \RuntimeException, which App\Boom::go() declares and the docblock does not
            70:9-70:17 warning [doc-missing-throws] throws \RuntimeException, which App\detonate() declares and the docblock does not
            78:20-78:23 warning [doc-missing-throws] throws \ContractFailure, which App\Contract::run() declares and the docblock does not
        "#]],
    );
}

#[test]
fn a_catch_of_throwable_covers_a_class_whose_ancestry_stops_short() {
    // Nothing but a `Throwable` can be thrown, so a `catch (\Throwable)` catches every class --
    // including one whose ancestry the index cannot walk up to it. DocuSign's client declares a
    // dev dependency's `InvalidArgumentException`, and the call inside the reference project's
    // `catch (\Throwable $exception)` was reported for it. The stub here gives `Exception` no
    // `Throwable`, which is the ancestry stopping short.
    check_asked_for(
        r#"
//- /spl.php
<?php
interface Throwable {}
class Exception {}

//- /lib.php
<?php
namespace Vendor\Missing;

class SomethingFailed extends \Exception {}

//- /client.php
<?php
namespace App;

class Client
{
    /**
     * @throws \Vendor\Missing\SomethingFailed
     */
    public function request(): void {}
}

//- /app.php
<?php
namespace App;

class Caller
{
    /**
     * Covered by the catch.
     */
    public function covered(Client $client): void
    {
        try {
            $client->request();
        }
        catch (\Throwable $e) {
        }
    }

    /**
     * Not covered.
     */
    public function uncovered(Client $client): void
    {
        $client->request();
    }
}
"#,
        &["doc-missing-throws"],
        expect![[r#"
            23:18-23:25 warning [doc-missing-throws] throws \Vendor\Missing\SomethingFailed, which App\Client::request() declares and the docblock does not
        "#]],
    );
}

/// A callee in the same file that throws and has not said so yet carries the throw up as well, so
/// the file is fixed by one save. The report says the callee *throws* it, since it does not declare
/// it -- and a callee in another file that documents nothing still propagates nothing.
#[test]
fn a_callee_in_the_same_file_carries_up_what_it_throws_undocumented() {
    check_asked_for(
        r#"
//- /spl.php
<?php
class Exception {}
class RuntimeException extends Exception {}

//- /lib.php
<?php
namespace App;

class Elsewhere
{
    public function quiet(): void
    {
        throw new \RuntimeException();
    }
}

//- /app.php
<?php
namespace App;

class Repo
{
    /**
     * Finds one.
     */
    public function find(Elsewhere $other): void
    {
        $this->load();
        $other->quiet();
    }

    /**
     * Loads one.
     */
    public function load(): void
    {
        throw new \RuntimeException();
    }
}
"#,
        &["doc-missing-throws"],
        expect![[r#"
            11:16-11:20 warning [doc-missing-throws] throws \RuntimeException, which App\Repo::load() throws and the docblock does not declare
            20:9-20:38 warning [doc-missing-throws] throws \RuntimeException, which the docblock does not declare
        "#]],
    );
}
