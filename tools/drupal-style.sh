#!/bin/zsh
#
# Does the formatter break Drupal's house style?
#
# The one instrument that does not reimplement a sniff: it reprints every user file of a project,
# runs the project's own phpcs over both trees, and reports the errors the reprint *introduced*,
# grouped by sniff. A formatter cannot be argued with about a coding standard -- the standard has an
# executable -- and the delta is what matters, because a project's existing findings are not ours.
#
#   tools/drupal-style.sh ~/Projects/example-site/docroot/modules/custom
#
# Needs a phpcs with the Drupal standard installed; a Drupal project vendors one at
# `vendor/bin/phpcs`. Set PHPCS to point elsewhere.
set -e

root=${1:?usage: drupal-style.sh <directory of user code> [phpcs standard]}
standard=${2:-Drupal}
phpcs=${PHPCS:-$(cd "$root/../../.." 2>/dev/null && pwd)/vendor/bin/phpcs}
work=${TMPDIR:-/tmp}/phrost-drupal-style

if [[ ! -x $phpcs ]]; then
  print -u2 "no phpcs at $phpcs -- set PHPCS to one with the Drupal standard installed"
  exit 1
fi

rm -rf "$work"
mkdir -p "$work"
# No --style: the command resolves it the way the server does, from the project above this
# directory and the frameworks it detects there. Naming it here instead would let the two
# drift, which is what this instrument exists to notice.
cargo run --release -q -p phrost-cli -- reprint "$root" "$work"
# PHP's default 128 MB is not enough for Drupal core's own modules: phpcs dies writing a fatal error
# where the report should be.
for tree in before after; do
  "$phpcs" -d memory_limit=4G --standard="$standard" --extensions=php,module,inc,install,theme \
    --report=csv --parallel=8 -n --no-cache "$work/$tree" > "$work/$tree.csv" 2>/dev/null || true
done
python3 "${0:a:h}/drupal-style-diff.py" "$work" "${@:3}"
