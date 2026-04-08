#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Virtual filesystem driver for reovim.
//!
//! Linux equivalent: `fs/`
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                         VFS Layer                           │
//! │                                                             │
//! │  ┌─────────────────┐  ┌─────────────────┐                   │
//! │  │   VfsDriver     │  │  PathNormalizer │                   │
//! │  │ (filesystem     │  │ (path utilities)│                   │
//! │  │  operations)    │  │                 │                   │
//! │  └────────┬────────┘  └─────────────────┘                   │
//! │           │                                                 │
//! │           ├──────────────────┐                              │
//! │           │                  │                              │
//! │           ▼                  ▼                              │
//! │  ┌─────────────────┐  ┌─────────────────┐                   │
//! │  │   FileHandle    │  │   FileWatcher   │                   │
//! │  │ (streaming I/O) │  │ (change events) │                   │
//! │  └─────────────────┘  └─────────────────┘                   │
//! │                                                             │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              │ (implementations in server/modules/)
//!                              ▼
//!              ┌───────────────────────────────┐
//!              │      Concrete VFS Impls       │
//!              │  - StandardVfs (std::fs)      │
//!              │  - RemoteVfs (future)         │
//!              │  - MemoryVfs (testing)        │
//!              └───────────────────────────────┘
//! ```
//!
//! # Components
//!
//! - [`VfsDriver`] - Main filesystem operations trait
//! - [`FileHandle`] - Open file streaming operations
//! - [`FileWatcher`] - File change monitoring
//! - [`PathNormalizer`] - Cross-platform path handling
//! - [`FileMetadata`] - File information
//! - [`FilePermissions`] - Unix-style permissions
//! - [`VfsError`] - Error types
//!
//! # Usage Note
//!
//! All filesystem access in the codebase goes through VFS.
//!
//! See the `VfsDriver` trait documentation for the complete API.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_vfs::{VfsDriver, OpenOptions, VfsError};
//! use std::path::Path;
//!
//! fn read_file(vfs: &dyn VfsDriver, path: &Path) -> Result<String, VfsError> {
//!     vfs.read_to_string(path)
//! }
//!
//! fn write_file(vfs: &dyn VfsDriver, path: &Path, content: &str) -> Result<(), VfsError> {
//!     vfs.write_str(path, content)
//! }
//! ```

// byte_undo_log moved to reovim-kernel::block (#740)
mod byte_source;
mod error;
mod file_mapping;
mod filetype;
mod instance;
mod mapped_file;
mod metadata;
mod mock;
mod module_ext;
mod path;
mod piece_table;
mod provider;
mod registry;
mod scheme;
mod standard;
mod traits;
mod watch;

// Note: ByteEdit, ByteUndoLog, ByteUndoEntry moved to reovim-kernel::block (#740)

// Re-export error types
pub use error::VfsError;

// Re-export byte-level large-file support types
pub use {
    file_mapping::FileMapping,
    piece_table::{Piece, PieceMetrics, PieceSource, PieceTree},
};

// Re-export VFS implementations
pub use {
    mock::{MockErrorKind, MockVfs},
    standard::{StandardFileHandle, StandardVfs},
};

// Re-export filetype types
pub use filetype::{FiletypeInfo, FiletypeRegistry, detect_filetype, filetype_id, global_registry};

// Re-export metadata types
pub use metadata::{FileMetadata, FilePermissions};

// Re-export path types
pub use path::{PathNormalizer, StandardPathNormalizer};

// Re-export watch types
pub use watch::{WatchEvent, WatchHandle, WatchId};

// Re-export memory-mapped file type
pub use {
    byte_source::{ByteSource, ByteSourceCapabilities, HeapByteSource, MappedByteSource},
    mapped_file::MappedFile,
};

// Re-export traits and related types
pub use traits::{DirEntry, FileHandle, FileWatcher, OpenOptions, SeekFrom, VfsDriver};

// Re-export provider types (Epic #415 - Module provider hooks)
pub use {
    module_ext::VfsProviderModule,
    provider::{ProviderPriority, VfsProvider},
};

// Re-export typed key and registry (Epic #417 - UniqueProvider abstraction)
pub use {registry::VfsProviderRegistry, scheme::VfsScheme};

// Re-export VFS instance wrapper (Epic #465 - ex-command VFS access)
pub use instance::VfsInstance;

#[cfg(test)]
#[path = "byte_source_tests.rs"]
mod byte_source_tests;
