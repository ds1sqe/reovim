//! Treesitter initialization benchmarks
//!
//! Measures query compilation time - the main bottleneck identified in #86.
//! Tree-sitter query compilation during language registration blocks the event bus,
//! causing auto-pair latency of ~100ms+ when opening files.

use std::hint::black_box;
use std::sync::Arc;

use criterion::{BenchmarkId, Criterion};
use reovim_plugin_treesitter::{LanguageSupport, TreesitterManager};
use tree_sitter::Query;

/// Benchmark pure query compilation (bypassing cache)
/// This is the actual bottleneck - tree_sitter::Query::new()
pub fn bench_query_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("treesitter/query_compile");

    // Define languages with their highlight query sizes
    let languages: Vec<(&str, Box<dyn LanguageSupport + Send + Sync>)> = vec![
        ("rust", Box::new(reovim_lang_rust::RustLanguage)),
        ("c", Box::new(reovim_lang_c::CLanguage)),
        (
            "javascript",
            Box::new(reovim_lang_javascript::JavaScriptLanguage),
        ),
        ("python", Box::new(reovim_lang_python::PythonLanguage)),
        ("bash", Box::new(reovim_lang_bash::BashLanguage)),
        ("json", Box::new(reovim_lang_json::JsonLanguage)),
        ("toml", Box::new(reovim_lang_toml::TomlLanguage)),
        (
            "markdown",
            Box::new(reovim_lang_markdown::MarkdownLanguage),
        ),
    ];

    for (name, lang) in &languages {
        let ts_lang = lang.tree_sitter_language();
        let query_src = lang.highlights_query();

        group.bench_with_input(BenchmarkId::new("highlights", name), &(), |b, _| {
            b.iter(|| {
                // Direct Query::new - bypasses cache, measures pure compilation
                let query = Query::new(black_box(&ts_lang), black_box(query_src));
                black_box(query)
            });
        });
    }

    group.finish();
}

/// Benchmark full language registration (all query types)
pub fn bench_language_registration(c: &mut Criterion) {
    let mut group = c.benchmark_group("treesitter/register_language");

    // Test individual language registration
    group.bench_function("rust", |b| {
        b.iter(|| {
            let mut manager = TreesitterManager::new();
            manager.register_language(Arc::new(reovim_lang_rust::RustLanguage));
            black_box(manager)
        });
    });

    group.bench_function("c", |b| {
        b.iter(|| {
            let mut manager = TreesitterManager::new();
            manager.register_language(Arc::new(reovim_lang_c::CLanguage));
            black_box(manager)
        });
    });

    group.bench_function("javascript", |b| {
        b.iter(|| {
            let mut manager = TreesitterManager::new();
            manager.register_language(Arc::new(reovim_lang_javascript::JavaScriptLanguage));
            black_box(manager)
        });
    });

    group.bench_function("python", |b| {
        b.iter(|| {
            let mut manager = TreesitterManager::new();
            manager.register_language(Arc::new(reovim_lang_python::PythonLanguage));
            black_box(manager)
        });
    });

    group.finish();
}

/// Benchmark total startup time - all languages
pub fn bench_all_languages_registration(c: &mut Criterion) {
    c.bench_function("treesitter/register_all_languages", |b| {
        b.iter(|| {
            let mut manager = TreesitterManager::new();
            manager.register_language(Arc::new(reovim_lang_rust::RustLanguage));
            manager.register_language(Arc::new(reovim_lang_c::CLanguage));
            manager.register_language(Arc::new(reovim_lang_javascript::JavaScriptLanguage));
            manager.register_language(Arc::new(reovim_lang_python::PythonLanguage));
            manager.register_language(Arc::new(reovim_lang_json::JsonLanguage));
            manager.register_language(Arc::new(reovim_lang_toml::TomlLanguage));
            manager.register_language(Arc::new(reovim_lang_bash::BashLanguage));
            manager.register_language(Arc::new(reovim_lang_markdown::MarkdownLanguage));
            manager.register_language(Arc::new(reovim_lang_markdown::MarkdownInlineLanguage));
            black_box(manager)
        });
    });
}
