//! Inline multi-file PHP fixtures and snapshot helpers.
//!
//! One fixture format is used by every layer, so a test written against the def-map and a test
//! written against completion describe their input the same way. That is what makes the layered
//! debugging story work: the same fixture can be pointed at a different layer to find out where
//! a fact went missing.
//!
//! # Format
//!
//! ```text
//! //- /composer.json
//! {"autoload": {"psr-4": {"App\\": "src/"}}}
//!
//! //- /src/Repo.php
//! <?php
//! namespace App;
//! class Repo {
//!     public function find(int $id): ?User {}
//! }
//!
//! //- /src/Use.php  open
//! <?php
//! namespace App;
//! function go(Repo $r) {
//!     $u = $r->fi@member@nd(1);
//! }
//!
//! //- /vendor/psr/log/LoggerInterface.php  vendor
//! <?php
//! namespace Psr\Log;
//! interface LoggerInterface {}
//! ```
//!
//! The syntax -- headers, `$0` and `@name@` markers, escapes, indentation, and the rule that every
//! marker must be asked about -- is `lspkit_fixture`'s, shared with orthra. What is phrost's is what
//! the header flags mean: `vendor`, `stub` (source origin), `open` / `closed` (editor state, closed
//! by default), `php:8.3` (per-file language version). A flag that is none of these stops the test.
//!
//! Paths are spelled without the leading slash once parsed (`spec.file("src/Repo.php")`), which is
//! how every layer above names a file relative to the project root.

mod materialize;
mod snapshot;
mod spec;
#[cfg(test)]
mod tests;

pub use lspkit_fixture::{CURSOR, Marker as FixtureMarker, Markers as FixtureMarkers};

pub use self::{
    materialize::MaterializedFixture,
    snapshot::{Snapshot, render_position, render_range},
    spec::{FixtureFile, FixtureSpec},
};
