//! Change history log.
//!
//! Provides a linear log of all changes with timestamps,
//! useful for debugging and change tracking.

use {crate::mm::Edit, std::time::Instant};

/// An entry in the change history.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// Edits that were made.
    edits: Vec<Edit>,
    /// When the change occurred.
    timestamp: Instant,
    /// Sequential change number.
    seq_num: u64,
}

impl HistoryEntry {
    /// Get the edits in this entry.
    #[must_use]
    pub fn edits(&self) -> &[Edit] {
        &self.edits
    }

    /// Get when this change occurred.
    #[must_use]
    pub const fn timestamp(&self) -> Instant {
        self.timestamp
    }

    /// Get the sequential change number.
    #[must_use]
    pub const fn seq_num(&self) -> u64 {
        self.seq_num
    }
}

/// Change history log.
///
/// Records all changes made to a buffer in chronological order.
/// Unlike `UndoTree`, this is a simple linear log without branching.
///
/// # Use Cases
///
/// - Debugging: See what changes were made and when
/// - Auditing: Track modification history
/// - Collaboration: Synchronize changes between editors
///
/// # Memory Management
///
/// The history has a configurable maximum number of entries.
/// When exceeded, the oldest entries are removed.
#[derive(Debug)]
pub struct History {
    /// All recorded entries.
    entries: Vec<HistoryEntry>,
    /// Maximum number of entries to retain.
    max_entries: usize,
    /// Sequential change counter.
    seq_counter: u64,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    /// Default maximum number of entries.
    pub const DEFAULT_MAX_ENTRIES: usize = 10000;

    /// Create a new empty history.
    #[must_use]
    pub fn new() -> Self {
        Self::with_max_entries(Self::DEFAULT_MAX_ENTRIES)
    }

    /// Create a new history with custom maximum entries.
    #[must_use]
    pub fn with_max_entries(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries: max_entries.max(1),
            seq_counter: 0,
        }
    }

    /// Record a new change.
    pub fn record(&mut self, edits: Vec<Edit>) {
        if edits.is_empty() {
            return;
        }

        self.seq_counter += 1;

        let entry = HistoryEntry {
            edits,
            timestamp: Instant::now(),
            seq_num: self.seq_counter,
        };

        self.entries.push(entry);

        // Trim if over limit
        while self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    /// Get all entries in chronological order.
    #[must_use]
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Get the most recent entry.
    #[must_use]
    pub fn last(&self) -> Option<&HistoryEntry> {
        self.entries.last()
    }

    /// Get an entry by index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&HistoryEntry> {
        self.entries.get(index)
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.entries.clear();
        // Note: seq_counter is not reset to maintain uniqueness
    }

    /// Get the number of entries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if history is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the maximum entries limit.
    #[must_use]
    pub const fn max_entries(&self) -> usize {
        self.max_entries
    }

    /// Set the maximum entries limit.
    pub fn set_max_entries(&mut self, max: usize) {
        self.max_entries = max.max(1);
        while self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    /// Get the current sequential counter value.
    #[must_use]
    pub const fn seq_counter(&self) -> u64 {
        self.seq_counter
    }

    /// Get entries since a given sequence number.
    #[must_use]
    pub fn since(&self, seq_num: u64) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.seq_num > seq_num)
            .collect()
    }

    /// Get entries within a time range.
    #[must_use]
    pub fn between(&self, start: Instant, end: Instant) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .collect()
    }
}
