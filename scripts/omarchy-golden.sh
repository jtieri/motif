#!/bin/sh
# Regenerates the expected output of the Omarchy golden tests by running
# Omarchy's own scripts over every fixture. Needs bash and gawk.
set -eu

cd "$(dirname "$0")/../crates/motif-core/tests/fixtures/omarchy"

# The resolver trims values with [:space:] and the converter matches with
# awk, and both depend on the locale.
export LC_ALL=C.UTF-8

for colors in themes/*/colors.toml cases/*/colors.toml; do
    dir=$(dirname "$colors")
    bash tools/omarchy-theme-color --file "$colors" --all >"$dir/expected.tsv" 2>/dev/null
done

# The converter writes colors.toml into the theme folder, so it runs on a
# copy. A case it writes nothing for has no expected files.
tmp=$(mktemp -d)
trap 'rm -r "$tmp"' EXIT
for alacritty in alacritty/*/alacritty.toml; do
    dir=$(dirname "$alacritty")
    theme=$(mktemp -d "$tmp/theme.XXXXXX")
    cp "$alacritty" "$theme/"
    if [ -e "$dir/light.mode" ]; then
        cp "$dir/light.mode" "$theme/"
    fi
    rm -f "$dir/expected.colors.toml" "$dir/expected.tsv"
    bash tools/omarchy-theme-colors-from-alacritty "$theme" 2>/dev/null
    if [ -e "$theme/colors.toml" ]; then
        cp "$theme/colors.toml" "$dir/expected.colors.toml"
        bash tools/omarchy-theme-color --file "$theme/colors.toml" --all >"$dir/expected.tsv" 2>/dev/null
    fi
done
