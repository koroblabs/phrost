#!/bin/sh
# Whether a built phrost runs on this machine: it says its version, and finds an undefined method
# with its embedded stubs installed.
#
#   sh tools/dist/smoke.sh <path to phrost>
set -eu
phrost=$1
"$phrost" --version
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/src"
printf '<?php\n\nclass Box\n{\n    public function open(): void {}\n}\n\n(new Box())->close();\nstrlen("stubs are in");\n' > "$work/src/Box.php"
# `check` exits 1 when it reports something, which is what this asks of it.
set +e
"$phrost" check "$work" > "$work/out.txt" 2>&1
code=$?
set -e
cat "$work/out.txt"
if [ "$code" != 1 ] || ! grep -q 'undefined-method' "$work/out.txt" || grep -q 'undefined-function' "$work/out.txt"; then
    echo "smoke: expected exit 1 and exactly an undefined method, got exit $code" >&2
    exit 1
fi
