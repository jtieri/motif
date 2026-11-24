//! Property tests: any theme a source can produce maps onto a valid palette.

use motif_core::{
    Background, Color, Foreground, Mode, NamedColor, Options, Palette, Provenance, Selection,
    Semantic, SourceKind, Theme,
};
use proptest::prelude::*;

fn color() -> impl Strategy<Value = Color> {
    any::<(u8, u8, u8)>().prop_map(|(r, g, b)| Color::new(r, g, b))
}

fn hue() -> impl Strategy<Value = NamedColor> {
    (
        prop::sample::select(vec![
            "red", "yellow", "green", "cyan", "blue", "magenta", "base0E",
        ]),
        color(),
    )
        .prop_map(|(name, color)| NamedColor {
            name: name.to_owned(),
            color,
        })
}

prop_compose! {
    fn theme()(
        dark in any::<bool>(),
        ansi in prop::array::uniform16(color()),
        background in (color(), prop::option::of(color()), prop::option::of(color()), prop::option::of(color())),
        foreground in (color(), prop::option::of(color()), prop::option::of(color())),
        accent in prop::option::of(color()),
        muted in prop::option::of(color()),
        cursor in prop::option::of(color()),
        hues in prop::collection::vec(hue(), 0..12),
    ) -> Theme {
        Theme {
            name: "Generated".into(),
            mode: if dark { Mode::Dark } else { Mode::Light },
            ansi,
            background: Background {
                base: background.0,
                dark: background.1,
                darker: background.2,
                lighter: background.3,
            },
            foreground: Foreground {
                base: foreground.0,
                dark: foreground.1,
                light: foreground.2,
                bright: None,
            },
            accent,
            selection: Selection::default(),
            muted,
            cursor,
            hues,
            extras: Vec::new(),
            provenance: Provenance {
                kind: SourceKind::Base16,
                url: None,
                commit: None,
                path: None,
            },
        }
    }
}

proptest! {
    #[test]
    fn every_theme_maps_to_a_valid_palette(
        theme in theme(),
        semantic in prop::sample::select(vec![Semantic::Literal, Semantic::Tinted, Semantic::Fixed]),
    ) {
        let palette = Palette::from_theme(&theme, &Options { semantic, ..Options::default() }).unwrap();
        let report = palette.validate();
        prop_assert!(report.problems.is_empty(), "{:?}", report.problems);
        prop_assert_eq!(palette.palette.len(), 63);

        // Identity colors are the theme's own.
        prop_assert_eq!(palette.palette.get("surface"), Some(theme.background.base));
        prop_assert_eq!(palette.palette.get("on_surface"), Some(theme.foreground.base));
        if let Some(accent) = theme.accent {
            prop_assert_eq!(palette.palette.get("primary"), Some(accent));
            prop_assert_eq!(palette.meta.source_color, accent);
        }
        for (i, color) in theme.ansi.iter().enumerate() {
            prop_assert_eq!(palette.ansi.get(&format!("color{i}")), Some(*color));
        }

        // The accent picker offers exactly the named colors.
        let names: Vec<&str> = palette.colors.iter().map(|(name, _)| name).collect();
        prop_assert_eq!(&palette.meta.accents.available, &names);

        // The JSON reads back as the palette.
        let json = palette.to_json();
        let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(serde_json::to_value(&palette).unwrap(), doc);
    }
}
