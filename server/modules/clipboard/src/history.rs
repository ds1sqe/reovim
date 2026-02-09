//! Yank history ring buffer.
//!
//! Implements registers 0-9 as a push-down stack.

use reovim_kernel::api::v1::RegisterContent;

/// Ring buffer for yank history (registers 0-9).
///
/// When new content is pushed:
/// - New content becomes entry 0
/// - Previous entry 0 becomes entry 1
/// - Previous entry 1 becomes entry 2
/// - ... and so on up to capacity
/// - Oldest entry is dropped when full
///
/// # Capacity
///
/// Default capacity is 10 to support registers 0-9.
#[derive(Debug)]
pub struct HistoryRing {
    entries: Vec<RegisterContent>,
    capacity: usize,
}

impl HistoryRing {
    /// Create a new history ring with specified capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Push new content to the history.
    ///
    /// The new content becomes entry 0, shifting all existing entries down.
    pub fn push(&mut self, content: RegisterContent) {
        // Insert at the front
        self.entries.insert(0, content);

        // Trim to capacity
        if self.entries.len() > self.capacity {
            self.entries.truncate(self.capacity);
        }
    }

    /// Get an entry by index (0 = most recent).
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&RegisterContent> {
        self.entries.get(index)
    }

    /// Get all entries as a slice.
    #[must_use]
    pub fn all(&self) -> &[RegisterContent] {
        &self.entries
    }

    /// Get the current number of entries.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::len() is not const
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the history is empty.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty() is not const
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for HistoryRing {
    fn default() -> Self {
        Self::new(10) // Default: registers 0-9
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::YankType};

    fn content(text: &str) -> RegisterContent {
        RegisterContent::new(text.to_string(), YankType::Characterwise)
    }

    #[test]
    fn test_push_and_get() {
        let mut ring = HistoryRing::new(10);

        ring.push(content("first"));
        assert_eq!(ring.get(0).map(|c| c.text.as_str()), Some("first"));

        ring.push(content("second"));
        assert_eq!(ring.get(0).map(|c| c.text.as_str()), Some("second"));
        assert_eq!(ring.get(1).map(|c| c.text.as_str()), Some("first"));
    }

    #[test]
    fn test_capacity_limit() {
        let mut ring = HistoryRing::new(3);

        ring.push(content("a"));
        ring.push(content("b"));
        ring.push(content("c"));
        ring.push(content("d")); // Should push out "a"

        assert_eq!(ring.len(), 3);
        assert_eq!(ring.get(0).map(|c| c.text.as_str()), Some("d"));
        assert_eq!(ring.get(1).map(|c| c.text.as_str()), Some("c"));
        assert_eq!(ring.get(2).map(|c| c.text.as_str()), Some("b"));
        assert!(ring.get(3).is_none()); // "a" was dropped
    }

    #[test]
    fn test_empty() {
        let ring = HistoryRing::new(10);
        assert!(ring.is_empty());
        assert_eq!(ring.len(), 0);
        assert!(ring.get(0).is_none());
    }

    #[test]
    fn test_clear() {
        let mut ring = HistoryRing::new(10);
        ring.push(content("test"));
        assert!(!ring.is_empty());

        ring.clear();
        assert!(ring.is_empty());
    }

    #[test]
    fn test_default_capacity_is_ten() {
        let ring = HistoryRing::default();
        assert_eq!(ring.capacity(), 10);
        assert!(ring.is_empty());
    }

    #[test]
    fn test_capacity_returns_correct_value() {
        let ring = HistoryRing::new(5);
        assert_eq!(ring.capacity(), 5);
    }

    #[test]
    fn test_all_returns_entries_in_order() {
        let mut ring = HistoryRing::new(10);
        ring.push(content("first"));
        ring.push(content("second"));
        ring.push(content("third"));

        let all = ring.all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].text, "third");
        assert_eq!(all[1].text, "second");
        assert_eq!(all[2].text, "first");
    }

    #[test]
    fn test_all_empty_ring() {
        let ring = HistoryRing::new(10);
        assert!(ring.all().is_empty());
    }

    #[test]
    fn test_capacity_one() {
        let mut ring = HistoryRing::new(1);
        ring.push(content("first"));
        ring.push(content("second"));

        assert_eq!(ring.len(), 1);
        assert_eq!(ring.get(0).map(|c| c.text.as_str()), Some("second"));
        assert!(ring.get(1).is_none());
    }

    #[test]
    fn test_push_preserves_yank_type() {
        let mut ring = HistoryRing::new(10);
        ring.push(RegisterContent::new("line".to_string(), YankType::Linewise));

        let entry = ring.get(0).unwrap();
        assert_eq!(entry.yank_type, YankType::Linewise);
    }

    #[test]
    fn test_len_after_push_and_clear() {
        let mut ring = HistoryRing::new(10);
        assert_eq!(ring.len(), 0);

        ring.push(content("a"));
        assert_eq!(ring.len(), 1);

        ring.push(content("b"));
        assert_eq!(ring.len(), 2);

        ring.clear();
        assert_eq!(ring.len(), 0);
    }

    #[test]
    fn test_get_out_of_bounds() {
        let mut ring = HistoryRing::new(10);
        ring.push(content("only"));

        assert!(ring.get(0).is_some());
        assert!(ring.get(1).is_none());
        assert!(ring.get(100).is_none());
    }

    #[test]
    fn test_capacity_exact_fill() {
        let mut ring = HistoryRing::new(3);
        ring.push(content("a"));
        ring.push(content("b"));
        ring.push(content("c"));

        // Exactly at capacity, nothing should be dropped
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.get(0).map(|c| c.text.as_str()), Some("c"));
        assert_eq!(ring.get(1).map(|c| c.text.as_str()), Some("b"));
        assert_eq!(ring.get(2).map(|c| c.text.as_str()), Some("a"));
    }
}
