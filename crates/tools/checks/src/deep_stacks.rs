//! Every thread that lowers PHP gets a stack big enough for it.
//!
//! Lowering, resolution and the linter all recurse over the expression tree, so the *smallest*
//! stack in the process decides what phrost survives -- and a stack overflow is the one failure
//! no panic containment can catch, because it is an abort rather than an unwind. There is no
//! error to report and nothing to restart from: the process is gone.
//!
//! The process gives `main` and every tokio worker
//! [`DEEP_RECURSION_STACK`](phrost_model::DEEP_RECURSION_STACK). A **scoped** thread gets neither.
//! Both of the two that exist had the default until this was checked, and one of them was measured
//! aborting `phrost check` on three shapes the server answers -- a 4 000-deep method chain, a
//! 20 000-term `+`, a 20 000-term `.`. A checker that dies on a legal file reports a clean bill it
//! has not earned.
//!
//! So the spelling is forbidden rather than the mistake documented: `scope.spawn(..)` takes the
//! default and `Builder::new().stack_size(..).spawn_scoped(scope, ..)` does not, and only the
//! second is allowed. §1.1's rule -- the two sites were routed by hand, and the third is the one
//! nobody remembers.

#[cfg(test)]
use std::{fmt::Write as _, fs};

#[cfg(test)]
use crate::workspace_root;

/// The spelling that takes whatever the OS defaults to.
#[cfg(test)]
const DEFAULTED: &str = "scope.spawn(";

#[test]
fn a_scoped_thread_is_given_a_stack() {
    let root = workspace_root();
    let mut offenders = Vec::new();

    for entry in super::walk_rust_files(&root.join("crates")) {
        let path = entry
            .strip_prefix(&root)
            .unwrap_or(&entry)
            .to_string_lossy()
            .replace('\\', "/");
        // This crate names the spelling in order to look for it.
        if path.starts_with("crates/tools/") {
            continue;
        }
        if fs::read_to_string(&entry)
            .unwrap_or_default()
            .contains(DEFAULTED)
        {
            offenders.push(path);
        }
    }

    if offenders.is_empty() {
        return;
    }

    let mut report = String::from(
        "a scoped thread takes the operating system's default stack, not the one this process gave \
         `main` and its tokio workers -- and lowering recurses over the expression tree, so the \
         smallest stack decides what phrost survives.\n\nThese spawn one without a \
         size:\n\n",
    );
    for path in &offenders {
        let _ = writeln!(report, "  {path}");
    }
    let _ = write!(
        report,
        "\nUse `std::thread::Builder::new().stack_size(phrost_model::DEEP_RECURSION_STACK)\
         .spawn_scoped(scope, ..)` instead. A stack overflow is an abort, so there is no error to \
         report and nothing to restart from."
    );
    panic!("{report}");
}
