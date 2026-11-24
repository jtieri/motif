#!/bin/sh
# Regenerates the expected tones of the harvest golden test by asking matugen
# for the tonal scheme of each seed color. Needs matugen and jq.
#
# matugen reads a config file even for `color hex`, so a minimal one is used
# to keep a scheme type or contrast set in the user's config out of the result.
set -eu

cd "$(dirname "$0")/../crates/motif-core/tests/fixtures/harvest"

config=$(mktemp)
trap 'rm -f "$config"' EXIT
printf '[config]\n\n[templates]\n' >"$config"

keys='on_primary primary_container on_primary_container primary_fixed
primary_fixed_dim on_primary_fixed on_primary_fixed_variant inverse_primary'

{
    printf '# matugen %s, scheme-tonal-spot, contrast 0\n' "$(matugen --version | cut -d' ' -f2)"
    printf '# seed\tmode\trole\ttone\n'
    grep -v '^# ' seeds.txt | while read -r seed; do
        [ -n "$seed" ] || continue
        json=$(matugen --config "$config" color hex "$seed" --json hex --dry-run)
        for mode in dark light; do
            for key in $keys; do
                tone=$(printf '%s' "$json" | jq -r --arg k "$key" --arg m "$mode" '.colors[$k][$m].color')
                printf '%s\t%s\t%s\t%s\n' "$seed" "$mode" "$key" "$tone"
            done
        done
    done
} >expected.tsv
