//! Parses arbitrary text as an Omarchy color value.

#![no_main]

use libfuzzer_sys::fuzz_target;
use motif_core::omarchy::parse_color;

fuzz_target!(|value: &str| {
    if let Ok(color) = parse_color(value) {
        assert_eq!(parse_color(&color.to_string()), Ok(color));
    }
});
