#!/usr/bin/env python3
"""What the reprint did to a project's phpcs errors, grouped by sniff.

Reads the two CSV reports `tools/drupal-style.sh` produced and prints the errors that appear only
in the reprinted tree -- the formatter's -- and the ones it removed. Counted per file per sniff, not
by line, because a reprint moves lines around and a line number says nothing across it.

  tools/drupal-style-diff.py <work directory> [-v]

`-v` prints each introduced error with the three lines above it, which is what a fix starts from.
"""

import collections
import csv
import pathlib
import sys


def load(work: pathlib.Path, tree: str) -> dict:
    rows: dict = collections.defaultdict(list)
    report = work / f"{tree}.csv"
    if not report.exists():
        raise SystemExit(f"no report at {report}")
    with report.open() as handle:
        reader = csv.DictReader(handle)
        # A phpcs that died writes its error where the report goes, which reads as no findings.
        if reader.fieldnames is None or "File" not in reader.fieldnames:
            first = ",".join(reader.fieldnames or [])[:200]
            raise SystemExit(f"{report} is not a phpcs CSV report; it begins: {first}")
        for row in reader:
            path = row["File"].split(f"/{tree}/", 1)[-1]
            rows[(path, row["Source"])].append((int(row["Line"]), row["Message"]))
    return rows


def main() -> None:
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    work = pathlib.Path(sys.argv[1])
    before, after = load(work, "before"), load(work, "after")

    added: collections.Counter = collections.Counter()
    removed: collections.Counter = collections.Counter()
    sites = []
    for key in sorted(set(before) | set(after)):
        delta = len(after.get(key, [])) - len(before.get(key, []))
        if delta > 0:
            added[key[1]] += delta
            for line, message in after[key][-delta:]:
                sites.append((key[1], key[0], line, message))
        elif delta < 0:
            removed[key[1]] += -delta

    written = sum(len(v) for v in before.values())
    reprinted = sum(len(v) for v in after.values())
    print(f"errors as written {written}, after reprinting {reprinted}")
    print(f"introduced {sum(added.values())}, removed {sum(removed.values())}")
    for source, count in added.most_common():
        print(f"  +{count:>4}  {source}")
    for source, count in removed.most_common():
        print(f"  -{count:>4}  {source}")

    if "-v" not in sys.argv:
        return
    for source, path, line, message in sites:
        print(f"--- {source} {path}:{line}")
        print(f"    {message}")
        text = (work / "after" / path).read_text(errors="replace").splitlines()
        for number in range(max(0, line - 3), min(len(text), line + 1)):
            print(f"   {number + 1:>5} {text[number][:140]}")


if __name__ == "__main__":
    main()
