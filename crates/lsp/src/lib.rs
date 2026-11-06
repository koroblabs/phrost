//! The LSP server: transport, dispatch, and protocol conversion. No PHP semantics.
//!
//! Handlers here translate, guard, call, and convert. They do not parse PHP, walk the workspace, or
//! decide what a name means — every answer comes from `phrost-analysis` over a `phrost-view`
//! projection. The parser and the store crates are not in this crate's dependency graph.
//!
//! Two things this layer genuinely owns, because they are protocol concerns and nothing below
//! should carry them:
//!
//! - **Encoding.** LSP counts columns in UTF-16 code units; phrost counts UTF-8 bytes. See
//!   [`convert`].
//! - **Markdown.** Hover comes back as blocks and is rendered here, so a formatting change does not
//!   ripple into the feature layer's tests. See [`markdown`].

mod answers;
mod bootstrap;
mod convert;
mod documents;
mod markdown;
mod project;
mod server;
mod session;

#[cfg(test)]
mod tests;

/// The binary's allocator, in this crate's tests as well: the flow tests then run on what the
/// server runs on, and `tests::a_session_gives_its_memory_back` can read what is live.
#[cfg(test)]
#[global_allocator]
static HEAP: lspkit_heap::Heap = lspkit_heap::Heap;
mod watching;
mod workspace;

pub use self::workspace::{DocumentUpdate, Reference, WatchedOutcome, Workspace};

use self::server::Backend;

/// What the server calls itself in the messages `lspkit_runtime` writes for it: "phrost failed while
/// answering ...". Named once, because a contained panic is reported in these words and a test pins
/// them.
const NAME: &str = "phrost";

/// Serves the protocol over stdin and stdout.
///
/// [`lspkit_runtime::serve_stdio`] wraps the service before serving it: a bug in a handler answers
/// with an error and the session carries on, where it once took the whole process with it, and
/// stdout goes through a queue, so how fast the client reads is not how fast the server runs. The
/// tools next to this in the binary are deliberately left bare — see [`lspkit_runtime::panics`].
///
/// What comes back is the exit status the protocol asks for: 0 after `shutdown` then `exit`, 1 for
/// an `exit` without `shutdown`. The session ends at `exit` even while the client keeps stdin open.
pub async fn serve() -> std::process::ExitCode {
    lspkit_runtime::serve_stdio(NAME, Backend::new)
        .await
        .exit_code()
}
