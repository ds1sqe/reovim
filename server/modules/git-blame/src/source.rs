//! Blame annotation source.

use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use {
    reovim_kernel::api::v1::BufferId,
    reovim_driver_annotation::{
        Annotation, AnnotationContext, AnnotationKind, AnnotationPayload, AnnotationSource,
        AnnotationTarget,
    },
    reovim_driver_git::{GitProvider, types::BlameEntry},
};

use crate::format::format_blame;

/// Annotation source that shows git blame information per line.
///
/// Blame is on-demand: must be toggled on for a buffer before
/// annotations are produced. When active, caches blame results
/// per file path.
pub struct BlameAnnotationSource {
    provider: Arc<dyn GitProvider>,
    /// Set of buffers with blame active.
    active: RwLock<HashSet<BufferId>>,
    /// Cached blame data keyed by file path.
    cache: RwLock<HashMap<PathBuf, Vec<BlameEntry>>>,
}

impl BlameAnnotationSource {
    /// Create a new blame annotation source.
    pub fn new(provider: Arc<dyn GitProvider>) -> Self {
        Self {
            provider,
            active: RwLock::new(HashSet::new()),
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Toggle blame on/off for a buffer.
    ///
    /// When toggling on, runs `git blame` and caches the result.
    /// When toggling off, removes from active set.
    ///
    /// # Panics
    ///
    /// Panics if any internal lock is poisoned.
    pub fn toggle(&self, buffer_id: BufferId, file_path: Option<&PathBuf>) {
        let mut active = self.active.write().expect("active lock poisoned");
        if active.contains(&buffer_id) {
            active.remove(&buffer_id);
        } else {
            active.insert(buffer_id);
            drop(active);
            // Pre-cache blame data if file path available
            if let Some(path) = file_path {
                let entries = self.provider.blame(path);
                self.cache
                    .write()
                    .expect("cache lock poisoned")
                    .insert(path.clone(), entries);
            }
        }
    }

    /// Check if blame is active for a buffer.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn is_active(&self, buffer_id: BufferId) -> bool {
        self.active
            .read()
            .expect("active lock poisoned")
            .contains(&buffer_id)
    }

    /// Priority for blame annotations.
    const PRIORITY: u8 = 5;
}

impl AnnotationSource for BlameAnnotationSource {
    fn id(&self) -> &'static str {
        "plugin.git-blame"
    }

    fn provides(&self) -> Vec<AnnotationKind> {
        vec![AnnotationKind::new("blame.info")]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn annotations(
        &self,
        buffer_id: BufferId,
        range: Range<usize>,
        context: &AnnotationContext,
    ) -> Vec<Annotation> {
        if !self.is_active(buffer_id) {
            return vec![];
        }

        let Some(file_path) = &context.file_path else {
            return vec![];
        };

        // Ensure cache is populated.
        // Note: read-check-drop-write has a benign TOCTOU window under
        // concurrent access — two threads may both call blame() for the
        // same path, but the result is idempotent (last write wins with
        // identical data), so no correctness issue.
        {
            let cache = self.cache.read().expect("cache lock poisoned");
            if cache.get(file_path).is_none() {
                drop(cache);
                let entries = self.provider.blame(file_path);
                self.cache
                    .write()
                    .expect("cache lock poisoned")
                    .insert(file_path.clone(), entries);
            }
        }

        let cache = self.cache.read().expect("cache lock poisoned");
        let Some(entries) = cache.get(file_path) else {
            return vec![];
        };
        let result = entries
            .iter()
            .filter(|e| {
                let line_idx = e.line.saturating_sub(1);
                range.contains(&line_idx)
            })
            .map(|e| Annotation {
                kind: AnnotationKind::new("blame.info"),
                target: AnnotationTarget::Line(e.line.saturating_sub(1)),
                priority: Self::PRIORITY,
                payload: AnnotationPayload::text(format_blame(e, None)),
            })
            .collect();
        drop(cache);
        result
    }
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
