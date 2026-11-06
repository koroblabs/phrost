//! A buffer in a language this server does not read.
//!
//! A client decides which languages it routes here, and it may route more than PHP: the Zed
//! extension registers YAML beside PHP on purpose, because Drupal and Symfony declare their services
//! there and a later feature will want them. Until something reads YAML, one must cost nothing --
//! not a parse on every keystroke, and above all not a claim on the names it appears to declare.

use crate::tests::{Fixture, definition_target};

/// A YAML buffer that looks like PHP still declares nothing.
///
/// The strongest observable there is: if the buffer had been ingested, the class in it would resolve
/// from real PHP. Bootstrap cannot be what indexed it -- discovery only walks PHP extensions -- so
/// this can only be answered by whether the open path took it.
#[tokio::test]
async fn a_yaml_buffer_is_not_indexed_as_php() {
    let mut server = Fixture::started(
        r#"
//- /app/services.yml
<?php namespace App; class Ghost {}

//- /src/User.php
<?php
namespace App;

class User extends Gh@ghost@ost {}
"#,
        "src/User.php",
    )
    .await;

    server.open("app/services.yml").await;
    server.settle().await;

    let response = server.definition("src/User.php", "ghost").await;
    assert!(
        definition_target(&response, server.project.root()).is_none(),
        "a class written in a YAML buffer must not become a declaration"
    );
    assert!(
        !server
            .diagnostics_published()
            .iter()
            .any(|line| line.contains("services.yml")),
        "and nothing is published about a buffer that was never read"
    );
}

/// An extension a framework contributed *is* PHP, and is read like it.
///
/// The other half of the same rule: the guard asks the project which extensions hold PHP, so
/// Drupal's `.module` files must pass it. A guard that only knew `.php` would silently stop indexing
/// every hook implementation in a Drupal project.
#[tokio::test]
async fn a_framework_contributed_extension_is_still_read() {
    let mut server = Fixture::started(
        r#"
//- /composer.json
{}

//- /web/core/lib/Drupal.php
<?php class Drupal {}

//- /web/modules/mine/mine.info.yml
name: Mine

//- /web/modules/mine/mine.module
<?php namespace App; class Hook {}

//- /src/User.php
<?php
namespace App;

class User extends Ho@hook@ok {}
"#,
        "src/User.php",
    )
    .await;

    server.open("web/modules/mine/mine.module").await;
    server.settle().await;

    let response = server.definition("src/User.php", "hook").await;
    assert!(
        definition_target(&response, server.project.root()).is_some(),
        "a .module buffer holds PHP and the project says so"
    );
}

/// A configuration buffer answers about the names it holds, and still declares nothing.
///
/// Both halves in one test on purpose. The reason a YAML buffer was refused outright is that the
/// two index tiers are updated from the one string a document event carries, so holding one meant
/// claiming its declarations -- and the way this is safe is that the text is held and nothing else.
/// A test that only checked the jump would pass with the buffer ingested, which is the failure the
/// refusal existed to prevent.
///
/// The marker is placed after the dot of the service id deliberately. A fixture marker is `@name@`,
/// and a container reference is `'@name'` -- so `'@mi@wired@ne.repo'` reads as a marker called `mi`,
/// which the marker format's own docs say cannot happen in PHP and plainly can in YAML. Past a dot
/// the first `@` is followed by a name the marker reader refuses, and the real marker is found.
#[tokio::test]
async fn a_configuration_buffer_answers_and_declares_nothing() {
    let mut server = Fixture::started(
        r#"
//- /composer.json
{}

//- /web/core/lib/Drupal.php
<?php class Drupal {}

//- /web/modules/mine/mine.info.yml
name: Mine

//- /web/modules/mine/src/Repo.php
<?php
namespace Drupal\mine;

class Repo {}

//- /web/modules/mine/mine.services.yml
services:
  mine.repo:
    class: Drupal\mine\Repo
  mine.other:
    class: Drupal\mine\Repo
    arguments: ['@mine.re@wired@po', '@mine.repp']

//- /src/User.php
<?php
namespace App;

class User extends Gh@ghost@ost {}
"#,
        "src/User.php",
    )
    .await;

    server.open("web/modules/mine/mine.services.yml").await;
    server.settle().await;

    let response = server
        .definition("web/modules/mine/mine.services.yml", "wired")
        .await;
    assert_eq!(
        definition_target(&response, server.project.root()).as_deref(),
        Some("web/modules/mine/src/Repo.php"),
        "a container id in a held buffer reaches the class the catalog says it names"
    );

    // The same buffer names `Ghost` nowhere, and the class it *does* name is a real one -- so the
    // observable that a YAML buffer declares nothing has to be the one it always was.
    let response = server.definition("src/User.php", "ghost").await;
    assert!(
        definition_target(&response, server.project.root()).is_none(),
        "and nothing written in a configuration buffer becomes a declaration"
    );
    // And it *is* checked. A container id is a key of a catalog read from files that can be read in
    // full, so one missing from it does not exist -- the same claim, judged by the same function, as
    // the identical mistake written in PHP.
    let published = server.diagnostics_published();
    let about = published
        .iter()
        .filter(|line| line.contains("services.yml"))
        .collect::<Vec<_>>();
    assert!(
        about.iter().any(|line| line.contains("mine.repp")),
        "an id no catalog holds is reported, and near enough to a real one to say which: {about:?}"
    );
    // One finding and not two: the id beside it is one the catalog holds. Counted rather than
    // matched on the text, because the suggestion names the good id inside the bad one's message.
    assert_eq!(
        about.len(),
        1,
        "and the id beside it, which the catalog does hold, is not reported: {about:?}"
    );
}
