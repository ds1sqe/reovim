//! LSP completion source.
//!
//! Provides completion items from a Language Server Protocol server.
//! Uses a pre-cache pattern: the cache is populated asynchronously when
//! completion is triggered, and `complete()` reads from the cache synchronously.

use std::sync::Arc;

use {
    parking_lot::RwLock,
    reovim_kernel::api::v1::Service,
    reovim_subsys_completion::{
        CompletionContext, CompletionItem, CompletionKind, CompletionSource,
    },
};

/// Completion source backed by an LSP server.
///
/// The cache is populated by calling [`update_cache`](Self::update_cache) from
/// the completion trigger command handler after receiving the LSP response.
/// The sync [`complete`](CompletionSource::complete) method reads from the cache.
#[derive(Debug)]
pub struct LspCompletionSource {
    /// Cached items from the last LSP completion response.
    cache: Arc<RwLock<Vec<CompletionItem>>>,
}

impl LspCompletionSource {
    /// Create a new empty LSP completion source.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Update the cache with new items from an LSP response.
    pub fn update_cache(&self, items: Vec<CompletionItem>) {
        let mut cache = self.cache.write();
        *cache = items;
    }

    /// Clear the cached items.
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }

    /// Get the number of cached items.
    #[must_use]
    pub fn cache_len(&self) -> usize {
        self.cache.read().len()
    }

    /// Check if the cache is empty.
    #[must_use]
    pub fn is_cache_empty(&self) -> bool {
        self.cache.read().is_empty()
    }
}

impl Service for LspCompletionSource {}

impl Default for LspCompletionSource {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionSource for LspCompletionSource {
    fn id(&self) -> &'static str {
        "lsp"
    }

    fn priority(&self) -> u16 {
        200
    }

    fn is_available(&self, ctx: &CompletionContext) -> bool {
        ctx.language_id.is_some()
    }

    fn complete(&self, ctx: &CompletionContext) -> Vec<CompletionItem> {
        let cache = self.cache.read();
        if cache.is_empty() {
            return Vec::new();
        }

        let prefix = &ctx.prefix;
        if prefix.is_empty() {
            return cache.clone();
        }

        // Filter cached items by prefix (case-insensitive).
        let prefix_lower = prefix.to_ascii_lowercase();
        cache
            .iter()
            .filter(|item| item.label.to_ascii_lowercase().starts_with(&prefix_lower))
            .cloned()
            .collect()
    }
}

/// Map an `lsp_types::CompletionItem` to a driver `CompletionItem`.
#[must_use]
pub fn map_lsp_item(item: &lsp_types::CompletionItem) -> CompletionItem {
    let label = item.label.clone();
    let insert_text = item.insert_text.clone().unwrap_or_else(|| label.clone());

    let kind = item.kind.map_or(CompletionKind::Text, map_lsp_kind);

    let detail = item.detail.clone();

    let documentation = item.documentation.as_ref().map(|doc| match doc {
        lsp_types::Documentation::String(s) => s.clone(),
        lsp_types::Documentation::MarkupContent(mc) => mc.value.clone(),
    });

    let is_snippet = item
        .insert_text_format
        .is_some_and(|f| f == lsp_types::InsertTextFormat::SNIPPET);

    CompletionItem {
        label,
        insert_text,
        kind,
        detail,
        documentation,
        source_id: "lsp",
        is_snippet,
        sort_priority: 200,
    }
}

/// Map an `lsp_types::CompletionItemKind` to a driver `CompletionKind`.
#[must_use]
pub const fn map_lsp_kind(kind: lsp_types::CompletionItemKind) -> CompletionKind {
    match kind {
        lsp_types::CompletionItemKind::FUNCTION => CompletionKind::Function,
        lsp_types::CompletionItemKind::METHOD => CompletionKind::Method,
        lsp_types::CompletionItemKind::VARIABLE => CompletionKind::Variable,
        lsp_types::CompletionItemKind::FIELD => CompletionKind::Field,
        lsp_types::CompletionItemKind::KEYWORD => CompletionKind::Keyword,
        lsp_types::CompletionItemKind::SNIPPET => CompletionKind::Snippet,
        lsp_types::CompletionItemKind::MODULE => CompletionKind::Module,
        lsp_types::CompletionItemKind::CLASS | lsp_types::CompletionItemKind::STRUCT => {
            CompletionKind::Class
        }
        lsp_types::CompletionItemKind::INTERFACE => CompletionKind::Interface,
        lsp_types::CompletionItemKind::PROPERTY => CompletionKind::Property,
        lsp_types::CompletionItemKind::CONSTANT => CompletionKind::Constant,
        lsp_types::CompletionItemKind::ENUM => CompletionKind::Enum,
        lsp_types::CompletionItemKind::ENUM_MEMBER => CompletionKind::EnumMember,
        lsp_types::CompletionItemKind::FILE => CompletionKind::File,
        lsp_types::CompletionItemKind::FOLDER => CompletionKind::Folder,
        lsp_types::CompletionItemKind::TYPE_PARAMETER => CompletionKind::TypeParameter,
        _ => CompletionKind::Text,
    }
}

#[cfg(test)]
#[path = "lsp_source_tests.rs"]
mod tests;
