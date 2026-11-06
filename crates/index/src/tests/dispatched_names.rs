//! Extension points a file's own calls declare.
//!
//! The one catalog filled from a body rather than from a declaration, which puts three things under
//! test that no other catalog has: only a body-lowered file can contribute, only a receiver the file
//! itself can name is recorded, and what a re-ingest withdraws has to be exactly what it claimed.
//!
//! The rules the shared `build` installs are neutral -- `A\Hub::publish($name)` and
//! `A\Hub::announce($to, $names)` -- because what a dispatched name *means* is a framework's
//! business and nothing at this layer knows it. Between them they cover both shapes a rule can
//! vary: which argument holds the name, whether it may be a list, and what the declaration it
//! implies is called around what the call wrote.

use expect_test::expect;

use super::{Step, check_incremental, check_index};

/// A project whose own service dispatches by name, written the two ways a receiver can be named.
const DISPATCHING: &str = r#"
//- /src/Hub.php
<?php
namespace A;
interface Hub {
    public function publish(string $name): void;
    public function announce(string $to, array $names): void;
}

//- /src/Registry.php
<?php
namespace A;
class Registry {
    public static function hub(): Hub {}
}

//- /src/Reports.php
<?php
namespace A;
class Reports {
    public function __construct(private Hub $hub) {}

    public function run(): void {
        $this->hub->publish('reports_requested');
        Registry::hub()->announce('everyone', ['cache_cleared', 'index_rebuilt']);
    }
}
"#;

