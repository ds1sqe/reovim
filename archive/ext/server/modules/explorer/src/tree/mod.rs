//! File tree data structures for the explorer module.
//!
//! Provides the core tree representation (`FileTree`, `FileNode`, `NodeType`)
//! and rendering utilities (`build_tree_prefix`, `FlattenedNode`).

pub mod file_tree;
pub mod node;
pub mod prefix;

pub use {
    file_tree::{FileTree, FlattenedNode},
    node::{FileNode, NodeType, format_size},
    prefix::{TreeLineInfo, build_tree_prefix},
};
