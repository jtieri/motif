# motif

motif imports color themes from other ecosystems into a matugen-based desktop
palette, and lets you browse and preview them before you import anything.

It reads [Omarchy](https://github.com/basecamp/omarchy) themes (`colors.toml`
and the older `alacritty.toml`) and [Base16 and Base24](https://github.com/tinted-theming/schemes)
schemes, and writes a palette file plus, if you want them, the theme's
wallpapers.

Theme content is treated as data. motif never runs anything a theme ships.

motif is early in development.

## Layout

| Crate | Purpose |
|-------|---------|
| `motif-core` | Theme sources, the neutral theme model, conversion and validation |
| `motif-render` | Preview images drawn as a mockup of the desktop |
| `motif` | The command-line program |

## Usage

```sh
motif import ~/src/omarchy-nord-theme
```

reads the theme in that folder and saves it as a palette in
`~/.config/matugen/palettes`. `motif import --help` lists the options, and
[docs/mapping.md](docs/mapping.md) explains where each color of the palette
comes from.

## Building

```sh
cargo build --release
```

`scripts/check.sh` runs the same checks as CI: formatting, clippy, tests and
`cargo deny`.

## License

MIT
