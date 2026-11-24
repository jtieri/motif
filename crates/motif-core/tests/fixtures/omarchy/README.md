# Omarchy fixtures

`themes/` holds the `colors.toml` of each of Omarchy's 23 built-in themes,
`tools/omarchy-theme-color` is Omarchy's palette resolver and
`tools/omarchy-theme-colors-from-alacritty` is the script that generates a
`colors.toml` for a legacy theme that has only an `alacritty.toml`. All are
copied unchanged from [basecamp/omarchy](https://github.com/basecamp/omarchy) at
commit `4574aa8bb5350141542166b9d09206cc7413702c`, and are MIT licensed as
stated in `LICENSE`. No built-in theme has a `light.mode` file at that commit.

`cases/` holds small `colors.toml` files written for motif's tests, covering
the parser's quirks and each step of the resolver's cascade.

`alacritty/` holds small `alacritty.toml` files written for motif's tests,
covering each branch of the converter, with a `light.mode` where a case needs
one.

Each `expected.tsv` in `themes/` and `cases/` is what
`omarchy-theme-color --file colors.toml --all` prints for the `colors.toml`
beside it. In `alacritty/`, `expected.colors.toml` is what the converter
writes for the `alacritty.toml` beside it and `expected.tsv` is what the
resolver prints for that file; a case the converter writes nothing for has
neither. `scripts/omarchy-golden.sh` regenerates them all.
