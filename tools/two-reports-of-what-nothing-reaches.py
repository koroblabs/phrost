#!/usr/bin/env python3
"""`phrost unused` and the editor's `unused-declaration` name the same declarations.

    python3 tools/two-reports-of-what-nothing-reaches.py <project-root>

Two assemblies of one rule. The report gathers the reachability rules itself; the diagnostic asks
the view for them. Nothing fails when one assembly is missing a case, and both have been -- the
diagnostic once lacked the framework's discovery and the configuration, and the report lacked the
templates, the magic methods and the rule that a dead class's members say nothing more than the
class does. Measured before those were closed: **196 against 99** on one real checkout.

The diagnostic has to be switched on (`lint.on = ["unused-declaration"]` in `phrost.toml`), which is
what makes it a rule a project opts into rather than one it is given; without it this says so and
answers nothing.
"""
import argparse
import os
import re
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PHROST = os.path.join(REPO, "target/release/phrost")

REPORTED = re.compile(r"^(.+?):(\d+): (class|interface|trait|enum|method|constant) (\S+)$")
CHECKED = re.compile(r"^(.+?):(\d+):\d+: warning unused-declaration: (\w+) (\S+) ")


def ran(*arguments):
    done = subprocess.run([PHROST, *arguments], capture_output=True, text=True)
    return (done.stdout + "\n" + done.stderr).splitlines()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root")
    args = parser.parse_args()
    root = os.path.abspath(args.root)

    print("asking the report", flush=True)
    report = {
        (found.group(1), int(found.group(2)))
        for line in ran("unused", root)
        if (found := REPORTED.match(line.strip()))
    }
    print("asking the check", flush=True)
    check = {
        (found.group(1), int(found.group(2)))
        for line in ran("check", root)
        if (found := CHECKED.match(line.strip()))
    }
    if not check:
        print("the project does not switch `unused-declaration` on, so there is nothing to compare")
        return 0

    only_report = sorted(report - check)
    only_check = sorted(check - report)
    both = len(report & check)
    print()
    print(f"two reports of what nothing reaches  {both} agreed, "
          f"{len(only_report)} only in the report, {len(only_check)} only in the check")
    for where, line in only_report[:15]:
        print(f"    report only: {where}:{line}")
    for where, line in only_check[:15]:
        print(f"    check only : {where}:{line}")
    return 1 if only_report or only_check else 0


if __name__ == "__main__":
    sys.exit(main())
