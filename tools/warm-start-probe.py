"""Whether a warm server answers what a cold one does.

`phrost check` and `phrost query` are both cold starts, so neither can see a defect in the path that
reads a stored index. One such defect -- `ProjectSkeleton::adopt` re-claiming one of the three kinds
of catalog key -- cost 1 315 catalog keys on every server start after the first while both of those
instruments passed every probe that was thrown at them. The only way to see it is to drive a real
`serve` twice: once with no cache, and once reading the cache the first run wrote.

Writes nothing into the project. It opens files that are already there and compares two things the
server says about them.

**Diagnostics**, which is how the defect above was found: one file's report went from five to two
across a restart. But that half only sees a lost key where the project has a *mistake* -- an open
catalog says nothing about a key that is merely absent, so correct code is silent either way.

So also **hover over every string literal in the file**, which sees it on correct code: a catalog key
answers with what registers it and a plain string answers nothing, so a key the warm index lost turns
an answer into a null. No framework knowledge is needed for that and no guess about which literal is
a key -- ask about all of them and let the difference speak.

The cheap companion to this is `phrost index <root>` against `phrost index <root> --warm`, which
compares the catalog *counts* and so sees a lost key whether or not any file mentions it. Use that
first; use this to find out what a person would actually have seen.

Usage: python3 tools/warm-start-probe.py <project root> <file> [<file> ...]

Exit status is 1 when the two runs disagree, so this can gate a change to the cache, to `adopt`, or
to anything a file's ingest claims. Both halves of that are checked against the real defect: with
`adopt` put back the way it was, this reports

    DIFF  .../NotificationManager.php
            hover 166:58: cold "'notifications_alter' wh_consent_tracker_notif" / warm '(nothing)'

and exits 1 -- from a file whose code is *correct*, which is the case the diagnostics half cannot see.
"""
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lspclient import Server, path_of, uri_of

# A request arriving before bootstrap finishes is refused on purpose, and the refusal carries no
# `result` -- so it has to be retried rather than read as a null answer. A probe that skips this
# reports every capability dead at once, which is not the shape of any single defect and is the tell.
STILL_INDEXING = -32801


def cache_root():
    """Where the server stores an index, which is `dirs::cache_dir()/phrost/index`."""
    if sys.platform == "darwin":
        return os.path.expanduser("~/Library/Caches/phrost/index")
    return os.path.join(
        os.environ.get("XDG_CACHE_HOME", os.path.expanduser("~/.cache")), "phrost", "index"
    )


def literals_in(text):
    """Zero-based positions inside every simple string literal, as the protocol counts them.

    Escapes are skipped rather than handled: a literal containing one is not a catalog key in any
    framework here -- keys are machine identifiers -- and the point is to ask about many positions
    cheaply, not to lex PHP.
    """
    found = []
    for line, source in enumerate(text.split("\n")):
        at = 0
        while at < len(source):
            quote = source.find("'", at)
            double = source.find('"', at)
            if quote < 0 or (0 <= double < quote):
                quote = double
            if quote < 0:
                break
            end = source.find(source[quote], quote + 1)
            if end < 0:
                break
            if end > quote + 1 and "\\" not in source[quote + 1 : end]:
                found.append((line, quote + 1 + (end - quote - 1) // 2))
            at = end + 1
    return found


def published_for(root, files, label):
    """What the server says about `files`: its diagnostics, and its hover at every literal."""
    binary = os.path.join(
        os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "target/release/phrost"
    )
    found = {}

    def heard(method, params):
        if method == "textDocument/publishDiagnostics":
            found[path_of(params["uri"])] = params["diagnostics"]

    server = Server("phrost", [binary, "serve"], root)
    server.on_notification(heard)
    server.initialize(
        {
            "textDocument": {"publishDiagnostics": {}, "hover": {}},
            "window": {"workDoneProgress": True},
        }
    )
    for path in files:
        with open(path, encoding="utf-8", errors="replace") as handle:
            server.open(path, handle.read())

    # One request, retried, is how this waits: it is answered only once bootstrap is done, which is a
    # fact about the server rather than a guess about how long indexing takes.
    for _ in range(120):
        answer = server.request(
            "textDocument/hover",
            {
                "textDocument": {"uri": uri_of(files[0])},
                "position": {"line": 0, "character": 0},
            },
            timeout=60.0,
        )
        if answer.get("error", {}).get("code") != STILL_INDEXING:
            break
        server.pump(0.5)
    else:
        sys.exit(f"{label}: still indexing after a minute")

    # Diagnostics are published rather than asked, so they are waited for -- and waited for until the
    # server goes quiet, because a file is republished as more of the index becomes available.
    for _ in range(20):
        if not server.poll(1.5) and len(found) >= len(files):
            break

    answers = {}
    for path in files:
        with open(path, encoding="utf-8", errors="replace") as handle:
            text = handle.read()
        hovered = {}
        for line, character in literals_in(text):
            answer = server.request(
                "textDocument/hover",
                {
                    "textDocument": {"uri": uri_of(path)},
                    "position": {"line": line, "character": character},
                },
                timeout=30.0,
            ).get("result")
            contents = (answer or {}).get("contents")
            value = contents.get("value") if isinstance(contents, dict) else contents
            if value:
                hovered[f"{line + 1}:{character + 1}"] = value
        answers[path] = (
            sorted(d["message"] for d in found.get(path, [])),
            hovered,
        )
    server.stop()
    return answers


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__.strip().splitlines()[-3])
    root = os.path.abspath(sys.argv[1])
    files = [os.path.abspath(name) for name in sys.argv[2:]]
    for path in files:
        if not os.path.isfile(path):
            sys.exit(f"no such file: {path}")

    shutil.rmtree(cache_root(), ignore_errors=True)
    print(f"cold  -- no stored index, over {len(files)} file(s)")
    cold = published_for(root, files, "cold")
    print("warm  -- reading the cache the cold run wrote")
    warm = published_for(root, files, "warm")

    differing = 0
    for path in files:
        short = os.path.relpath(path, root)
        (cold_found, cold_hover), (warm_found, warm_hover) = cold[path], warm[path]
        if (cold_found, cold_hover) == (warm_found, warm_hover):
            print(f"  same  {len(cold_found)} diagnostics, {len(cold_hover)} literals answer  {short}")
            continue
        differing += 1
        print(f"  DIFF  {short}")
        for message in cold_found:
            if message not in warm_found:
                print(f"          diagnostic only cold: {message}")
        for message in warm_found:
            if message not in cold_found:
                print(f"          diagnostic only warm: {message}")
        for where in sorted(set(cold_hover) | set(warm_hover)):
            was, now = cold_hover.get(where), warm_hover.get(where)
            if was != now:
                one = " ".join((was or "(nothing)").split())[:60]
                two = " ".join((now or "(nothing)").split())[:60]
                print(f"          hover {where}: cold {one!r} / warm {two!r}")

    if differing:
        print(f"\nwarm start disagrees with cold on {differing} of {len(files)} file(s)")
        return 1
    print(f"\ncold == warm over {len(files)} file(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