#[test]
fn a_call_declares_the_extension_point_it_dispatches() {
    // The whole claim: two calls, three names, and each spelled the way a declaration of it would
    // be -- the convention's prefix, what the call wrote, and the method's own suffix. `publish`
    // adds nothing after the name and `announce` adds `_done`, which is the difference the seam
    // carries because one real framework's two dispatching methods disagree about it.
    check_index(
        DISPATCHING,
        expect![[r#"
            state ready
            file src/Hub.php [User]
              interface A\Hub @ 3:11-3:14
              A\Hub is interface
              A\Hub::method publish(string $name): void  public  @ 4:21-4:28 (full 4:5-4:49)
              A\Hub::method announce(string $to, array $names): void  public  @ 5:21-5:29 (full 5:5-5:62)
            file src/Registry.php [User]
              class A\Registry @ 3:7-3:15
              A\Registry is class
              A\Registry::method hub(): Hub  public static  @ 4:28-4:31 (full 4:5-4:41)
            file src/Reports.php [User]
              class A\Reports @ 3:7-3:14
              dispatches [event] on_reports_requested by A\Hub::publish on A\Reports::hub @ 7:29-7:48
              dispatches [event] on_cache_cleared_done by A\Hub::announce on A\Registry::hub @ 8:48-8:63
              dispatches [event] on_index_rebuilt_done by A\Hub::announce on A\Registry::hub @ 8:65-8:80
              A\Reports is class
              A\Reports::method __construct(private Hub $hub)  public  @ 4:21-4:32 (full 4:5-4:53)
              A\Reports::property Hub $hub  private  @ 4:45-4:49 (full 4:33-4:49)
              A\Reports::method run(): void  public  @ 6:21-6:24 (full 6:5-9:6)
            catalog
              [class] a\hub -> src/Hub.php (A\Hub)
              [class] a\registry -> src/Registry.php (A\Registry)
              [class] a\reports -> src/Reports.php (A\Reports)
            namespaces
              a -> A x3
            declared keys
            dispatched keys
              [event] on_cache_cleared_done -> src/Reports.php 220..235
              [event] on_index_rebuilt_done -> src/Reports.php 237..252
              [event] on_reports_requested -> src/Reports.php 151..170
        "#]],
    );
}

#[test]
fn only_a_name_the_call_wrote_out_is_one() {
    // A name built by concatenation is not a name this can read, and neither is one handed over in
    // a variable -- so both are silence rather than half a key. The list form keeps the literals
    // beside the expression it cannot read, because they are separate names and one of them is
    // still written down.
    check_index(
        r#"
//- /src/Hub.php
<?php
namespace A;
interface Hub {
    public function publish(string $name): void;
    public function announce(string $to, array $names): void;
}

//- /src/Reports.php
<?php
namespace A;
class Reports {
    public function __construct(private Hub $hub, private string $kind) {}

    public function run(): void {
        $this->hub->publish($this->kind);
        $this->hub->publish('reports_' . $this->kind);
        $this->hub->announce('everyone', ['settled', 'reports_' . $this->kind]);
    }
}
"#,
        expect![[r#"
            state ready
            file src/Hub.php [User]
              interface A\Hub @ 3:11-3:14
              A\Hub is interface
              A\Hub::method publish(string $name): void  public  @ 4:21-4:28 (full 4:5-4:49)
              A\Hub::method announce(string $to, array $names): void  public  @ 5:21-5:29 (full 5:5-5:62)
            file src/Reports.php [User]
              class A\Reports @ 3:7-3:14
              dispatches [event] on_settled_done by A\Hub::announce on A\Reports::hub @ 9:43-9:52
              A\Reports is class
              A\Reports::method __construct(private Hub $hub, private string $kind)  public  @ 4:21-4:32 (full 4:5-4:75)
              A\Reports::property Hub $hub  private  @ 4:45-4:49 (full 4:33-4:49)
              A\Reports::property string $kind  private  @ 4:66-4:71 (full 4:51-4:71)
              A\Reports::method run(): void  public  @ 6:21-6:24 (full 6:5-10:6)
            catalog
              [class] a\hub -> src/Hub.php (A\Hub)
              [class] a\reports -> src/Reports.php (A\Reports)
            namespaces
              a -> A x2
            declared keys
            dispatched keys
              [event] on_settled_done -> src/Reports.php 284..293
        "#]],
    );
}

#[test]
fn a_receiver_the_file_cannot_name_is_recorded_without_one() {
    // A bare local says nothing about what the call is on, and the row is kept anyway -- see the
    // dispatch module for why the two readers of this table want opposite things from that. What
    // makes it safe is that the row names no receiver at all, so nothing can mistake it for
    // evidence about which class was called.
    check_index(
        r#"
//- /src/Hub.php
<?php
namespace A;
interface Hub {
    public function publish(string $name): void;
}

//- /src/Reports.php
<?php
namespace A;
class Reports {
    public function run(Hub $hub): void {
        $hub->publish('reports_requested');
    }
}
"#,
        expect![[r#"
            state ready
            file src/Hub.php [User]
              interface A\Hub @ 3:11-3:14
              A\Hub is interface
              A\Hub::method publish(string $name): void  public  @ 4:21-4:28 (full 4:5-4:49)
            file src/Reports.php [User]
              class A\Reports @ 3:7-3:14
              dispatches [event] on_reports_requested by A\Hub::publish on an unidentified receiver @ 5:23-5:42
              A\Reports is class
              A\Reports::method run(Hub $hub): void  public  @ 4:21-4:24 (full 4:5-6:6)
            catalog
              [class] a\hub -> src/Hub.php (A\Hub)
              [class] a\reports -> src/Reports.php (A\Reports)
            namespaces
              a -> A x2
            declared keys
            dispatched keys
              [event] on_reports_requested -> src/Reports.php 99..118
        "#]],
    );
}

#[test]
fn a_dependency_declares_no_extension_point() {
    // Bodies are lowered for the project's own code and nothing else, so a dependency's dispatches
    // are invisible here. Deliberate, and the boundary the whole feature is worth having at: what a
    // framework dispatches is documented in files the index already reads, and what a project
    // dispatches is documented nowhere.
    check_index(
        r#"
//- /vendor/a/Hub.php
<?php
namespace A;
interface Hub {
    public function publish(string $name): void;
}
class Vendored {
    public function __construct(private Hub $hub) {}
    public function run(): void {
        $this->hub->publish('vendor_thing');
    }
}
"#,
        expect![[r#"
            state ready
            file vendor/a/Hub.php [Vendor]
              interface A\Hub @ 3:11-3:14
              class A\Vendored @ 6:7-6:15
              A\Hub is interface
              A\Hub::method publish(string $name): void  public  @ 4:21-4:28 (full 4:5-4:49)
              A\Vendored is class
              A\Vendored::method __construct(private Hub $hub)  public  @ 7:21-7:32 (full 7:5-7:53)
              A\Vendored::property Hub $hub  private  @ 7:45-7:49 (full 7:33-7:49)
              A\Vendored::method run(): void  public  @ 8:21-8:24 (full 8:5-10:6)
            catalog
              [class] a\hub -> vendor/a/Hub.php (A\Hub)
              [class] a\vendored -> vendor/a/Hub.php (A\Vendored)
            namespaces
              a -> A x2
            declared keys
            dispatched keys
        "#]],
    );
}

#[test]
fn editing_the_dispatching_call_moves_what_it_declares() {
    // The invalidation half, which is the one that fails silently: a claim a re-ingested file failed
    // to withdraw leaves a key pointing at a call that no longer writes it, and nothing but this
    // comparison would notice.
    check_incremental(
        DISPATCHING,
        &[Step::write(
            "src/Reports.php",
            r#"
<?php
namespace A;
class Reports {
    public function __construct(private Hub $hub) {}

    public function run(): void {
        $this->hub->publish('reports_renamed');
    }
}
"#,
        )],
        expect![[r#"
            state ready
            file src/Hub.php [User]
              interface A\Hub @ 3:11-3:14
              A\Hub is interface
              A\Hub::method publish(string $name): void  public  @ 4:21-4:28 (full 4:5-4:49)
              A\Hub::method announce(string $to, array $names): void  public  @ 5:21-5:29 (full 5:5-5:62)
            file src/Registry.php [User]
              class A\Registry @ 3:7-3:15
              A\Registry is class
              A\Registry::method hub(): Hub  public static  @ 4:28-4:31 (full 4:5-4:41)
            file src/Reports.php [User]
              class A\Reports @ 3:7-3:14
              dispatches [event] on_reports_renamed by A\Hub::publish on A\Reports::hub @ 7:29-7:46
              A\Reports is class
              A\Reports::method __construct(private Hub $hub)  public  @ 4:21-4:32 (full 4:5-4:53)
              A\Reports::property Hub $hub  private  @ 4:45-4:49 (full 4:33-4:49)
              A\Reports::method run(): void  public  @ 6:21-6:24 (full 6:5-8:6)
            catalog
              [class] a\hub -> src/Hub.php (A\Hub)
              [class] a\registry -> src/Registry.php (A\Registry)
              [class] a\reports -> src/Reports.php (A\Reports)
            namespaces
              a -> A x3
            declared keys
            dispatched keys
              [event] on_reports_renamed -> src/Reports.php 151..168
        "#]],
    );
}

#[test]
fn deleting_the_dispatching_file_takes_its_extension_points_with_it() {
    check_incremental(
        DISPATCHING,
        &[Step::delete("src/Reports.php")],
        expect![[r#"
            state ready
            file src/Hub.php [User]
              interface A\Hub @ 3:11-3:14
              A\Hub is interface
              A\Hub::method publish(string $name): void  public  @ 4:21-4:28 (full 4:5-4:49)
              A\Hub::method announce(string $to, array $names): void  public  @ 5:21-5:29 (full 5:5-5:62)
            file src/Registry.php [User]
              class A\Registry @ 3:7-3:15
              A\Registry is class
              A\Registry::method hub(): Hub  public static  @ 4:28-4:31 (full 4:5-4:41)
            catalog
              [class] a\hub -> src/Hub.php (A\Hub)
              [class] a\registry -> src/Registry.php (A\Registry)
            namespaces
              a -> A x2
            declared keys
            dispatched keys
        "#]],
    );
}

/// Opening a buffer must claim exactly what the closed reading of it claimed.
///
/// The hazard §8 records for the lowering gate, arriving through a different door. Opening a file
/// re-ingests it through `ingest_lowered` with the buffer's own HIR, so a fact derived on one path
/// and not the other means *opening a file empties the catalog* -- a table that hollows out while
/// somebody works, with nothing failing and no error anywhere.
///
/// `with_open_scopes` cannot see this: it opens buffers into an `OpenSet` and never re-ingests, so
/// the skeleton it compares against is the closed one both times. The re-ingest is what has to be
/// driven, and this is the only place that drives it.
#[test]
fn opening_the_dispatching_file_claims_exactly_what_closing_it_did() {
    use crate::OpenSet;

    let files = super::fixture_files(DISPATCHING);
    let mut built = super::build(&files, super::Order::ByPath);
    let closed = built.render();

    let path = "src/Reports.php";
    let file = built.ids.get(path).expect("the fixture declares it");
    let text = files[path].clone();

    let mut open = OpenSet::new();
    open.set_keyed_attributes(built.skeleton.keyed_attributes().clone());
    open.open(file, text.clone());
    let hir = open.get(file).expect("just opened").hir();
    built.skeleton.ingest_lowered(
        file,
        &super::fixture_path(path),
        phrost_model::SourceOrigin::User,
        &text,
        phrost_vfs::FileStamp::of(text.as_bytes()),
        hir,
    );

    assert_eq!(
        closed,
        built.render(),
        "opening the dispatching file changed what the index holds"
    );
}
