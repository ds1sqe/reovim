# vfs/ - Virtual Filesystem

File operations abstraction.

## Source Location

`lib/drivers/vfs/src/`

## Key Traits

```rust
pub trait VfsDriver: Send + Sync {
    fn read(&self, path: &Path) -> Result<Vec<u8>>;
    fn write(&mut self, path: &Path, content: &[u8]) -> Result<()>;
    fn exists(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;
    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>>;
    fn create_dir(&mut self, path: &Path) -> Result<()>;
    fn remove(&mut self, path: &Path) -> Result<()>;
    fn metadata(&self, path: &Path) -> Result<Metadata>;
}
```

## Types

```rust
pub struct DirEntry {
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

pub struct Metadata {
    pub size: u64,
    pub modified: SystemTime,
    pub is_readonly: bool,
}
```

## Related Documents

- [Driver Overview](../overview.md) - Driver layer architecture
