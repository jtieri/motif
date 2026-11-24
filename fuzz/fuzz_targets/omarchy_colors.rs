//! Reads arbitrary bytes as an Omarchy `colors.toml` and builds a theme.

#![no_main]

use libfuzzer_sys::fuzz_target;
use motif_core::omarchy::ColorsToml;
use motif_core::{Provenance, SourceKind};

fuzz_target!(|data: &[u8]| {
    let colors = ColorsToml::parse(data);
    for light_mode_file in [false, true] {
        let resolved = colors.resolve(light_mode_file);
        let provenance = Provenance::local(SourceKind::OmarchyColors, "fuzz");
        if let Ok(theme) = resolved.to_theme(String::new(), provenance) {
            for hue in theme.hues.iter().chain(&theme.extras) {
                assert!(resolved.get(&hue.name).is_some());
            }
        }
    }
});
