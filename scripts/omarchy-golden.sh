#!/bin/sh
# Regenerates the expected output of the Omarchy golden tests by running
# Omarchy's own resolver over every fixture. Needs bash and gawk.
set -eu

cd "$(dirname "$0")/../crates/motif-core/tests/fixtures/omarchy"

# The resolver trims values with [:space:], which depends on the locale.
export LC_ALL=C.UTF-8

for colors in themes/*/colors.toml cases/*/colors.toml; do
    dir=$(dirname "$colors")
    bash tools/omarchy-theme-color --file "$colors" --all >"$dir/expected.tsv" 2>/dev/null
done
