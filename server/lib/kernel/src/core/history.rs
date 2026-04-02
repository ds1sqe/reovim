//! Clipboard history ring — re-exported from `reovim-types-text`.

pub use reovim_types_text::HistoryRing;

#[cfg(test)]
#[path = "tests/history.rs"]
mod tests;
