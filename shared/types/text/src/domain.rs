//! Text domain marker implementing [`Domain`].

use reovim_domain::Domain;

use crate::{TextEdit, TextPosition};

/// Text content domain.
///
/// Marker type that implements [`Domain`] for text editing, binding
/// the abstract domain types to concrete text representations:
///
/// - `Position` = [`TextPosition`] (line:column)
/// - `Edit` = [`TextEdit`] (insert/delete with text)
/// - `Content` = [`String`] (decoded UTF-8 text)
pub struct Text;

impl Domain for Text {
    type Position = TextPosition;
    type Edit = TextEdit;
    type Content = String;
}

#[cfg(test)]
#[path = "domain_tests.rs"]
mod tests;
