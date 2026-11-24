//! Theme sources, the neutral theme model and its conversion into palettes.

mod color;
pub mod file;
pub mod omarchy;
pub mod source;
mod theme;

pub use color::{Color, ParseColorError};
pub use source::{LoadError, Loaded, Source, Warning};
pub use theme::{
    Background, Foreground, Mode, NamedColor, Provenance, Selection, SourceKind, Theme,
};
