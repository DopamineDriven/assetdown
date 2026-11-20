#!/usr/bin/env bash
#
# Sets one package's version in every file that states it, so that they all move together.
#
#   ./version.sh <package> <version>
#   ./version.sh pdfdown 0.17.0
#
#   <package>  pdfdown, tiffdown, jpegdown, rtfdown, xlsxdown, docxdown, pptxdown, audiodown, videodown, or mimedown
#   <version>  a semver version: 1.2.3, or 1.2.3-next.1 for a prerelease
#
# Every file is read before any is written, so a file that is missing or states no version
# leaves all of them as they were. A version lower than one already stated is refused.
# Nothing is committed: CI publishes a package from a commit whose subject is exactly
# <package>@<version>.
#
# Needs jq (for the package.json files) and awk (for the Cargo.toml files).

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
semver='^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$'

usage() {
  cat >&2 <<'EOF'
usage: ./version.sh <package> <version>

  <package>  pdfdown, tiffdown, jpegdown, rtfdown, xlsxdown, docxdown, pptxdown, audiodown, videodown, or mimedown
  <version>  a semver version: 1.2.3, or 1.2.3-next.1 for a prerelease

example: ./version.sh pdfdown 0.17.0
EOF
  exit 2
}

fail() {
  echo "version.sh: $*" >&2
  exit 1
}

# For a Cargo.toml: finds `version = "..."` in the [package] table. With mode=get it prints the
# version. With mode=set it prints the file with `version` in its place, leaving the line's
# spacing and anything after the value as they are.
cargo_version='
/^[ \t]*\[/ { in_package = ($0 ~ /^[ \t]*\[package\]/) }
!found && in_package && match($0, /^version[ \t]*=[ \t]*"/) {
  found = 1
  rest = substr($0, RLENGTH + 1)
  if (mode == "get") {
    sub(/".*$/, "", rest)
    print rest
    exit
  }
  sub(/^[^"]*/, "", rest)
  print substr($0, 1, RLENGTH) version rest
  next
}
mode == "set" { print }
'

# Prints the version a file states: the top-level "version" of a package.json (not the
# "version" script further down), or `version` in the [package] table of a Cargo.toml.
# Prints nothing when the file states none.
stated_version() {
  case "$1" in
    *.json) jq -r '.version | strings' "$1" ;;
    *.toml) awk -v mode=get "$cargo_version" "$1" ;;
  esac
}

# Prints a file's contents with its version set to $2.
with_version() {
  case "$1" in
    *.json) jq --arg version "$2" '.version = $version' "$1" ;;
    *.toml) awk -v mode=set -v version="$2" "$cargo_version" "$1" ;;
  esac
}

# Succeeds when the X.Y.Z of $1 is lower than the X.Y.Z of $2. Prerelease parts are not compared.
is_lower() {
  local a b i
  IFS=. read -r -a a <<<"${1%%[-+]*}"
  IFS=. read -r -a b <<<"${2%%[-+]*}"
  for i in 0 1 2; do
    ((10#${a[i]} < 10#${b[i]})) && return 0
    ((10#${a[i]} > 10#${b[i]})) && return 1
  done
  return 1
}

[ $# -eq 2 ] || usage
package="$1"
version="$2"

case "$package" in
  pdfdown)
    files=(
      crates/pdfdown-node/package.json
      crates/pdfdown-node/ocr/package.json
      crates/pdfdown-node/Cargo.toml
      crates/pdfdown/Cargo.toml
      crates/pdfdown-ocr/Cargo.toml
      crates/pdfdown-meta/Cargo.toml
    )
    ;;
  tiffdown | jpegdown)
    files=(
      "crates/$package-node/package.json"
      "crates/$package-node/ocr/package.json"
      "crates/$package-node/Cargo.toml"
      "crates/$package/Cargo.toml"
    )
    ;;
  rtfdown | xlsxdown | docxdown | pptxdown | audiodown | videodown | mimedown)
    files=(
      "crates/$package-node/package.json"
      "crates/$package-node/Cargo.toml"
      "crates/$package/Cargo.toml"
    )
    ;;
  *)
    echo "version.sh: \"$package\" is not a package this script knows" >&2
    usage
    ;;
esac

[[ "$version" =~ $semver ]] || fail "\"$version\" is not a version like 1.2.3 or 1.2.3-next.1"
command -v jq >/dev/null || fail "jq is needed to edit the package.json files"

# Read every file before writing any.
before=()
for file in "${files[@]}"; do
  [ -f "$root/$file" ] || fail "$file does not exist"
  stated="$(stated_version "$root/$file")" || fail "$file could not be read"
  [ -n "$stated" ] || fail "$file states no version"
  if [[ "$stated" =~ $semver ]] && is_lower "$version" "$stated"; then
    fail "$file is at $stated, which is higher than $version"
  fi
  before+=("$stated")
done

scratch="$(mktemp)"
trap 'rm -f "$scratch"' EXIT

changed=0
for i in "${!files[@]}"; do
  file="${files[$i]}"
  stated="${before[$i]}"
  if [ "$stated" = "$version" ]; then
    printf '  %-38s %s (already)\n' "$file" "$stated"
    continue
  fi
  with_version "$root/$file" "$version" >"$scratch"
  # Written over the file in place, which keeps its permissions.
  cat "$scratch" >"$root/$file"
  [ "$(stated_version "$root/$file")" = "$version" ] || fail "$file did not take the version"
  printf '  %-38s %s -> %s\n' "$file" "$stated" "$version"
  changed=$((changed + 1))
done

if [ "$changed" -eq 0 ]; then
  echo "$package is already at $version in all ${#files[@]} files"
else
  echo "$package is at $version in all ${#files[@]} files ($changed changed)"
  echo "CI publishes it from a commit whose subject is exactly: $package@$version"
fi
