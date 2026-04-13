//! Git signs annotation source.

use std::{ops::Range, sync::Arc};

use {
    reovim_kernel::api::v1::{BufferId, ServiceRegistry},
    reovim_subsys_annotation::{
        Annotation, AnnotationContext, AnnotationKind, AnnotationPayload, AnnotationSource,
        AnnotationTarget,
    },
    reovim_subsys_git::GitProviderStore,
};

use crate::hunk::{SignKind, hunk_to_signs};

/// Annotation source that produces gutter signs from git diff hunks.
///
/// Reads `DiffHunk` data from the `GitProvider` and converts them
/// into `"git.add"`, `"git.change"`, and `"git.delete"` annotations.
pub struct GitSignsSource {
    services: Arc<ServiceRegistry>,
}

impl GitSignsSource {
    /// Create a new git signs source.
    pub const fn new(services: Arc<ServiceRegistry>) -> Self {
        Self { services }
    }

    /// Get the annotation kind string for a sign kind.
    const fn kind_str(kind: SignKind) -> &'static str {
        match kind {
            SignKind::Add => "git.add",
            SignKind::Change => "git.change",
            SignKind::Delete => "git.delete",
        }
    }

    /// Priority for git sign annotations.
    const PRIORITY: u8 = 10;
}

impl AnnotationSource for GitSignsSource {
    fn id(&self) -> &'static str {
        "plugin.git-signs"
    }

    fn provides(&self) -> Vec<AnnotationKind> {
        vec![
            AnnotationKind::new("git.add"),
            AnnotationKind::new("git.change"),
            AnnotationKind::new("git.delete"),
        ]
    }

    fn annotations(
        &self,
        _buffer_id: BufferId,
        range: Range<usize>,
        context: &AnnotationContext,
    ) -> Vec<Annotation> {
        let Some(file_path) = &context.file_path else {
            return vec![];
        };

        let Some(store) = self.services.get::<GitProviderStore>() else {
            return vec![];
        };
        let Some(git) = store.get() else {
            return vec![];
        };

        let hunks = git.diff_hunks(file_path);

        hunks
            .iter()
            .flat_map(hunk_to_signs)
            .filter(|sign| range.contains(&sign.line))
            .map(|sign| Annotation {
                kind: AnnotationKind::new(Self::kind_str(sign.kind)),
                target: AnnotationTarget::Line(sign.line),
                priority: Self::PRIORITY,
                payload: AnnotationPayload::default(),
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
