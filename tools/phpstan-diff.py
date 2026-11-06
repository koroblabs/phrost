#!/usr/bin/env python3
"""What PHPStan reports on a project and what phrost says about the same code.

The question this answers is "are we worse", and it has to be asked of a real checkout because
neither tool's rule list predicts what a project actually trips. Run it at the level the project
uses and at the level above, because the two differ in kind: PHPStan's level 9 is a position about
`mixed` rather than a set of rules, and counting it against phrost reads as a 2 800-finding gap
where the honest number is a few hundred.

Three columns, and the third is the one to read:

  rule            PHPStan's error identifier, which is its stable name for a rule
  n               how many it reports
  ours            what phrost says at those same sites, by check name and count

The third column names the checks rather than counting them, because a bare count lies here. The
two project-level checks fire on most declarations in a documented project, so any rule at all
overlaps them by luck -- a column reading `194 / 20` invites "we catch a tenth of these" when all
twenty were `doc-missing-throws` landing on the same line for unrelated reasons. Named, the reader
sees at once whether the overlap is a type check or a coincidence.

The match is within a line either way. A receiver and its call are often a line apart, and the two
tools underline different ends of one mistake -- phrost names the value on the left of `->` where
PHPStan names the call.

Findings phrost has and PHPStan has none of are counted separately at the end. There are two whole
categories: what a docblock fails to declare about what a call can throw, and what a project
declares and nothing reaches.

    tools/phpstan-diff.py <project> [--level N] [--phrost path/to/phrost]

Needs a vendored phpstan (`vendor/bin/phpstan`) and the project's own configuration, and a memory
limit large enough for it -- a crashed worker reports zero errors and zero files, which reads
exactly like a clean project. This passes `--memory-limit=6G` and refuses a run whose `errors` list
is not empty.
"""

import argparse
import collections
import json
import pathlib
import re
import subprocess
import sys


def phpstan(project, level, config, paths) -> dict:
    binary = project / "vendor" / "bin" / "phpstan"
    if not binary.exists():
        sys.exit(f"no phpstan at {binary}")
    # A project whose configuration names no path -- Laravel ships one per component -- has to be
    # told which one, and a project with several has to be told which of them. Both are the
    # project's own business, which is why they are arguments rather than a rule written here.
    chosen = ["-c", config] if config else []
    run = subprocess.run(
        [str(binary), "analyse", "--no-progress", "--memory-limit=6G",
         f"--level={level}", "--error-format=json", *chosen, *paths],
        cwd=project, capture_output=True, text=True,
    )
    try:
        report = json.loads(run.stdout)
    except json.JSONDecodeError:
        sys.exit(f"phpstan produced no report at level {level}:\n{run.stdout[-2000:]}")
    # A crashed worker leaves the count at zero and says so here rather than in the exit code.
    if report.get("errors"):
        sys.exit("phpstan did not finish: " + "; ".join(report["errors"])[:400])
    return report


def phrost(binary: str, project: pathlib.Path) -> dict:
    run = subprocess.run([binary, "check", str(project)], capture_output=True, text=True)
    found = collections.defaultdict(set)
    for line in run.stdout.splitlines():
        site = re.match(r"(\S+?):(\d+):\d+: \w+ ([\w-]+):", line)
        if site:
            found[(site.group(1), int(site.group(2)))].add(site.group(3))
    if not found:
        sys.exit(f"phrost reported nothing at all, which is not an answer:\n{run.stdout[-800:]}")
    return found


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("project", type=pathlib.Path)
    parser.add_argument("--level", type=int, default=8)
    parser.add_argument("--phrost", default="./target/release/phrost")
    parser.add_argument(
        "--rule",
        help="list the sites of one rule instead of the summary, with what phrost says at each",
    )
    parser.add_argument("--sites", type=int, default=25)
    parser.add_argument("--config", help="a phpstan configuration the project ships, by path")
    parser.add_argument("paths", nargs="*", help="what to analyse, where the configuration says nothing")
    args = parser.parse_args()

    project = args.project.resolve()
    ours = phrost(args.phrost, project)
    report = phpstan(project, args.level, args.config, args.paths)

    root = str(project) + "/"
    by_rule = collections.defaultdict(lambda: [0, collections.Counter()])
    for name, entry in report["files"].items():
        relative = name.replace(root, "")
        for message in entry["messages"]:
            rule = message.get("identifier") or "(unnamed)"
            by_rule[rule][0] += 1
            # Within a line, because the two tools underline different ends of one mistake.
            for near in (0, -1, 1):
                for code in ours.get((relative, message["line"] + near), ()):
                    by_rule[rule][1][code] += 1

    # One rule's sites, which is what turns the summary into work: a count says a gap exists and a
    # list says whether it is one gap or forty, and whether phrost is silent because the check is
    # missing or because the receiver it would need never resolved.
    if args.rule:
        shown = 0
        for name, entry in sorted(report["files"].items()):
            relative = name.replace(root, "")
            for message in entry["messages"]:
                if (message.get("identifier") or "(unnamed)") != args.rule:
                    continue
                shown += 1
                if shown > args.sites:
                    continue
                said = ", ".join(
                    code
                    for near in (0, -1, 1)
                    for code in ours.get((relative, message["line"] + near), ())
                ) or "(phrost says nothing)"
                print(f"{relative}:{message['line']}: {message['message'][:120]}")
                print(f"    {said}")
        print()
        print(f"{shown} sites of {args.rule}")
        return

    docblock = sum(n for rule, (n, _) in by_rule.items() if rule.startswith("missingType"))
    mixed = sum(1 for e in report["files"].values() for m in e["messages"]
                if "mixed" in m["message"] and not (m.get("identifier") or "").startswith("missingType"))
    total = report["totals"]["file_errors"]

    print(f"phpstan level {args.level} over {project.name}: {total} findings")
    print(f"  {docblock:>6}  a docblock is less precise than it could be (missingType.*)")
    print(f"  {mixed:>6}  a value is `mixed`, which is a position rather than a rule")
    print(f"  {total - docblock - mixed:>6}  the rest, which is what 'are we worse' is about")
    print()
    print(f"  {'rule':<38}{'n':>6}  ours")
    for rule, (n, hit) in sorted(by_rule.items(), key=lambda kv: -kv[1][0]):
        if rule.startswith("missingType"):
            continue
        said = ", ".join(f"{code} {count}" for code, count in hit.most_common(3)) or "-"
        print(f"  {rule:<38}{n:>6}  {said}")

    print()
    mine = collections.Counter(code for codes in ours.values() for code in codes)
    print("  phrost, by check:")
    for code, n in mine.most_common():
        print(f"  {code:<38}{n:>6}")


if __name__ == "__main__":
    main()
