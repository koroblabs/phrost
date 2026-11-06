#!/bin/sh
# Packs one platform's release archive: the binary, the project's own licenses, and the licenses of
# everything in the binary that is not phrost's.
#
#   sh tools/dist/pack.sh <target> <path to phrost> <THIRD-PARTY-LICENSES.md> <out dir>
#
# writes <out dir>/phrost-<target>.tar.gz and its .sha256. The archive holds one directory,
# phrost-<target>/, and in it:
# - phrost, README.md, CHANGELOG.md, LICENSE-MIT, LICENSE-APACHE and NOTICE;
# - THIRD-PARTY-LICENSES.md, every crate linked in, by license (cargo-about, `about.toml`);
# - licenses/php-prelude/, the provenance and notices of the PHP stubs embedded in the binary.
set -eu
target=$1
binary=$2
third_party=$3
out=$4

name="phrost-$target"
stage="$out/$name"
rm -rf "$stage"
mkdir -p "$stage/licenses/php-prelude"
cp "$binary" "$stage/phrost"
cp README.md CHANGELOG.md LICENSE-MIT LICENSE-APACHE NOTICE "$stage/"
cp "$third_party" "$stage/THIRD-PARTY-LICENSES.md"
cp third_party/php-prelude/README.md third_party/php-prelude/*.NOTICE "$stage/licenses/php-prelude/"
# macOS tar would add extended attributes as `._` files.
COPYFILE_DISABLE=1 tar -C "$out" -czf "$out/$name.tar.gz" "$name"
(cd "$out" && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
rm -rf "$stage"
echo "$out/$name.tar.gz"
