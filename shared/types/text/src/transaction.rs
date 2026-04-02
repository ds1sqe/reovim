//! Transaction for grouping edits.
//!
//! A transaction groups multiple edits together so they can be
//! undone/redone as a single unit.

use crate::Edit;

/// A transaction groups multiple edits for atomic undo/redo.
///
/// When a transaction is undone, all its edits are reversed in reverse order.
/// When redone, all edits are applied in original order.
///
/// # Example
///
/// ```
/// use reovim_types_text::*;
///
/// let mut txn = Transaction::new();
/// txn.push(Edit::insert(Position::new(0, 0), "Hello"));
/// txn.push(Edit::insert(Position::new(0, 5), " World"));
///
/// assert_eq!(txn.len(), 2);
/// assert!(!txn.is_empty());
///
/// // Get inverse for undo
/// let inverse = txn.inverse();
/// assert_eq!(inverse.len(), 2);
/// // Inverse edits are in reverse order
/// ```
#[derive(Debug, Clone, Default)]
pub struct Transaction {
    /// Edits in this transaction (in order applied).
    edits: Vec<Edit>,
}

impl Transaction {
    /// Create a new empty transaction.
    #[must_use]
    pub const fn new() -> Self {
        Self { edits: Vec::new() }
    }

    /// Returns an iterator over the edits.
    pub fn iter(&self) -> std::slice::Iter<'_, Edit> {
        self.edits.iter()
    }

    /// Create a transaction with pre-allocated capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            edits: Vec::with_capacity(capacity),
        }
    }

    /// Add an edit to this transaction.
    pub fn push(&mut self, edit: Edit) {
        self.edits.push(edit);
    }

    /// Get all edits in this transaction.
    #[must_use]
    pub fn edits(&self) -> &[Edit] {
        &self.edits
    }

    /// Consume the transaction and return the edits.
    #[must_use]
    pub fn into_edits(self) -> Vec<Edit> {
        self.edits
    }

    /// Check if this transaction has no edits.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Get the number of edits in this transaction.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.edits.len()
    }

    /// Create the inverse of this transaction (for undo).
    ///
    /// The inverse contains the inverse of each edit, in reverse order.
    /// Applying the inverse undoes the original transaction.
    #[must_use]
    pub fn inverse(&self) -> Self {
        Self {
            edits: self.edits.iter().rev().map(Edit::inverse).collect(),
        }
    }

    /// Clear all edits from this transaction.
    pub fn clear(&mut self) {
        self.edits.clear();
    }
}

impl From<Vec<Edit>> for Transaction {
    fn from(edits: Vec<Edit>) -> Self {
        Self { edits }
    }
}

impl From<Edit> for Transaction {
    fn from(edit: Edit) -> Self {
        Self { edits: vec![edit] }
    }
}

impl IntoIterator for Transaction {
    type Item = Edit;
    type IntoIter = std::vec::IntoIter<Edit>;

    fn into_iter(self) -> Self::IntoIter {
        self.edits.into_iter()
    }
}

impl<'a> IntoIterator for &'a Transaction {
    type Item = &'a Edit;
    type IntoIter = std::slice::Iter<'a, Edit>;

    fn into_iter(self) -> Self::IntoIter {
        self.edits.iter()
    }
}
