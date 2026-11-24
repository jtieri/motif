# How a theme becomes a palette

motif reads a theme into a neutral model (`Theme` in `motif-core`) and then
builds a dotfiles palette from that model alone, so every source format maps
the same way. The palette format is described in the dotfiles'
`home/dot_config/matugen/palettes/_schema.md`. This page lists where every key
of a palette comes from.

The mapping automates the schema's recipe for a curated palette:

- **Identity colors** are the theme's own colors, unchanged. Anything a user
  sees as "the theme" (backgrounds, text, the accent, the status colors) is a
  color its author chose.
- **Derived tones** are the darker and lighter partners Material You pairs
  with each accent, which a theme does not publish. They come from
  [material-colors](https://github.com/Aiving/material-colors), the library
  matugen is built on, at the version matugen 4.2 uses: each accent seeds a
  tonal spot scheme at standard contrast, exactly as
  `matugen color hex <accent>` does, and its primary tones are read back.
  The text tones on the accents are the one adjustment, described under
  "Text on the accents". A
  golden test (`scripts/harvest-golden.sh`) checks the tones against
  matugen's own output for 25 colors in both modes.

The model's names follow Omarchy's: the background tiers are `base`, `dark`,
`darker` and `lighter`, and the foreground tiers `base`, `dark`, `light` and
`bright`.

## `meta`

| Key | Value | Why |
|-----|-------|-----|
| `name` | `--name`, else the theme's name | Omarchy themes are named after their folder as Omarchy names them, so `omarchy-tokyo-night-theme` becomes `Tokyo Night`. |
| `slug` | `--slug`, else the name lowercased with every run of other characters turned into one hyphen | A slug names a file and is passed to shell scripts, so it is limited to `a-z`, `0-9` and `-`, starts with a letter or digit, is at most 64 characters and is never `wallpaper-derived`. |
| `mode` | The theme's mode | Each source decides it; Omarchy's rules are ported exactly. |
| `source_color` | The theme's accent | theme-apply seeds matugen with it. It is the same color as `primary`. |
| `scheme_names` | The fallbacks `derive-palette.py` uses | An imported theme has no Neovim, Zed or GTK theme of its own yet, so it uses the ones rendered from the palette: `dotfiles-base16`, `Dotfiles`, `adw-gtk3-dark` or `adw-gtk3`, `Papirus-Dark` or `Papirus-Light`, `capitaine-cursors` or `capitaine-cursors-light`, and `Catppuccin Mocha` for JetBrains, as derived palettes have. `papirus_folder_color` is the accent's hex, which recolors a custom Papirus folder. A later release names a theme's official ports when it knows them. |
| `accents.available` | The keys of `colors` | The accent picker offers the theme's own hues by name, as it does for curated palettes. |
| `accents.gtk_theme` | `adw-gtk3-dark` or `adw-gtk3` | The same theme for every accent; its colors come from the rendered overrides. |
| `accents.papirus_folder_color` | `{accent_hex}` | No Papirus preset matches an arbitrary color. |
| `source` | `kind`, `url` and `commit` of the theme | Records where a palette came from. `kind` is the format (`omarchy-colors`); `url` and `commit` are left out for a local folder. The folder's path is not recorded: palettes are synced to every machine, where a path from one of them means nothing. |

`meta.wallpaper` is not written yet.

## `ansi`

| Key | Value |
|-----|-------|
| `color0` to `color15` | The theme's 16 terminal colors, unchanged. |
| `cursor` | The theme's cursor color, else its foreground. |

## `colors`

The theme's hues under the theme's own names and in its order, such as `red`,
`yellow`, `orange`, `green`, `cyan`, `blue`, `magenta`, `purple` and `brown`
for an Omarchy theme. Omarchy's `orange`, `purple` and `brown` appear only
when the theme sets them, since Omarchy otherwise copies another hue into
them. Colors a theme gives for one application, such as a window border, are
left out.

A theme that names no hues gets ANSI colors 1 to 6 as `red`, `green`,
`yellow`, `blue`, `magenta` and `cyan`.

## `palette`

### Surfaces and text

| Key | Value |
|-----|-------|
| `background`, `surface` | Background `base`. |
| `surface_dim`, `surface_container_low` | Background `dark`. |
| `surface_container_lowest` | Background `darker`. |
| `surface_container`, `surface_variant`, `outline_variant` | Background `lighter`. |
| `surface_container_high` | Background `lighter` mixed 12% toward the foreground. |
| `surface_container_highest`, `surface_bright` | Background `lighter` mixed 24% toward the foreground. |
| `on_surface`, `on_background`, `inverse_surface` | Foreground `base`. |
| `on_surface_variant` | Foreground `light`. |
| `on_surface_dim` | Foreground `dark`. |
| `outline` | The theme's muted color. |
| `inverse_on_surface` | Background `dark`. |
| `scrim`, `shadow` | `#000000`, as in every palette. |

The raised surfaces continue the theme's own step from `base` to `lighter`
toward its text, so they keep rising in elevation in the theme's hue. The
12% and 24% steps are close to the steps between Catppuccin Mocha's
`surface0`, `surface1` and `surface2`, and `surface_bright` equals
`surface_container_highest` as it does in the curated Catppuccin palette. In a
light theme the foreground is dark, so raised surfaces get darker, as Material
You's light containers do.

### Accents

| Key | Value |
|-----|-------|
| `primary`, `surface_tint`, `accent` | The theme's accent. |
| `secondary`, `tertiary` | Two of the theme's hues, chosen as below. |
| `on_primary` | A readable tone for the accent, as below. |
| `primary_container`, `on_primary_container`, `primary_fixed`, `primary_fixed_dim`, `on_primary_fixed`, `on_primary_fixed_variant`, `inverse_primary` | matugen's tones for the accent. |
| `on_secondary`, `on_tertiary` | A readable tone for that hue, as below. |
| The other six keys for `secondary` and for `tertiary` | matugen's tones for that hue, read from its `primary` keys. |

`secondary` and `tertiary` are distinct in hue from the primary and from each
other, as Catppuccin pairs mauve with blue and teal. Hues are compared in HCT,
matugen's color space, and two colors count as distinct when their hues are
at least 30 degrees apart. Each role takes the hue nearest to the primary
that is distinct from every role chosen before it, which picks neighbors on
the color wheel rather than opposites. Candidates are tried in groups, moving
to the next only when a group is empty:

1. the theme's hues with an HCT chroma of at least 16 (not greys), other than
   the accent and the three status colors, that have at least 3:1 contrast
   against the background;
2. the same without the contrast requirement;
3. any of the theme's hues other than the accent.

If no candidate in the group is distinct, the one farthest in hue from the
roles already chosen is taken. Ties go to the hue the theme lists first. A
theme whose hues are all the accent uses the accent for both roles.

### Text on the accents

matugen chooses `on_primary` to read on the primary it re-tones (tone 80 in a
dark theme, tone 40 in a light one), but the palette keeps the theme's own
color in the role, which can be much darker or lighter. matugen's text tone then often
fails to read on it. So `on_primary`, `on_secondary`, `on_tertiary` and
`on_error` start from matugen's tone, and when it has less than 4.5:1 contrast
against the role's color, they move one tone at a time along the same HCT
tonal palette (the seed's hue and chroma, so the text stays in the role's
color family) toward whichever end, tone 0 or tone 100, contrasts more with
the role's color, stopping at the first tone that reaches 4.5:1. Black or
white always reaches at least 4.58:1, so one is always found. Every other
tone is matugen's, unchanged, so the containers and fixed tones match the
curated recipe exactly.

### Status colors

| Key | Value |
|-----|-------|
| `error` | The theme's red, as chosen by `--semantic`. |
| `on_error` | A readable tone for `error`, as below. |
| `error_container`, `on_error_container` | matugen's tones for `error`. |
| `warning` | The theme's yellow, as chosen by `--semantic`. |
| `success` | The theme's green, as chosen by `--semantic`. |

The theme's red, yellow and green are its hues of those names, or else ANSI
colors 1, 3 and 2. `--semantic` works as it does for `theme-generate`, using
the same functions as `derive-palette.py`:

- `literal`, the default for imports, keeps the theme's colors, even where a
  theme's "red" is not red (Lumon's is blue). The theme stays as its author
  made it.
