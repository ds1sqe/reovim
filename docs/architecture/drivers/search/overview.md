# search/ - Search Provider Driver

Pattern matching interface for buffer searching.

## Source Location

`server/lib/drivers/search/src/`

## Purpose

Defines the interface for pattern matching in buffers. Provides the trait and
types for search functionality.

Following the mechanism/policy separation:
- **Mechanism** (this driver): `SearchProvider` trait, types, registry
- **Policy** (modules): Implementations like regex-based `SearchEngine`

## Architecture

```
lib/drivers/search/   -> Trait + Types + Key + Registry (MECHANISM)
modules/search/       -> SearchEngine implementation (POLICY)
```

## Key Types

### SearchProvider Trait

```rust
pub trait SearchProvider: Send + Sync {
    /// Search for pattern in buffer
    fn search(
        &self,
        buffer: &Buffer,
        pattern: &str,
        from: Position,
        direction: Direction,
    ) -> Result<Option<SearchMatch>, SearchError>;

    /// Find all matches in buffer
    fn find_all(
        &self,
        buffer: &Buffer,
        pattern: &str,
    ) -> Result<Vec<SearchMatch>, SearchError>;

    /// Replace matches
    fn replace(
        &self,
        buffer: &mut Buffer,
        pattern: &str,
        replacement: &str,
        range: Option<Range>,
    ) -> Result<usize, SearchError>;
}
```

### Direction

```rust
pub enum Direction {
    Forward,
    Backward,
}
```

### SearchMatch

```rust
pub struct SearchMatch {
    pub start: Position,
    pub end: Position,
    pub text: String,
}
```

### SearchError

```rust
pub enum SearchError {
    InvalidPattern(String),
    NotFound,
    RegexError(String),
}
```

### Registry

```rust
pub enum SearchKey {
    Default,
    // Future: Fuzzy, Aho-Corasick, etc.
}

pub struct SearchProviderRegistry {
    providers: RwLock<HashMap<SearchKey, Arc<dyn SearchProvider>>>,
}
```

## Dependencies

- `reovim_kernel::api::v1` - Buffer, Position

## Related Documents

- [Driver Overview](../overview.md)
