//! Converts arbitrary bytes as an Omarchy `alacritty.toml` and builds a theme
//! from the result.

#![no_main]

use libfuzzer_sys::fuzz_target;
use motif_core::omarchy::{ColorsToml, colors_from_alacritty};
use motif_core::{Provenance, SourceKind};

fuzz_target!(|data: &[u8]| {
    let Ok(colors) = colors_from_alacritty(data) else {
        return;
    };
    let colors = ColorsToml::parse(&colors);
    // Only a tab in a header or key can put anything but a color in a value.
    let clean = !data.contains(&b'\t');
    if clean {
        assert!(colors.skipped().is_empty());
        assert_eq!(colors.entries().len(), 20);
    }
    for light_mode_file in [false, true] {
        let provenance = Provenance::local(SourceKind::OmarchyAlacritty, "fuzz");
        let theme = colors.resolve(light_mode_file).to_theme(String::new(), provenance);
        assert!(theme.is_ok() || !clean);
    }
});
