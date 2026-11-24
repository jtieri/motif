//! Theme sources, the neutral theme model and its conversion into palettes.

mod color;
pub mod contrast;
pub mod file;
pub mod omarchy;
pub mod palette;
pub mod slug;
pub mod source;
pub mod store;
mod theme;
pub mod validate;

pub use color::{Color, ParseColorError};
pub use palette::{Options, Palette, Semantic};
pub use slug::{Slug, SlugError, SlugErrorKind};
pub use source::{LoadError, Loaded, Source, Warning};
pub use theme::{
    Background, Foreground, Mode, NamedColor, Provenance, Selection, SourceKind, Theme,
};
