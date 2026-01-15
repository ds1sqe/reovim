# lsp/ - LSP Driver

Language Server Protocol client infrastructure.

## Source Location

`lib/drivers/lsp/src/`

## Key Traits

```rust
pub trait LspClient: Send + Sync {
    fn initialize(&mut self, root: &Path) -> Result<InitializeResult>;
    fn shutdown(&mut self) -> Result<()>;

    fn did_open(&mut self, doc: TextDocumentItem) -> Result<()>;
    fn did_change(&mut self, uri: &Url, changes: Vec<TextEdit>) -> Result<()>;
    fn did_save(&mut self, uri: &Url) -> Result<()>;
    fn did_close(&mut self, uri: &Url) -> Result<()>;

    fn completion(&mut self, params: CompletionParams) -> Result<CompletionList>;
    fn hover(&mut self, params: HoverParams) -> Result<Option<Hover>>;
    fn definition(&mut self, params: GotoParams) -> Result<Vec<Location>>;
    fn references(&mut self, params: ReferenceParams) -> Result<Vec<Location>>;
}
```

## LSP Message

```rust
pub struct LspMessage {
    pub jsonrpc: String,
    pub id: Option<RequestId>,
    pub method: Option<String>,
    pub params: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<ResponseError>,
}
```

## Related Documents

- [Driver Overview](../overview.md) - Driver layer architecture
- [Network Driver](../net/overview.md) - Transport layer