- `tinted` turns each color to a canonical hue (5, 45 and 140 degrees in HLS
  for a dark theme; 5, 42 and 140 for a light one) and keeps its lightness and
  saturation.
- `fixed` uses canonical colors for the mode whatever the theme.

### The accent set

`accent_1` to `accent_10` take, in order:

1. the theme's hues as listed in `colors`, skipping any color already taken;
2. the bright ANSI colors 9 to 14 that differ from every color taken;
3. variants of the colors taken, in the order they were taken, with HLS
   lightness raised by 0.1 at a time (lowered for a light theme), skipping
   any variant that repeats a color.

Only a palette whose every color is black or white can run out of variants;
the colors taken are then repeated.

## When a source leaves a color out

Omarchy fills every color through its own fallbacks, so an Omarchy theme
always has all of them. Formats that motif will read later may not, so the
model allows a missing tier, and the mapping fills it the way
`derive-palette.py`'s `build_base16` fills the slot it maps to:

| Missing | Filled with |
|---------|-------------|
| Background `dark` | The background mixed 3% toward the foreground. |
| Background `darker` | The background mixed 30% toward black (white for a light theme). |
| Background `lighter` | The background mixed 5% toward the foreground. |
| Foreground `light` | The foreground mixed 28% toward the background. |
| Foreground `dark` | The foreground mixed 45% toward the background. |
| Muted | The foreground mixed 55% toward the background. |
| Accent | The most saturated of ANSI colors 1 to 6 and 9 to 14, the first of equals. |
| Cursor | The foreground. |

Mixing is per channel in sRGB and rounds half to even, as Python does, so the
result is the same color `derive-palette.py` would produce.

## Validation

`motif_core::validate` checks a palette file, curated or imported. A palette
is unusable when a section or key is missing or a value is not a `#rrggbb`
color, when the slug is invalid or the mode is not `dark` or `light`, or when
an accent it offers names no color. Hex digits may be in either case, since
matugen accepts both and the dotfiles' wallpaper-derived palettes can contain
uppercase ones; motif always writes lowercase.

Contrast is measured with WCAG 2's relative luminance. Text roles on their
backgrounds should reach 4.5:1. Accents, status colors and outlines on the
surface, and the text roles meant for low emphasis (`on_surface_dim` and
`on_*_fixed_variant`), should reach 3:1, the level WCAG asks of large text and
of the parts of a control that show its state. A pair below its level is a
warning, not an error: an imported theme keeps the colors its author chose,
and `motif import` reports how many pairs fall short (`--verbose` lists
them) so the user can decide.
