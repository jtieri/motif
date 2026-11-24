# Omarchy fixtures

`themes/` holds the `colors.toml` of each of Omarchy's 23 built-in themes, and
`tools/omarchy-theme-color` is Omarchy's palette resolver. Both are copied
unchanged from [basecamp/omarchy](https://github.com/basecamp/omarchy) at
commit `4574aa8bb5350141542166b9d09206cc7413702c`, and are MIT licensed as
stated in `LICENSE`. No built-in theme has a `light.mode` file at that commit.

`cases/` holds small `colors.toml` files written for motif's tests, covering
the parser's quirks and each step of the resolver's cascade.

Each `expected.tsv` is what `omarchy-theme-color --file colors.toml --all`
prints for the `colors.toml` beside it. `scripts/omarchy-golden.sh`
regenerates them all.
