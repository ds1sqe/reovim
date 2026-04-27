use super::*;

fn make_ctx(content: &str, prefix: &str) -> CompletionContext {
    CompletionContext {
        content: content.to_owned(),
        cursor_offset: content.len(),
        line: 0,
        col: prefix.len(),
        prefix: prefix.to_owned(),
        buffer_id: 1,
        file_path: None,
        language_id: None,
    }
}

#[test]
fn source_id() {
    let source = BufferWordsSource;
    assert_eq!(source.id(), "buffer");
}

#[test]
fn source_priority() {
    let source = BufferWordsSource;
    assert_eq!(source.priority(), 100);
}

#[test]
fn always_available() {
    let source = BufferWordsSource;
    let ctx = make_ctx("hello", "h");
    assert!(source.is_available(&ctx));
}

#[test]
fn available_without_language_id() {
    let source = BufferWordsSource;
    let ctx = CompletionContext {
        content: String::new(),
        cursor_offset: 0,
        line: 0,
        col: 0,
        prefix: String::new(),
        buffer_id: 1,
        file_path: None,
        language_id: None,
    };
    assert!(source.is_available(&ctx));
}

#[test]
fn empty_prefix_returns_empty() {
    let source = BufferWordsSource;
    let ctx = make_ctx("hello world foo", "");
    let items = source.complete(&ctx);
    assert!(items.is_empty());
}

#[test]
fn matches_prefix_case_insensitive() {
    let source = BufferWordsSource;
    let ctx = make_ctx("Hello World Help here", "he");
    let items = source.complete(&ctx);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"Hello"));
    assert!(labels.contains(&"Help"));
    assert!(labels.contains(&"here"));
}

#[test]
fn excludes_exact_prefix() {
    let source = BufferWordsSource;
    let ctx = make_ctx("hello hello_world help", "hello");
    let items = source.complete(&ctx);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    // "hello" is excluded because it exactly matches the prefix.
    assert!(!labels.contains(&"hello"));
    // "hello_world" matches prefix and is included.
    assert!(labels.contains(&"hello_world"));
}

#[test]
fn deduplicates_words() {
    let source = BufferWordsSource;
    let ctx = make_ctx("foo foo foo bar foobar", "fo");
    let items = source.complete(&ctx);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels.iter().filter(|l| **l == "foo").count(), 1);
    assert!(labels.contains(&"foobar"));
}

#[test]
fn skips_short_words() {
    let source = BufferWordsSource;
    let ctx = make_ctx("a b ab abc", "a");
    let items = source.complete(&ctx);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    // "a" is too short (< 2), "b" is too short and doesn't match.
    assert!(!labels.contains(&"a"));
    assert!(labels.contains(&"ab"));
    assert!(labels.contains(&"abc"));
}

#[test]
fn items_have_correct_kind() {
    let source = BufferWordsSource;
    let ctx = make_ctx("hello world help", "he");
    let items = source.complete(&ctx);
    for item in &items {
        assert_eq!(item.kind, CompletionKind::Text);
        assert_eq!(item.source_id, "buffer");
        assert!(!item.is_snippet);
    }
}

#[test]
fn handles_underscores() {
    let source = BufferWordsSource;
    let ctx = make_ctx("my_var my_func other_thing", "my");
    let items = source.complete(&ctx);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"my_var"));
    assert!(labels.contains(&"my_func"));
    assert!(!labels.contains(&"other_thing"));
}

#[test]
fn handles_empty_content() {
    let source = BufferWordsSource;
    let ctx = make_ctx("", "fo");
    let items = source.complete(&ctx);
    assert!(items.is_empty());
}

#[test]
fn extract_words_basic() {
    let words: Vec<&str> = extract_words("hello world").collect();
    assert_eq!(words, vec!["hello", "world"]);
}

#[test]
fn extract_words_with_punctuation() {
    let words: Vec<&str> = extract_words("fn main() { let x = 42; }").collect();
    assert!(words.contains(&"fn"));
    assert!(words.contains(&"main"));
    assert!(words.contains(&"let"));
    assert!(words.contains(&"x"));
    assert!(words.contains(&"42"));
}

#[test]
fn extract_words_with_underscores() {
    let words: Vec<&str> = extract_words("my_var = some_func()").collect();
    assert!(words.contains(&"my_var"));
    assert!(words.contains(&"some_func"));
}

#[test]
fn extract_words_empty() {
    assert!(extract_words("").next().is_none());
}

#[test]
fn extract_words_only_punctuation() {
    assert!(extract_words("  (){}[];  ").next().is_none());
}

#[test]
fn debug_impl() {
    let source = BufferWordsSource;
    let debug = format!("{source:?}");
    assert!(debug.contains("BufferWordsSource"));
}
