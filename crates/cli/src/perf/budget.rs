//! The performance budgets.
//!
//! This file is the source of truth for what a request is allowed to cost. The table in
//! `CLAUDE.md` is a description of it; if the two disagree, this one is right.
//!
//! Budgets are p95, not averages. An average hides the case a person actually notices: the
//! completion that took 200 ms while the other ninety-nine took two. The numbers are phrost's; the
//! percentile and the verdict are `lspkit_perf`'s, shared with orthra.

use lspkit_perf::Budget;

pub(crate) const BUDGETS: &[Budget] = &[
    Budget::ms("completion", 12.0),
    // The list and the declaration of the row a person lands on, which arrives as its own request.
    // One budget for the pair because it is one interaction: split in two, both halves can read as
    // fine while what anyone waits through does not.
    Budget::ms("completion + commit", 20.0),
    Budget::ms("hover", 12.0),
    // Asked on every cursor move, so it belongs with hover: the editor redraws the marks as the
    // caret goes.
    Budget::ms("document highlight", 12.0),
    Budget::ms("goto definition", 12.0),
    Budget::ms("diagnostics", 30.0),
    Budget::ms("inlay hints", 12.0),
    // The same class as hover: it fires from a keystroke and is read immediately.
    Budget::ms("signature help", 12.0),
    // Its own row because it is its own path: hundreds of candidates from the whole project, each
    // needing to be told how it would have to be written. The member-access row says nothing about
    // it, and for a long time nothing did.
    Budget::ms("completion of a name", 12.0),
    // The same list with the name half typed. The row above asks at the end of a name, and a
    // list is asked for on every letter before that: fewer letters are held loosely by more of
    // the project's names, each of which is scored. Measured on Symfony's 13 118 files over
    // 22 039 names: p95 1.20 ms at the end and 1.48 ms halfway, so a quarter dearer and not a
    // different order -- which is worth a row because the first guess, from one request timed
    // through a server, was seven times that. See `docs/08-performance.md`.
    Budget::ms("completion of half a name", 12.0),
    // The only budget that pays for a parse. It reads the buffer rather than the index, which
    // is what lets it answer before the project is indexed -- and what makes it the one row
    // whose cost grows with the size of a single file rather than with the project.
    Budget::ms("folding ranges", 12.0),
    // The other buffer-only row, and the one that produces the most: thousands of names per
    // file, each of which the client is sent five numbers for.
    Budget::ms("semantic tokens", 12.0),
    // Whole-file and answered from the skeleton, so it costs a walk of what one file declares and
    // nothing else. Budgeted with the rest at 12 ms rather than tighter: the point of the row is
    // that a change which starts reading bodies to build an outline shows up here.
    Budget::ms("document symbols", 12.0),
    // Pulled rather than pushed, so a person is waiting for it with a menu half-open. Measured over
    // the whole file at once, which is the ceiling rather than the ask.
    Budget::ms("code actions", 12.0),
    // The same request as an editor sends it: an empty selection at the cursor. Budgeted tighter
    // than the ceiling because this is the one a person actually waits on, and because the
    // applicability gate that keeps the menu affordable is invisible in the row above -- a range
    // covering the whole file matches every subject in it.
    Budget::ms("code actions at a cursor", 12.0),
    // Deciding which callable the cursor is on, which is a declaration lookup and a containment
    // search over what one file declares. The same class as goto, and budgeted with it.
    Budget::ms("call hierarchy head", 12.0),
    // One declaration's body, resolved call by call. Nearer hover than goto in what it does -- each
    // call is a receiver to resolve -- so it gets hover's budget rather than a looser one.
    Budget::ms("outgoing calls", 12.0),
    // The other project-wide search, and unlike incoming calls it is sent on every character rather
    // than on a menu click -- so it is budgeted with the keystroke rows and not two orders of
    // magnitude above them. It reads no file: the catalog answers the top-level half and the
    // project's own class tables answer the member half. That is what the row protects. Walking
    // every member row in `vendor/` instead of the project's own, or reading a file per hit to
    // place it, are both changes that look local and would land here at ten times the number.
    Budget::ms("workspace symbols", 12.0),
    // The one row here that greps, and the reason it has a row at all: a
    // request nobody measures is how a budget gets blown quietly. Two orders of magnitude above
    // everything else on purpose -- it greps every file the project owns and then analyses the
    // ones that matched, which is the same work find-references does and is not something a
    // 12 ms budget could describe. A person invoking it knows they asked for a search; what this
    // number protects is that it stays a search and does not become a re-index.
    Budget::ms("incoming calls", 1500.0),
];
