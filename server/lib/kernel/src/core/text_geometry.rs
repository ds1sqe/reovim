//! Read-only text geometry trait — re-exported from `reovim-types-text`.

#[allow(unused_imports)]
pub use reovim_types_text::{SimpleText, TextGeometry};

#[cfg(test)]
#[path = "text_geometry_tests.rs"]
mod tests;
