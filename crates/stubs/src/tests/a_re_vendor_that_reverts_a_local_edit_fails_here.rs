//! The shape of everything this project changed in the vendored set, in one place.
//!
//! `third_party/php-prelude-upstream` is `mago-prelude` as published and never edited; the set
//! beside it is that plus some ten thousand changed lines. Until both existed, "what did we change"
//! was prose in a README written from memory -- and it was already incomplete. Four of its seven
//! recorded categories had nothing at all that would fail if a re-vendor reverted them, and the
//! fifth only because somebody remembered to add a row to a table by hand.
//!
//! So this is the dumb, total one: every file, how many lines are ours and how many are upstream's.
//! An edit that vanishes moves a number down and names its file. An edit nobody recorded moves one
//! up, which is the same demand the other README makes in prose and this makes at the suite.
//!
//! # Why a line multiset and not a diff
//!
//! Because what it has to answer is "is our delta still there", not "what does it read like" --
//! `just stubs-diff` answers the second and a person is the right reader for it. Comparing the
//! lines as multisets needs no diff algorithm, is stable under a line moving, and moves for exactly
//! the edits that matter: an added tag, a rewritten one, a whole docblock restored to upstream's.
//!
//! # What the first reading settled
//!
//! That the prose record is accurate for what it lists. Almost every file is `+N -0` -- the prose
//! pass only ever added lines -- and every file that *removes* any accounts for itself exactly:
//! `date.php -14` is `DateInterval`'s eleven `readonly` modifiers plus three merged docblocks,
//! `dom.php -33` and one of `spl.php -4` and `xmlreader.php -3` are the rest of the forty, the other
//! three of `spl.php` are the `class_*` returns rewritten as maps, and `standard.php -6` holds the
//! rewritten `base64_decode`. Four numbers agreeing with four sentences
//! written months apart is a better argument for the README than the README is.
//!
//! The semantic tripwires beside this one -- [`super::the_added_type_tags_survive`],
//! [`super::the_documented_names_still_carry_their_prose`],
//! [`super::no_declaration_carries_two_docblocks`] -- say that a *particular* tag is still attached
//! to a *particular* declaration. This says the delta is still the size and shape it was. Neither
//! subsumes the other: this one covers the edits nobody wrote a test for, and those name which edit
//! and where.

use std::{collections::BTreeMap, fmt::Write as _, fs, path::PathBuf};

use expect_test::expect;

/// One file's delta: lines only in ours, and lines only in upstream's.
struct Delta {
    ours: usize,
    theirs: usize,
}

#[test]
fn the_local_delta_is_the_one_that_was_recorded() {
    let vendored = third_party("php-prelude");
    let upstream = third_party("php-prelude-upstream");

    let mut deltas: BTreeMap<String, Delta> = BTreeMap::new();
    let mut missing = Vec::new();

    for name in stub_names(&upstream) {
        let Ok(theirs) = fs::read_to_string(upstream.join(&name)) else {
            continue;
        };
        let Ok(ours) = fs::read_to_string(vendored.join(&name)) else {
            missing.push(format!("{name} is in upstream and not in the vendored set"));
            continue;
        };
        let delta = Delta {
            ours: lines_not_in(&ours, &theirs),
            theirs: lines_not_in(&theirs, &ours),
        };
        if delta.ours > 0 || delta.theirs > 0 {
            deltas.insert(name, delta);
        }
    }
    // A file this project added outright is a local edit too, and the loudest one to lose.
    for name in stub_names(&vendored) {
        if !upstream.join(&name).exists() {
            missing.push(format!("{name} is ours alone"));
        }
    }

    let mut rendered = String::new();
    for (name, delta) in &deltas {
        let _ = writeln!(rendered, "{name} +{} -{}", delta.ours, delta.theirs);
    }
    for note in &missing {
        let _ = writeln!(rendered, "{note}");
    }
    let _ = writeln!(
        rendered,
        "\n{} files differ, {} lines ours",
        deltas.len(),
        deltas.values().map(|delta| delta.ours).sum::<usize>()
    );

    expect![[r#"
        core.php +686 -0
        ctype.php +35 -0
        date.php +566 -14
        dom.php +616 -33
        fileinfo.php +38 -0
        filter.php +30 -0
        hash.php +97 -0
        iconv.php +60 -0
        json.php +30 -0
        libxml.php +33 -0
        mbstring.php +304 -0
        openssl.php +3 -3
        pcre.php +58 -4
        pdo.php +264 -0
        phar.php +437 -0
        redis.php +296 -0
        reflection.php +971 -4
        session.php +192 -0
        simple-xml.php +171 -0
        spl.php +1388 -1
        sqlite3.php +121 -0
        standard.php +2396 -8
        tokenizer.php +42 -0
        xml.php +92 -0
        xmlreader.php +143 -3
        xmlwriter.php +338 -0
        zip.php +308 -0
        zlib.php +138 -0
        swoole.php is in upstream and not in the vendored set

        28 files differ, 9853 lines ours
    "#]]
    .assert_eq(&rendered);
}

/// Where a `third_party` directory is, from the crate this test lives in.
fn third_party(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party")
        .join(name)
}

/// Every stub file in a directory, by name.
fn stub_names(directory: &PathBuf) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .expect("the third_party directory is part of the repository")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == "php").then(|| path.file_name()?.to_str().map(str::to_owned))?
        })
        .collect();
    names.sort();
    names
}

/// How many of `text`'s lines the other side does not also have, counting repeats.
fn lines_not_in(text: &str, other: &str) -> usize {
    let mut available: BTreeMap<&str, usize> = BTreeMap::new();
    for line in other.lines() {
        *available.entry(line).or_default() += 1;
    }
    text.lines()
        .filter(|line| match available.get_mut(line) {
            Some(0) | None => true,
            Some(left) => {
                *left -= 1;
                false
            }
        })
        .count()
}
