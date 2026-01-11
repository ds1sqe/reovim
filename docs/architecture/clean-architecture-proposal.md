# Reovim Clean Architecture Refactoring Proposal

## EPIC: Project Kernel - Linux-Inspired Architecture Overhaul

> *"Those who do not understand Unix are condemned to reinvent it, poorly."*
> — Henry Spencer

> *"Linux is evolution, not intelligent design."*
> — Linus Torvalds

This document defines the complete architectural transformation of Reovim following Linux kernel design principles. The goal: create an editor architecture as clean, modular, and battle-tested as the world's most successful open-source operating system.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Design Decisions](#2-design-decisions)
3. [Architecture Overview](#3-architecture-overview)
4. [Layer Specifications](#4-layer-specifications)
   - 4.1 [Architecture Layer (lib/arch/)](#41-architecture-layer-libarch)
   - 4.2 [Kernel Layer (lib/kernel/)](#42-kernel-layer-libkernel)
   - 4.3 [Driver Layer (lib/drivers/)](#43-driver-layer-libdrivers)
   - 4.4 [Module System](#44-module-system)
5. [Dynamic Module Loading](#5-dynamic-module-loading)
6. [Interface Contracts](#6-interface-contracts)
7. [Subsystem Deep Dives](#7-subsystem-deep-dives)
8. [Migration Plan](#8-migration-plan)
9. [Deprecation Schedule](#9-deprecation-schedule)
10. [Testing Strategy](#10-testing-strategy)
11. [Performance Contracts](#11-performance-contracts)
12. [Appendices](#12-appendices)

---

## 1. Executive Summary

### 1.1 Vision

Transform Reovim from a monolithic editor into a **microkernel-inspired architecture** where:

- **Kernel** provides minimal, pure mechanisms
- **Drivers** bridge kernel to hardware/services via trait objects
- **Modules** implement policies and features, loadable at runtime
- **No circular dependencies** - strict layered architecture
- **Hot-reloadable components** - change syntax highlighting without restart

### 1.2 Inspiration: Linux Kernel

| Linux Kernel | Reovim | Purpose |
|--------------|--------|---------|
| `arch/` | `lib/arch/` | Platform abstraction |
| `kernel/` | `lib/kernel/` | Core mechanisms |
| `drivers/` | `lib/drivers/*` | Hardware/service adapters |
| `fs/` | `lib/drivers/vfs/` | Filesystem abstraction |
| `net/` | `lib/drivers/net/` | Network/RPC subsystem |
| `mm/` | `lib/kernel/mm/` | Memory management |
| `include/linux/` | `lib/kernel/api/` | Stable kernel API |
| Loadable Kernel Modules | `plugins/` | Dynamic modules |
| `/proc`, `/sys` | Debug/introspection API | Runtime inspection |

### 1.3 Current State Analysis

**Critical Issues:**
```
lib/core/Cargo.toml:
├── tree-sitter-rust    ← Language in core!
├── tree-sitter-c       ← Language in core!
├── tree-sitter-python  ← Language in core!
└── ... 9 languages embedded

render_pipeline.rs: 45KB (965 lines) ← God file
handlers.rs: 47KB (1073 lines)       ← God file
event_loop.rs: 39KB (890 lines)      ← God file
```

**Target State:**
```
lib/kernel/:           ZERO external syntax dependencies
lib/drivers/syntax/:   SyntaxDriver trait only
plugins/languages/*:   tree-sitter implementations
```

---

## 2. Design Decisions

### 2.1 Answered Questions

| Question | Decision | Rationale |
|----------|----------|-----------|
| Maintain `lib/core/` as re-export? | **Full Deprecation** | Clean break, no legacy baggage |
| Driver crate organization? | **Multiple crates** | Independent versioning, parallel compilation |
| Minimum Rust version? | **Nightly OK** | Access to powerful unstable features |
| Dynamic module loading? | **Yes** | Hot-reload, runtime extensibility |

### 2.2 Nightly Features We'll Use

```rust
#![feature(trait_upcasting)]           // Simplify trait hierarchies
#![feature(type_alias_impl_trait)]     // Cleaner async trait returns
#![feature(impl_trait_in_assoc_type)]  // Async traits without boxing
#![feature(allocator_api)]             // Custom allocators for buffers
#![feature(ptr_metadata)]              // Better trait object handling
#![feature(negative_impls)]            // Explicit !Send/!Sync
#![feature(specialization)]            // Default trait implementations
#![feature(const_type_id)]             // Compile-time type checks
#![feature(adt_const_params)]          // Const generics for IDs
#![feature(lazy_cell)]                 // Better lazy statics
```

### 2.3 Design Principles

Following Linux kernel philosophy:

1. **Mechanism, not Policy**
   - Kernel provides *how* (mechanisms)
   - Modules decide *what* (policies)
   - Example: Kernel provides motion math, module decides "w moves by word"

2. **Everything is a File (Buffer)**
   - Buffers are the universal abstraction
   - Commands operate on buffers
   - Even UI elements are buffer-like

3. **Subsystem Independence**
   - Each subsystem compiles alone
   - No cross-subsystem dependencies
   - Communication via well-defined interfaces

4. **Fail Fast, Fail Loud**
   - Panics in kernel indicate bugs
   - Errors in modules are recoverable
   - Never silently corrupt state

5. **Performance is a Feature**
   - Hot paths must be allocation-free
   - Lock-free where possible
   - Cache-friendly data structures

6. **Kernel Purity**
   - Kernel uses minimal external dependencies (pure Rust where possible)
   - Kernel provides data structures and accessors
   - Drivers/modules handle format-specific concerns (serialization, I/O formats)
   - Example: Kernel's `Snapshot` provides `lines()`, `cursor()` accessors;
     driver layer handles JSON serialization for file storage

7. **Linux-Aligned printk**
   - Kernel has its own `printk/` subsystem (like Linux `kernel/printk/`)
   - Kernel defines `Logger` trait (mechanism), drivers implement (policy)
   - Zero external dependencies in kernel - std only
   - External deps (tracing, etc.) belong in `drivers/log`, not kernel
   - Arch layer provides sync primitives only, NOT logging
   - Macros: `pr_err!()`, `pr_warn!()`, `pr_info!()`, `pr_debug!()`

---

## 3. Architecture Overview

### 3.1 Layer Diagram

```
┌────────────────────────────────────────────────────────────────────────────┐
│                              USER SPACE                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         runner/                                     │   │
│  │   CLI │ Server │ Config │ Plugin Loader │ Module Manager            │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────────────┘
                                     │
                        ═════════════╪═════════════ System Call Boundary
                                     │
┌────────────────────────────────────────────────────────────────────────────┐
│                           MODULES (plugins/)                               │
│                                                                            │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │ features/                                                            │  │
│  │ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐     │  │
│  │ │completion│ │ explorer │ │telescope │ │which-key │ │statusline│     │  │
│  │ └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘     │  │
│  │ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐     │  │
│  │ │  lsp-ui  │ │  notify  │ │ settings │ │  health  │ │ context  │     │  │
│  │ └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘     │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                            │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │ languages/ (Loadable Syntax Modules - LSM)                           │  │
│  │ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐    │  │
│  │ │  rust  │ │   c    │ │  python│ │   js   │ │  json  │ │  toml  │    │  │
│  │ │.so/.dll│ │.so/.dll│ │.so/.dll│ │.so/.dll│ │.so/.dll│ │.so/.dll│    │  │
│  │ └────────┘ └────────┘ └────────┘ └────────┘ └────────┘ └────────┘    │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────────┘
                                     │
                        ═════════════╪═════════════ Module Interface
                                     │
┌────────────────────────────────────────────────────────────────────────────┐
│                          DRIVERS (lib/drivers/*)                           │
│                                                                            │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐             │
│  │ lib/drivers/    │  │ lib/drivers/    │  │ lib/drivers/    │             │
│  │    display/     │  │    input/       │  │    syntax/      │             │
│  │ ┌─────────────┐ │  │ ┌─────────────┐ │  │ ┌─────────────┐ │             │
│  │ │   frame/    │ │  │ │  keyboard/  │ │  │ │   traits/   │ │             │
│  │ │   term/     │ │  │ │   mouse/    │ │  │ │  registry/  │ │             │
│  │ │ compositor/ │ │  │ │ clipboard/  │ │  │ │   cache/    │ │             │
│  │ └─────────────┘ │  │ └─────────────┘ │  │ └─────────────┘ │             │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘             │
│                                                                            │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐             │
│  │ lib/drivers/    │  │ lib/drivers/    │  │ lib/drivers/    │             │
│  │    lsp/         │  │    net/         │  │    vfs/         │             │
│  │ ┌─────────────┐ │  │ ┌─────────────┐ │  │ ┌─────────────┐ │             │
│  │ │   client/   │ │  │ │    rpc/     │ │  │ │   local/    │ │             │
│  │ │  protocol/  │ │  │ │ transport/  │ │  │ │   ops/      │ │             │
│  │ │ diagnostics/│ │  │ │  handler/   │ │  │ │   watch/    │ │             │
│  │ └─────────────┘ │  │ └─────────────┘ │  │ └─────────────┘ │             │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘             │
└────────────────────────────────────────────────────────────────────────────┘
                                     │
                        ═════════════╪═════════════ Driver Interface
                                     │
┌────────────────────────────────────────────────────────────────────────────┐
│                           KERNEL (lib/kernel/)                             │
│                                                                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │   sched/    │  │    mm/      │  │    ipc/     │  │   core/     │        │
│  │  ─────────  │  │  ─────────  │  │  ─────────  │  │  ─────────  │        │
│  │  runtime    │  │  buffer     │  │  event_bus  │  │  command    │        │
│  │  executor   │  │  rope       │  │  channel    │  │  motion     │        │
│  │  task       │  │  cache      │  │  scope      │  │  textobj    │        │
│  │  priority   │  │  pool       │  │  signal     │  │  register   │        │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
│                                                                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │   block/    │  │    api/     │  │  printk/    │  │   panic/    │        │
│  │  ─────────  │  │  ─────────  │  │  ─────────  │  │  ─────────  │        │
│  │  undo       │  │  stable     │  │  Logger     │  │  handler    │        │
│  │  history    │  │  versioned  │  │  pr_err!    │  │  recovery   │        │
│  │  transaction│  │  compat     │  │  pr_info!   │  │  kpanic!    │        │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
│                                                                            │
│  ┌─────────────┐                                                           │
│  │   debug/    │                                                           │
│  │  ─────────  │                                                           │
│  │  metrics    │                                                           │
│  │  profiler   │                                                           │
│  └─────────────┘                                                           │
└────────────────────────────────────────────────────────────────────────────┘
                                     │
                        ═════════════╪═════════════ Arch Interface
                                     │
┌────────────────────────────────────────────────────────────────────────────┐
│                            ARCH (lib/arch/)                                │
│                                                                            │
│  ┌───────────────────┐  ┌───────────────────┐  ┌───────────────────┐       │
│  │      unix/        │  │     windows/      │  │      wasm/        │       │
│  │  ───────────────  │  │  ───────────────  │  │  ───────────────  │       │
│  │  terminal.rs      │  │  console.rs       │  │  web_terminal.rs  │       │
│  │  signal.rs        │  │  win_signal.rs    │  │  (future)         │       │
│  │  pty.rs           │  │  conpty.rs        │  │                   │       │
│  │  mmap.rs          │  │  virtual_alloc.rs │  │                   │       │
│  └───────────────────┘  └───────────────────┘  └───────────────────┘       │
└────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Crate Dependency Graph

```
                                    runner
                                       │
                     ┌─────────────────┼─────────────────┐
                     │                 │                 │
                     ▼                 ▼                 ▼
              plugins/features   plugins/languages   lib/drivers/*
                     │                 │                 │
                     │                 │ (implements     │
                     │                 │  SyntaxDriver)  │
                     │                 │                 │
                     └────────┬────────┴────────┬────────┘
                              │                 │
                              ▼                 │
                     ┌─────────────────┐        │
                     │ lib/drivers/*   │◄───────┘
                     │ (trait crates)  │
                     │                 │
                     │ ┌─────────────┐ │
                     │ │display      │ │
                     │ │input        │ │
                     │ │syntax       │ │
                     │ │lsp          │ │
                     │ │net          │ │
                     │ │vfs          │ │
                     │ └─────────────┘ │
                     └────────┬────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │   lib/kernel/   │
                     │                 │
                     │ ┌─────────────┐ │
                     │ │sched        │ │
                     │ │mm           │ │
                     │ │ipc          │ │
                     │ │core         │ │
                     │ │block        │ │
                     │ │api          │ │
                     │ └─────────────┘ │
                     └────────┬────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │    lib/arch/    │
                     │                 │
                     │ ┌─────────────┐ │
                     │ │unix         │ │
                     │ │windows      │ │
                     │ │wasm (future)│ │
                     │ └─────────────┘ │
                     └─────────────────┘

Legend:
  ───▶  compile-time dependency
  - - ▶  runtime/dynamic dependency
```

### 3.3 Information Flow

```
User Input                                              Display Output
    │                                                        ▲
    ▼                                                        │
┌───────────┐     ┌───────────┐     ┌───────────┐     ┌───────────┐
│  Terminal │────▶│   Input   │────▶│  Kernel   │────▶│  Display  │────▶ Terminal
│  (arch)   │     │  Driver   │     │  (sched)  │     │  Driver   │
└───────────┘     └───────────┘     └───────────┘     └───────────┘
                       │                 │ ▲               │
                       │                 │ │               │
                       ▼                 ▼ │               ▼
                  ┌───────────┐     ┌───────────┐     ┌───────────┐
                  │  Keymap   │     │   Buffer  │     │   Frame   │
                  │  Module   │     │   (mm)    │     │  Buffer   │
                  └───────────┘     └───────────┘     └───────────┘
                                         │
                       ┌─────────────────┼─────────────────┐
                       │                 │                 │
                       ▼                 ▼                 ▼
                  ┌───────────┐     ┌───────────┐     ┌───────────┐
                  │  Syntax   │     │    LSP    │     │   VFS     │
                  │  Driver   │     │  Driver   │     │  Driver   │
                  └───────────┘     └───────────┘     └───────────┘
                       ▲                 ▲                 ▲
                       │                 │                 │
                  ┌───────────┐     ┌───────────┐     ┌───────────┐
                  │  Language │     │    LSP    │     │   File    │
                  │  Modules  │     │  Servers  │     │  System   │
                  └───────────┘     └───────────┘     └───────────┘
```

---

## 4. Layer Specifications

### 4.1 Architecture Layer (`lib/arch/`)

**Linux Equivalent:** `arch/x86/`, `arch/arm/`, etc.

**Purpose:** Abstract platform-specific details. Everything above this layer is platform-agnostic.

#### 4.1.1 Directory Structure

```
lib/arch/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── traits.rs           # Platform-agnostic traits
    ├── error.rs            # Arch-level errors
    │
    ├── unix/
    │   ├── mod.rs
    │   ├── terminal.rs     # Unix terminal I/O
    │   ├── signal.rs       # Signal handling (SIGWINCH, etc.)
    │   ├── pty.rs          # Pseudo-terminal support
    │   ├── mmap.rs         # Memory-mapped I/O
    │   └── poll.rs         # epoll/kqueue abstraction
    │
    ├── windows/
    │   ├── mod.rs
    │   ├── console.rs      # Windows Console API
    │   ├── signal.rs       # Windows signal emulation
    │   ├── conpty.rs       # ConPTY support
    │   └── virtual_alloc.rs # VirtualAlloc for buffers
    │
    └── wasm/               # Future: browser support
        ├── mod.rs
        └── web_terminal.rs # xterm.js integration
```

#### 4.1.2 Core Traits

```rust
// lib/arch/src/traits.rs

use std::io::{self, Read, Write};
use std::time::Duration;

/// Terminal backend abstraction
/// Linux equivalent: struct tty_driver
pub trait Terminal: Send + Sync {
    /// Get terminal dimensions
    fn size(&self) -> io::Result<TerminalSize>;

    /// Enable raw mode (disable line buffering, echo, etc.)
    fn enable_raw_mode(&mut self) -> io::Result<RawModeGuard>;

    /// Write bytes to terminal
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;

    /// Flush output buffer
    fn flush(&mut self) -> io::Result<()>;

    /// Check if output is a TTY
    fn is_tty(&self) -> bool;
}

/// Input source abstraction
/// Linux equivalent: struct input_dev
pub trait InputSource: Send + Sync {
    /// Poll for input with timeout
    fn poll(&mut self, timeout: Duration) -> io::Result<bool>;

    /// Read next input event (non-blocking after poll returns true)
    fn read_event(&mut self) -> io::Result<InputEvent>;

    /// Drain all pending events
    fn drain(&mut self) -> Vec<InputEvent>;
}

/// Raw input event from platform
#[derive(Debug, Clone)]
pub enum InputEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(TerminalSize),
    FocusGained,
    FocusLost,
    Paste(String),
}

/// Terminal size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
}

/// Key event with modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: Modifiers,
    pub kind: KeyEventKind,
}

/// Platform-agnostic key codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Char(char),
    F(u8),
    Backspace,
    Enter,
    Tab,
    Escape,
    Up, Down, Left, Right,
    Home, End,
    PageUp, PageDown,
    Insert, Delete,
    // ... etc
}

/// Signal handler abstraction
/// Linux equivalent: signal handlers
pub trait SignalHandler: Send + Sync {
    /// Register handler for terminal resize
    fn on_resize(&mut self, handler: Box<dyn Fn(TerminalSize) + Send + Sync>);

    /// Register handler for interrupt (Ctrl+C)
    fn on_interrupt(&mut self, handler: Box<dyn Fn() + Send + Sync>);

    /// Register handler for suspend (Ctrl+Z)
    fn on_suspend(&mut self, handler: Box<dyn Fn() + Send + Sync>);
}

/// Memory mapping abstraction
/// Linux equivalent: mmap/munmap
pub trait MemoryMapper: Send + Sync {
    /// Map file into memory
    fn map_file(&self, path: &Path, writable: bool) -> io::Result<MappedMemory>;

    /// Map anonymous memory region
    fn map_anonymous(&self, size: usize) -> io::Result<MappedMemory>;
}

/// RAII guard for raw mode
pub struct RawModeGuard {
    // Restores terminal state on drop
    _private: (),
}
```

#### 4.1.3 Unix Implementation

```rust
// lib/arch/src/unix/terminal.rs

use crate::traits::*;
use std::os::unix::io::{AsRawFd, RawFd};

pub struct UnixTerminal {
    stdin: RawFd,
    stdout: RawFd,
    original_termios: Option<libc::termios>,
}

impl UnixTerminal {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            stdin: std::io::stdin().as_raw_fd(),
            stdout: std::io::stdout().as_raw_fd(),
            original_termios: None,
        })
    }
}

impl Terminal for UnixTerminal {
    fn size(&self) -> io::Result<TerminalSize> {
        let mut winsize: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(self.stdout, libc::TIOCGWINSZ, &mut winsize) } == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(TerminalSize {
            cols: winsize.ws_col,
            rows: winsize.ws_row,
            pixel_width: winsize.ws_xpixel,
            pixel_height: winsize.ws_ypixel,
        })
    }

    fn enable_raw_mode(&mut self) -> io::Result<RawModeGuard> {
        let mut termios: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(self.stdin, &mut termios) } == -1 {
            return Err(io::Error::last_os_error());
        }

        self.original_termios = Some(termios);

        // Disable canonical mode, echo, signals
        termios.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG | libc::IEXTEN);
        termios.c_iflag &= !(libc::IXON | libc::ICRNL | libc::BRKINT | libc::INPCK | libc::ISTRIP);
        termios.c_oflag &= !libc::OPOST;
        termios.c_cflag |= libc::CS8;

        // Set read timeout
        termios.c_cc[libc::VMIN] = 0;
        termios.c_cc[libc::VTIME] = 0;

        if unsafe { libc::tcsetattr(self.stdin, libc::TCSAFLUSH, &termios) } == -1 {
            return Err(io::Error::last_os_error());
        }

        Ok(RawModeGuard { _private: () })
    }

    // ... other implementations
}

impl Drop for UnixTerminal {
    fn drop(&mut self) {
        if let Some(termios) = self.original_termios.take() {
            unsafe { libc::tcsetattr(self.stdin, libc::TCSAFLUSH, &termios) };
        }
    }
}
```

#### 4.1.4 Feature Flags

```toml
# lib/arch/Cargo.toml

[package]
name = "reovim-arch"
version = "0.1.0"
edition = "2024"

[features]
default = ["unix"]
unix = []
windows = ["windows-sys"]
wasm = ["wasm-bindgen", "web-sys"]

[dependencies]
# Common
libc = "0.2"

# Unix-specific
[target.'cfg(unix)'.dependencies]
nix = { version = "0.29", features = ["term", "signal", "poll"] }

# Windows-specific
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.52", features = ["Win32_System_Console"], optional = true }

# WASM-specific
[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = { version = "0.2", optional = true }
web-sys = { version = "0.3", optional = true }
```

---

### 4.2 Kernel Layer (`lib/kernel/`)

**Linux Equivalent:** `kernel/`, `mm/`, `ipc/`

**Purpose:** Core editor mechanisms - pure, allocation-efficient, no I/O.

#### 4.2.1 Directory Structure

```
lib/kernel/
├── Cargo.toml
└── src/
    ├── lib.rs
    │
    ├── api/                    # Stable kernel API (like include/linux/)
    │   ├── mod.rs
    │   ├── v1.rs              # API version 1 (stable)
    │   └── unstable.rs        # Unstable/experimental APIs
    │
    ├── sched/                  # Scheduler subsystem
    │   ├── mod.rs
    │   ├── runtime.rs         # Main runtime loop
    │   ├── executor.rs        # Async task executor
    │   ├── task.rs            # Task abstraction
    │   ├── priority.rs        # Priority queues
    │   ├── work_queue.rs      # Deferred work
    │   └── idle.rs            # Idle handling
    │
    ├── mm/                     # Memory management
    │   ├── mod.rs
    │   ├── buffer.rs          # Buffer struct (text storage)
    │   ├── rope.rs            # Rope data structure
    │   ├── piece_table.rs     # Alternative: piece table
    │   ├── gap_buffer.rs      # Alternative: gap buffer
    │   ├── cache.rs           # LRU cache primitives
    │   ├── pool.rs            # Object pool allocator
    │   ├── arena.rs           # Arena allocator
    │   └── cow.rs             # Copy-on-write utilities
    │
    ├── ipc/                    # Inter-process communication
    │   ├── mod.rs
    │   ├── event_bus.rs       # Type-erased event bus
    │   ├── channel.rs         # mpsc/broadcast channels
    │   ├── scope.rs           # EventScope (GC-like sync)
    │   ├── signal.rs          # Signal/notification system
    │   ├── semaphore.rs       # Counting semaphore
    │   └── barrier.rs         # Barrier synchronization
    │
    ├── core/                   # Core primitives
    │   ├── mod.rs
    │   ├── command.rs         # Command dispatch
    │   ├── motion.rs          # Cursor motion calculations
    │   ├── textobj.rs         # Text object primitives
    │   ├── register.rs        # Register (clipboard) operations
    │   ├── mark.rs            # Mark/bookmark operations
    │   ├── fold.rs            # Fold calculations
    │   ├── indent.rs          # Indentation logic
    │   └── search.rs          # Search/replace primitives
    │
    ├── block/                  # Block device operations
    │   ├── mod.rs
    │   ├── undo.rs            # Undo tree
    │   ├── history.rs         # Change history
    │   ├── transaction.rs     # Atomic transactions
    │   ├── journal.rs         # Write-ahead log
    │   └── snapshot.rs        # Buffer snapshots
    │
    ├── printk/                 # Kernel logging (like Linux kernel/printk/)
    │   ├── mod.rs             # Public API, re-exports
    │   ├── level.rs           # Level enum (Error, Warn, Info, Debug, Trace)
    │   ├── record.rs          # Record struct (level, message, file, line)
    │   ├── logger.rs          # Logger trait, NopLogger
    │   └── macros.rs          # pr_err!, pr_warn!, pr_info!, pr_debug!
    │
    ├── debug/                  # Debug utilities (separate from logging)
    │   ├── mod.rs
    │   ├── metrics.rs         # Performance metrics
    │   ├── profiler.rs        # Built-in profiler
    │   └── dump.rs            # State dumping
    │
    └── panic/                  # Panic handling
        ├── mod.rs
        ├── handler.rs         # Custom panic handler
        ├── recovery.rs        # Crash recovery
        └── report.rs          # Crash reporting
```

#### 4.2.2 Scheduler Subsystem (`sched/`)

**Linux Equivalent:** `kernel/sched/`

```rust
// lib/kernel/src/sched/runtime.rs

use crate::ipc::{EventBus, RuntimeEvent};
use crate::mm::BufferManager;

/// The kernel runtime - heart of the editor
/// Linux equivalent: struct task_struct for init process
pub struct Runtime {
    /// Buffer management
    buffers: BufferManager,

    /// Event bus for IPC
    event_bus: EventBus,

    /// Task executor
    executor: Executor,

    /// Work queue for deferred tasks
    work_queue: WorkQueue,

    /// Runtime state
    state: RuntimeState,
}

/// Runtime state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    /// Initializing subsystems
    Booting,
    /// Normal operation
    Running,
    /// Shutting down
    Stopping,
    /// Emergency shutdown (panic recovery)
    Emergency,
}

impl Runtime {
    /// Create new runtime
    pub fn new() -> Self {
        Self {
            buffers: BufferManager::new(),
            event_bus: EventBus::new(),
            executor: Executor::new(),
            work_queue: WorkQueue::new(),
            state: RuntimeState::Booting,
        }
    }

    /// Main event loop
    /// Linux equivalent: schedule() in kernel/sched/core.c
    pub async fn run(&mut self) -> Result<(), RuntimeError> {
        self.state = RuntimeState::Running;

        loop {
            // Process high-priority events
            while let Some(event) = self.event_bus.poll_high_priority() {
                self.handle_event(event).await?;
            }

            // Process normal events
            if let Some(event) = self.event_bus.poll() {
                self.handle_event(event).await?;
            }

            // Process deferred work
            self.work_queue.process_pending();

            // Check for shutdown
            if self.state == RuntimeState::Stopping {
                break;
            }

            // Yield to executor for async tasks
            self.executor.tick().await;
        }

        self.shutdown().await
    }

    /// Handle a runtime event
    async fn handle_event(&mut self, event: RuntimeEvent) -> Result<(), RuntimeError> {
        match event {
            RuntimeEvent::BufferEvent(be) => self.handle_buffer_event(be),
            RuntimeEvent::CommandEvent(ce) => self.handle_command_event(ce),
            RuntimeEvent::ModeChange(mc) => self.handle_mode_change(mc),
            RuntimeEvent::Render(signal) => self.handle_render(signal),
            RuntimeEvent::Kill => {
                self.state = RuntimeState::Stopping;
                Ok(())
            }
            // ... other events
        }
    }

    /// Schedule deferred work
    /// Linux equivalent: schedule_work()
    pub fn schedule_work(&self, work: impl FnOnce() + Send + 'static) {
        self.work_queue.push(Box::new(work));
    }

    /// Graceful shutdown
    async fn shutdown(&mut self) -> Result<(), RuntimeError> {
        // Flush all buffers
        self.buffers.flush_all().await?;

        // Wait for pending work
        self.work_queue.drain();

        // Shutdown executor
        self.executor.shutdown().await;

        Ok(())
    }
}
```

#### 4.2.3 Memory Management Subsystem (`mm/`)

**Linux Equivalent:** `mm/`

```rust
// lib/kernel/src/mm/buffer.rs

use crate::mm::rope::Rope;
use crate::block::{UndoTree, Transaction};
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique buffer identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferId(u64);

impl BufferId {
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

/// Buffer - fundamental text storage unit
/// Linux equivalent: struct inode (for file identity) + page cache (for content)
pub struct Buffer {
    /// Unique identifier
    id: BufferId,

    /// Text content (rope for efficient edits)
    content: Rope,

    /// Cursor positions (multiple cursors supported)
    cursors: Vec<Cursor>,

    /// Primary cursor index
    primary_cursor: usize,

    /// Undo tree
    undo: UndoTree,

    /// Current transaction (for grouping edits)
    transaction: Option<Transaction>,

    /// Modification count (for change detection)
    version: u64,

    /// Whether buffer has unsaved changes
    modified: bool,

    /// Associated file path (if any)
    file_path: Option<PathBuf>,

    /// File type (for syntax highlighting)
    file_type: Option<FileType>,

    /// Buffer-local marks
    marks: HashMap<char, Position>,
}

impl Buffer {
    /// Create new empty buffer
    pub fn new() -> Self {
        Self {
            id: BufferId::new(),
            content: Rope::new(),
            cursors: vec![Cursor::default()],
            primary_cursor: 0,
            undo: UndoTree::new(),
            transaction: None,
            version: 0,
            modified: false,
            file_path: None,
            file_type: None,
            marks: HashMap::new(),
        }
    }

    /// Create buffer from string
    pub fn from_string(s: impl Into<String>) -> Self {
        let mut buffer = Self::new();
        buffer.content = Rope::from(s.into());
        buffer
    }

    /// Insert text at position
    /// Returns the edit that was performed (for undo)
    pub fn insert(&mut self, pos: Position, text: &str) -> Edit {
        let byte_pos = self.position_to_byte(pos);
        self.content.insert(byte_pos, text);

        let edit = Edit::Insert {
            position: pos,
            text: text.to_string(),
        };

        self.record_edit(edit.clone());
        self.version += 1;
        self.modified = true;

        edit
    }

    /// Delete range
    pub fn delete(&mut self, range: Range<Position>) -> Edit {
        let start = self.position_to_byte(range.start);
        let end = self.position_to_byte(range.end);

        let deleted = self.content.slice(start..end).to_string();
        self.content.remove(start..end);

        let edit = Edit::Delete {
            range,
            text: deleted,
        };

        self.record_edit(edit.clone());
        self.version += 1;
        self.modified = true;

        edit
    }

    /// Begin a transaction (group multiple edits)
    pub fn begin_transaction(&mut self) {
        self.transaction = Some(Transaction::new());
    }

    /// Commit transaction
    pub fn commit_transaction(&mut self) {
        if let Some(txn) = self.transaction.take() {
            self.undo.push(txn);
        }
    }

    /// Undo last edit/transaction
    pub fn undo(&mut self) -> Option<Vec<Edit>> {
        self.undo.undo().map(|txn| {
            for edit in txn.edits.iter().rev() {
                self.apply_inverse(edit);
            }
            self.version += 1;
            txn.edits
        })
    }

    /// Redo
    pub fn redo(&mut self) -> Option<Vec<Edit>> {
        self.undo.redo().map(|txn| {
            for edit in &txn.edits {
                self.apply_edit(edit);
            }
            self.version += 1;
            txn.edits
        })
    }

    // ... line/position operations, etc.
}

/// Cursor within a buffer
#[derive(Debug, Clone, Copy, Default)]
pub struct Cursor {
    /// Current position
    pub position: Position,

    /// Anchor for selection (None = no selection)
    pub anchor: Option<Position>,

    /// Preferred column (for vertical movement)
    pub preferred_column: Option<usize>,
}

/// Position in buffer (line, column)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

/// Edit operation (for undo/redo)
#[derive(Debug, Clone)]
pub enum Edit {
    Insert { position: Position, text: String },
    Delete { range: Range<Position>, text: String },
}
```

#### 4.2.4 IPC Subsystem (`ipc/`)

**Linux Equivalent:** `ipc/`, kernel event mechanisms

```rust
// lib/kernel/src/ipc/event_bus.rs

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

/// Type-erased event trait
pub trait Event: Any + Send + Sync + 'static {
    /// Event priority (higher = processed first)
    fn priority(&self) -> u32 { 100 }

    /// Whether event can be batched with similar events
    fn batchable(&self) -> bool { false }
}

/// Event handler function type
type EventHandler = Box<dyn Fn(&dyn Any) + Send + Sync>;

/// Subscription handle (drop to unsubscribe)
pub struct Subscription {
    bus: Arc<EventBusInner>,
    type_id: TypeId,
    handler_id: u64,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.bus.unsubscribe(self.type_id, self.handler_id);
    }
}

/// Event bus for decoupled communication
/// Linux equivalent: notifier chains
pub struct EventBus {
    inner: Arc<EventBusInner>,
}

struct EventBusInner {
    /// Handlers by event type
    handlers: RwLock<HashMap<TypeId, Vec<(u64, u32, EventHandler)>>>,

    /// Next handler ID
    next_id: AtomicU64,

    /// Event queue (for async processing)
    queue: crossbeam_queue::SegQueue<Box<dyn Any + Send>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(EventBusInner {
                handlers: RwLock::new(HashMap::new()),
                next_id: AtomicU64::new(1),
                queue: crossbeam_queue::SegQueue::new(),
            }),
        }
    }

    /// Subscribe to event type
    pub fn subscribe<E: Event>(
        &self,
        priority: u32,
        handler: impl Fn(&E) + Send + Sync + 'static,
    ) -> Subscription {
        let type_id = TypeId::of::<E>();
        let handler_id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);

        let handler: EventHandler = Box::new(move |any| {
            if let Some(event) = any.downcast_ref::<E>() {
                handler(event);
            }
        });

        {
            let mut handlers = self.inner.handlers.write();
            let entry = handlers.entry(type_id).or_default();
            entry.push((handler_id, priority, handler));
            entry.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by priority descending
        }

        Subscription {
            bus: self.inner.clone(),
            type_id,
            handler_id,
        }
    }

    /// Emit event synchronously
    pub fn emit<E: Event>(&self, event: E) {
        let type_id = TypeId::of::<E>();
        let handlers = self.inner.handlers.read();

        if let Some(handlers) = handlers.get(&type_id) {
            for (_, _, handler) in handlers {
                handler(&event);
            }
        }
    }

    /// Emit event asynchronously (queued)
    pub fn emit_async<E: Event>(&self, event: E) {
        self.inner.queue.push(Box::new(event));
    }

    /// Process queued events
    pub fn process_queue(&self) {
        while let Some(event) = self.inner.queue.pop() {
            let type_id = (*event).type_id();
            let handlers = self.inner.handlers.read();

            if let Some(handlers) = handlers.get(&type_id) {
                for (_, _, handler) in handlers {
                    handler(&*event);
                }
            }
        }
    }
}

impl EventBusInner {
    fn unsubscribe(&self, type_id: TypeId, handler_id: u64) {
        let mut handlers = self.handlers.write();
        if let Some(handlers) = handlers.get_mut(&type_id) {
            handlers.retain(|(id, _, _)| *id != handler_id);
        }
    }
}
```

#### 4.2.5 Core Subsystem (`core/`)

**Linux Equivalent:** Core kernel functionality

```rust
// lib/kernel/src/core/motion.rs

use crate::mm::{Buffer, Position, Cursor};

/// Motion direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

/// Word boundary type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordBoundary {
    /// Word characters (alphanumeric + underscore)
    Word,
    /// WORD (non-whitespace)
    BigWord,
}

/// Motion types
/// Linux equivalent: cursor movement is a "mechanism"
#[derive(Debug, Clone, Copy)]
pub enum Motion {
    /// Character motion (h, l)
    Char(Direction),

    /// Line motion (j, k)
    Line(Direction),

    /// Word motion (w, b, e, ge)
    Word {
        direction: Direction,
        boundary: WordBoundary,
        end: bool, // true for 'e'/'ge', false for 'w'/'b'
    },

    /// Line position (0, ^, $)
    LinePosition(LinePosition),

    /// Paragraph motion ({, })
    Paragraph(Direction),

    /// Search motion (/, ?, n, N)
    Search {
        pattern: String,
        direction: Direction,
    },

    /// Find character (f, F, t, T)
    FindChar {
        char: char,
        direction: Direction,
        till: bool, // true for 't'/'T'
    },

    /// Jump to line (G, gg)
    JumpLine(Option<usize>),

    /// Jump to column
    JumpColumn(usize),

    /// Match bracket (%)
    MatchBracket,
}

/// Line position variants
#[derive(Debug, Clone, Copy)]
pub enum LinePosition {
    Start,      // 0
    FirstNonBlank, // ^
    End,        // $
    LastNonBlank,  // g_
}

/// Motion engine - pure calculation, no side effects
/// This is the "mechanism" - policies are in modules
pub struct MotionEngine;

impl MotionEngine {
    /// Calculate new position after motion
    /// Returns None if motion is invalid/impossible
    pub fn calculate(
        &self,
        motion: Motion,
        buffer: &Buffer,
        cursor: Cursor,
        count: usize,
    ) -> Option<Position> {
        let count = count.max(1);

        match motion {
            Motion::Char(dir) => self.char_motion(buffer, cursor.position, dir, count),
            Motion::Line(dir) => self.line_motion(buffer, cursor, dir, count),
            Motion::Word { direction, boundary, end } => {
                self.word_motion(buffer, cursor.position, direction, boundary, end, count)
            }
            Motion::LinePosition(pos) => self.line_position(buffer, cursor.position, pos),
            Motion::Paragraph(dir) => self.paragraph_motion(buffer, cursor.position, dir, count),
            Motion::FindChar { char, direction, till } => {
                self.find_char(buffer, cursor.position, char, direction, till, count)
            }
            Motion::JumpLine(line) => self.jump_line(buffer, cursor, line),
            Motion::MatchBracket => self.match_bracket(buffer, cursor.position),
            // ... other motions
        }
    }

    fn char_motion(
        &self,
        buffer: &Buffer,
        pos: Position,
        direction: Direction,
        count: usize,
    ) -> Option<Position> {
        let line_len = buffer.line_len(pos.line)?;

        match direction {
            Direction::Forward => {
                let new_col = pos.column.saturating_add(count);
                if new_col < line_len {
                    Some(Position { line: pos.line, column: new_col })
                } else {
                    Some(Position { line: pos.line, column: line_len.saturating_sub(1) })
                }
            }
            Direction::Backward => {
                let new_col = pos.column.saturating_sub(count);
                Some(Position { line: pos.line, column: new_col })
            }
        }
    }

    fn line_motion(
        &self,
        buffer: &Buffer,
        cursor: Cursor,
        direction: Direction,
        count: usize,
    ) -> Option<Position> {
        let line_count = buffer.line_count();

        let new_line = match direction {
            Direction::Forward => {
                let target = cursor.position.line.saturating_add(count);
                target.min(line_count.saturating_sub(1))
            }
            Direction::Backward => cursor.position.line.saturating_sub(count),
        };

        // Respect preferred column
        let preferred = cursor.preferred_column.unwrap_or(cursor.position.column);
        let line_len = buffer.line_len(new_line)?;
        let new_col = preferred.min(line_len.saturating_sub(1).max(0));

        Some(Position { line: new_line, column: new_col })
    }

    fn word_motion(
        &self,
        buffer: &Buffer,
        pos: Position,
        direction: Direction,
        boundary: WordBoundary,
        end: bool,
        count: usize,
    ) -> Option<Position> {
        let mut current = pos;

        for _ in 0..count {
            current = self.next_word_boundary(buffer, current, direction, boundary, end)?;
        }

        Some(current)
    }

    fn next_word_boundary(
        &self,
        buffer: &Buffer,
        pos: Position,
        direction: Direction,
        boundary: WordBoundary,
        end: bool,
    ) -> Option<Position> {
        let is_word_char = match boundary {
            WordBoundary::Word => |c: char| c.is_alphanumeric() || c == '_',
            WordBoundary::BigWord => |c: char| !c.is_whitespace(),
        };

        // Implementation: scan through characters...
        // This is pure calculation, no buffer modification
        todo!("word boundary calculation")
    }

    // ... other motion implementations
}

/// Text object types
#[derive(Debug, Clone, Copy)]
pub enum TextObject {
    /// Inner word (iw)
    InnerWord(WordBoundary),
    /// A word (aw)
    AWord(WordBoundary),
    /// Inner paragraph (ip)
    InnerParagraph,
    /// A paragraph (ap)
    AParagraph,
    /// Inner quotes (i", i')
    InnerQuote(char),
    /// A quotes (a", a')
    AQuote(char),
    /// Inner bracket (i(, i[, i{, i<)
    InnerBracket(char),
    /// A bracket (a(, a[, a{, a<)
    ABracket(char),
    /// Inner tag (it)
    InnerTag,
    /// A tag (at)
    ATag,
}

/// Text object engine
pub struct TextObjectEngine;

impl TextObjectEngine {
    /// Calculate range for text object
    pub fn range(
        &self,
        obj: TextObject,
        buffer: &Buffer,
        pos: Position,
        count: usize,
    ) -> Option<Range<Position>> {
        match obj {
            TextObject::InnerWord(boundary) => self.inner_word(buffer, pos, boundary, count),
            TextObject::AWord(boundary) => self.a_word(buffer, pos, boundary, count),
            TextObject::InnerBracket(bracket) => self.inner_bracket(buffer, pos, bracket, count),
            // ... other text objects
        }
    }

    // ... implementations
}
```

**Design Note: Virtual Text / Decoration Compatibility**

The `core/` module operates exclusively on **buffer positions** (actual text content), not
visual/screen positions. This is intentional:

```
Buffer:     let x = 5;
Visual:     let x: i32 = 5;   // ": i32" is virtual text (type hint)
                ^^^^^ not in buffer, handled by render Stage 4
```

- **MotionEngine**: Calculates positions in actual buffer content
- **TextObjectEngine**: Finds ranges in actual buffer content
- **Virtual text**: Handled in render pipeline (Stage 4: Decorations)

This separation ensures:
1. Motions operate on real text (`w` jumps to next real word, not virtual text)
2. Text objects select real content (operators act on buffer, not decorations)
3. The kernel provides "mechanisms"; rendering provides "visual policies"

Position mapping between buffer and visual coordinates is handled by the display
driver (`lib/drivers/display/`), not the kernel. See Stage 4 in the rendering
pipeline (section 5.3) for decoration/virtual text application.

#### 4.2.6 Kernel API (`api/`)

**Linux Equivalent:** `include/linux/` (stable interfaces)

```rust
// lib/kernel/src/api/v1.rs

//! Stable Kernel API v1
//!
//! This module defines the stable interface that drivers and modules
//! can depend on. Breaking changes require a new API version.
//!
//! Linux equivalent: EXPORT_SYMBOL_GPL

pub use crate::mm::{Buffer, BufferId, Position, Cursor, Edit};
pub use crate::ipc::{EventBus, Event, Subscription};
pub use crate::core::{Motion, MotionEngine, TextObject, TextObjectEngine};
pub use crate::block::{UndoTree, Transaction};
pub use crate::sched::{Runtime, RuntimeState};

/// API version for compatibility checking
pub const API_VERSION: (u32, u32, u32) = (1, 0, 0);

/// Kernel context provided to drivers and modules
#[derive(Clone)]
pub struct KernelContext {
    /// Event bus for IPC
    pub event_bus: Arc<EventBus>,

    /// Buffer manager
    pub buffers: Arc<BufferManager>,

    /// Motion engine
    pub motion: Arc<MotionEngine>,

    /// Text object engine
    pub text_objects: Arc<TextObjectEngine>,
}

/// Buffer manager interface
pub trait BufferManager: Send + Sync {
    /// Get buffer by ID
    fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>>;

    /// Create new buffer
    fn create(&self) -> BufferId;

    /// Create buffer from file
    fn open(&self, path: &Path) -> io::Result<BufferId>;

    /// Save buffer to file
    fn save(&self, id: BufferId, path: Option<&Path>) -> io::Result<()>;

    /// Close buffer
    fn close(&self, id: BufferId) -> bool;

    /// List all buffers
    fn list(&self) -> Vec<BufferId>;
}
```

---

### 4.3 Driver Layer (`lib/drivers/`)

**Linux Equivalent:** `drivers/`

Each driver is a **separate crate** for independent versioning and compilation.

#### 4.3.1 Driver Crate Overview

```
lib/drivers/
├── display/            # Display driver
│   ├── Cargo.toml
│   └── src/
├── input/              # Input driver
│   ├── Cargo.toml
│   └── src/
├── syntax/             # Syntax highlighting driver
│   ├── Cargo.toml
│   └── src/
├── lsp/                # Language Server Protocol driver
│   ├── Cargo.toml
│   └── src/
├── net/                # Network/RPC driver
│   ├── Cargo.toml
│   └── src/
└── vfs/                # Virtual filesystem driver
    ├── Cargo.toml
    └── src/
```

#### 4.3.2 Display Driver (`lib/drivers/display/`)

```
lib/drivers/display/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── traits.rs           # Display driver trait
    │
    ├── frame/              # Frame buffer management
    │   ├── mod.rs
    │   ├── buffer.rs       # 2D cell grid
    │   ├── cell.rs         # Cell (char + style)
    │   ├── diff.rs         # Diff algorithm
    │   └── renderer.rs     # Double-buffer renderer
    │
    ├── term/               # Terminal rendering
    │   ├── mod.rs
    │   ├── ansi.rs         # ANSI escape codes
    │   ├── sixel.rs        # Sixel graphics (future)
    │   └── kitty.rs        # Kitty protocol (future)
    │
    ├── compositor/         # Layer compositing
    │   ├── mod.rs
    │   ├── layer.rs        # Layer abstraction
    │   ├── zorder.rs       # Z-ordering
    │   └── blend.rs        # Blending modes
    │
    └── window/             # Window management
        ├── mod.rs
        ├── layout.rs       # Window layout
        ├── split.rs        # Split management
        └── tab.rs          # Tab management
```

```rust
// lib/drivers/display/src/traits.rs

use reovim_kernel::api::v1::*;

/// Display driver trait
/// Linux equivalent: struct fb_ops (framebuffer operations)
pub trait DisplayDriver: Send + Sync {
    /// Initialize display
    fn init(&mut self, ctx: &KernelContext) -> Result<(), DisplayError>;

    /// Get frame buffer for rendering
    fn frame_buffer(&mut self) -> &mut FrameBuffer;

    /// Render frame buffer to terminal
    fn render(&mut self) -> Result<(), DisplayError>;

    /// Handle resize event
    fn resize(&mut self, size: TerminalSize) -> Result<(), DisplayError>;

    /// Get current size
    fn size(&self) -> TerminalSize;

    /// Shutdown display
    fn shutdown(&mut self) -> Result<(), DisplayError>;
}

/// Window manager trait
pub trait WindowManager: Send + Sync {
    /// Get active window
    fn active_window(&self) -> Option<WindowId>;

    /// Set active window
    fn set_active(&mut self, id: WindowId);

    /// Create window for buffer
    fn create_window(&mut self, buffer_id: BufferId) -> WindowId;

    /// Close window
    fn close_window(&mut self, id: WindowId) -> bool;

    /// Split window
    fn split(&mut self, id: WindowId, direction: SplitDirection) -> WindowId;

    /// Get window bounds
    fn bounds(&self, id: WindowId) -> Option<Rect>;

    /// Layout all windows
    fn layout(&mut self, available: Rect);
}
```

#### 4.3.3 Input Driver (`lib/drivers/input/`)

```
lib/drivers/input/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── traits.rs           # Input driver trait
    │
    ├── keyboard/           # Keyboard handling
    │   ├── mod.rs
    │   ├── broker.rs       # Key event broker
    │   ├── keymap.rs       # Key notation parsing
    │   └── repeat.rs       # Key repeat handling
    │
    ├── mouse/              # Mouse handling
    │   ├── mod.rs
    │   ├── broker.rs       # Mouse event broker
    │   └── gesture.rs      # Gesture recognition
    │
    └── clipboard/          # Clipboard handling
        ├── mod.rs
        └── backend.rs      # System clipboard
```

```rust
// lib/drivers/input/src/traits.rs

use reovim_kernel::api::v1::*;
use reovim_arch::{InputEvent, KeyEvent, MouseEvent};

/// Input driver trait
/// Linux equivalent: struct input_handler
pub trait InputDriver: Send + Sync {
    /// Initialize input handling
    fn init(&mut self, ctx: &KernelContext) -> Result<(), InputError>;

    /// Start input processing (spawns async task)
    fn start(&mut self) -> Result<(), InputError>;

    /// Stop input processing
    fn stop(&mut self) -> Result<(), InputError>;

    /// Inject key event (for testing/RPC)
    fn inject_key(&mut self, event: KeyEvent);

    /// Inject mouse event
    fn inject_mouse(&mut self, event: MouseEvent);
}

/// Key event handler trait
pub trait KeyHandler: Send + Sync {
    /// Handle key event
    /// Returns true if event was consumed
    fn handle_key(&mut self, event: &KeyEvent, ctx: &KernelContext) -> bool;

    /// Priority (higher = called first)
    fn priority(&self) -> u32 { 100 }
}

/// Keymap registry
pub trait KeymapRegistry: Send + Sync {
    /// Register key binding
    fn bind(&mut self, keys: &str, command_id: CommandId);

    /// Unbind key
    fn unbind(&mut self, keys: &str);

    /// Lookup binding
    fn lookup(&self, keys: &[KeyEvent]) -> KeymapResult;
}

/// Keymap lookup result
pub enum KeymapResult {
    /// Complete match found
    Match(CommandId),
    /// Partial match (prefix of longer binding)
    Prefix,
    /// No match
    None,
}
```

#### 4.3.4 Syntax Driver (`lib/drivers/syntax/`)

**CRITICAL: NO tree-sitter dependency here!**

```
lib/drivers/syntax/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── traits.rs           # Syntax driver trait (NO tree-sitter!)
    ├── types.rs            # Highlight types
    │
    ├── registry/           # Language registry
    │   ├── mod.rs
    │   ├── loader.rs       # Dynamic module loader
    │   └── discovery.rs    # Language discovery
    │
    ├── cache/              # Highlight caching
    │   ├── mod.rs
    │   ├── line_cache.rs   # Per-line cache
    │   └── invalidation.rs # Cache invalidation
    │
    └── injection/          # Language injection
        ├── mod.rs
        └── types.rs
```

```rust
// lib/drivers/syntax/src/traits.rs

//! Syntax driver trait definitions
//!
//! IMPORTANT: This crate must NOT depend on tree-sitter!
//! tree-sitter is an implementation detail of language modules.

use reovim_kernel::api::v1::*;
use std::ops::Range;

/// Syntax driver trait - implemented by language modules
/// Linux equivalent: struct file_operations (but for syntax)
pub trait SyntaxDriver: Send + Sync {
    /// Get language name
    fn language(&self) -> &str;

    /// Parse/re-parse buffer content
    fn parse(&mut self, content: &str);

    /// Incremental update after edit
    fn update(&mut self, edit: &SyntaxEdit);

    /// Get highlight spans for range
    fn highlights(&self, range: Range<usize>) -> Vec<HighlightSpan>;

    /// Get injection points for embedded languages
    fn injections(&self) -> Vec<Injection>;

    /// Get fold ranges
    fn folds(&self) -> Vec<FoldRange>;

    /// Get indentation for line
    fn indent_for(&self, line: usize) -> Option<usize>;
}

/// Edit for incremental parsing
#[derive(Debug, Clone)]
pub struct SyntaxEdit {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_end_byte: usize,
    pub start_position: (usize, usize),
    pub old_end_position: (usize, usize),
    pub new_end_position: (usize, usize),
}

/// Highlight span
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub range: Range<usize>,
    pub highlight: HighlightGroup,
}

/// Highlight groups (semantic tokens)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightGroup {
    // Keywords
    Keyword,
    KeywordControl,
    KeywordOperator,
    KeywordFunction,
    KeywordType,

    // Types
    Type,
    TypeBuiltin,

    // Functions
    Function,
    FunctionBuiltin,
    FunctionMacro,
    Method,

    // Variables
    Variable,
    VariableBuiltin,
    Parameter,
    Field,

    // Literals
    String,
    StringEscape,
    Character,
    Number,
    Boolean,

    // Comments
    Comment,
    CommentDoc,

    // Punctuation
    Punctuation,
    PunctuationBracket,
    PunctuationDelimiter,

    // Operators
    Operator,

    // Special
    Error,
    Warning,
    Info,
    Hint,

    // Custom (for plugins)
    Custom(u32),
}

/// Language injection point
#[derive(Debug, Clone)]
pub struct Injection {
    pub range: Range<usize>,
    pub language: String,
    pub content_range: Range<usize>,
}

/// Fold range
#[derive(Debug, Clone)]
pub struct FoldRange {
    pub start_line: usize,
    pub end_line: usize,
    pub kind: FoldKind,
}

#[derive(Debug, Clone, Copy)]
pub enum FoldKind {
    Region,
    Imports,
    Comment,
}

/// Syntax driver factory - for dynamic loading
pub trait SyntaxDriverFactory: Send + Sync {
    /// Create new syntax driver instance
    fn create(&self) -> Box<dyn SyntaxDriver>;

    /// Get language info
    fn info(&self) -> LanguageInfo;
}

/// Language information
#[derive(Debug, Clone)]
pub struct LanguageInfo {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub mime_types: &'static [&'static str],
    pub comment_tokens: Option<CommentTokens>,
}

#[derive(Debug, Clone)]
pub struct CommentTokens {
    pub line: Option<&'static str>,
    pub block_start: Option<&'static str>,
    pub block_end: Option<&'static str>,
}

/// Language registry - manages syntax drivers
pub trait LanguageRegistry: Send + Sync {
    /// Register language from factory
    fn register(&mut self, factory: Box<dyn SyntaxDriverFactory>);

    /// Get driver for language
    fn get(&self, language: &str) -> Option<Box<dyn SyntaxDriver>>;

    /// Get driver for file extension
    fn get_for_extension(&self, ext: &str) -> Option<Box<dyn SyntaxDriver>>;

    /// List registered languages
    fn list(&self) -> Vec<&LanguageInfo>;

    /// Load language module dynamically
    fn load_module(&mut self, path: &Path) -> Result<(), ModuleError>;

    /// Unload language module
    fn unload_module(&mut self, language: &str) -> Result<(), ModuleError>;
}
```

#### 4.3.5 LSP Driver (`lib/drivers/lsp/`)

```
lib/drivers/lsp/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── traits.rs           # LSP driver trait
    │
    ├── client/             # LSP client
    │   ├── mod.rs
    │   ├── connection.rs   # Server connection
    │   ├── capabilities.rs # Capability negotiation
    │   └── lifecycle.rs    # Server lifecycle
    │
    ├── protocol/           # LSP protocol
    │   ├── mod.rs
    │   ├── messages.rs     # Request/response types
    │   └── notifications.rs # Notification types
    │
    ├── diagnostics/        # Diagnostic handling
    │   ├── mod.rs
    │   └── renderer.rs     # Diagnostic rendering
    │
    └── saturator/          # Background processing
        ├── mod.rs
        └── queue.rs        # Request queue
```

#### 4.3.6 Network Driver (`lib/drivers/net/`)

```
lib/drivers/net/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── traits.rs           # Network driver trait
    │
    ├── rpc/                # JSON-RPC server
    │   ├── mod.rs
    │   ├── server.rs       # RPC server
    │   ├── handler.rs      # Request handlers
    │   └── types.rs        # RPC types
    │
    └── transport/          # Transport layer
        ├── mod.rs
        ├── tcp.rs          # TCP transport
        ├── unix.rs         # Unix socket
        └── stdio.rs        # Stdio transport
```

#### 4.3.7 VFS Driver (`lib/drivers/vfs/`)

```
lib/drivers/vfs/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── traits.rs           # VFS trait
    │
    ├── local/              # Local filesystem
    │   ├── mod.rs
    │   └── watcher.rs      # File watcher
    │
    ├── ops/                # File operations
    │   ├── mod.rs
    │   ├── read.rs
    │   └── write.rs
    │
    └── path/               # Path handling
        ├── mod.rs
        └── normalize.rs
```

---

### 4.4 Module System

**Linux Equivalent:** Loadable Kernel Modules (LKM)

#### 4.4.1 Module Structure

```
plugins/
├── features/               # Feature modules
│   ├── completion/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── module.rs   # Module definition
│   ├── explorer/
│   ├── telescope/
│   └── ...
│
└── languages/              # Language modules (implement SyntaxDriver)
    ├── rust/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── module.rs   # Module definition
    │       └── syntax.rs   # SyntaxDriver implementation
    ├── c/
    ├── python/
    └── ...
```

#### 4.4.2 Module Trait

```rust
// In lib/kernel/src/api/v1.rs (or a separate module crate)

/// Module trait - all modules must implement this
/// Linux equivalent: struct module
pub trait Module: Send + Sync + 'static {
    /// Unique module identifier
    fn id(&self) -> ModuleId;

    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Version
    fn version(&self) -> Version;

    /// Required API version
    fn api_version(&self) -> (u32, u32, u32) {
        crate::API_VERSION
    }

    /// Dependencies (other module IDs)
    fn dependencies(&self) -> Vec<ModuleId> {
        vec![]
    }

    /// Initialize module
    /// Linux equivalent: module_init()
    fn init(&mut self, ctx: &ModuleContext) -> Result<(), ModuleError>;

    /// Cleanup module
    /// Linux equivalent: module_exit()
    fn exit(&mut self) -> Result<(), ModuleError>;

    /// Register commands
    fn commands(&self) -> Vec<CommandRegistration> {
        vec![]
    }

    /// Register keybindings
    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![]
    }

    /// Register event handlers
    fn subscribe(&self, bus: &EventBus) -> Vec<Subscription> {
        vec![]
    }
}

/// Module context provided during initialization
pub struct ModuleContext {
    /// Kernel context
    pub kernel: KernelContext,

    /// Driver registry (for accessing drivers)
    pub drivers: Arc<DriverRegistry>,

    /// Module registry (for accessing other modules)
    pub modules: Arc<ModuleRegistry>,

    /// Configuration for this module
    pub config: ModuleConfig,
}

/// Module identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleId(pub &'static str);

/// Module version
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}
```

---

## 5. Dynamic Module Loading

**Linux Equivalent:** `insmod`, `rmmod`, `modprobe`

### 5.1 Architecture

```
┌────────────────────────────────────────────────────────────────┐
│                      Module Manager                            │
│                                                                │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │  Module Loader  │  │ Module Registry │  │ Dependency      │ │
│  │  (.so/.dll)     │  │ (loaded modules)│  │ Resolver        │ │
│  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘ │
│           │                    │                    │          │
└───────────┼────────────────────┼────────────────────┼──────────┘
            │                    │                    │
            ▼                    ▼                    ▼
┌────────────────────────────────────────────────────────────────┐
│                       Module Discovery                         │
│                                                                │
│  ~/.config/reovim/modules/     System modules                  │
│  ~/.local/share/reovim/modules/ User modules                   │
│  ./plugins/                     Project modules                │
└────────────────────────────────────────────────────────────────┘
```

### 5.2 Dynamic Loading Implementation

```rust
// lib/kernel/src/module/loader.rs

use libloading::{Library, Symbol};
use std::path::Path;
use std::collections::HashMap;

/// Symbol name for module entry point
const MODULE_ENTRY: &[u8] = b"reovim_module_entry";

/// Module entry function type
/// Linux equivalent: init_module()
type ModuleEntryFn = unsafe extern "C" fn() -> *mut dyn Module;

/// Dynamic module loader
/// Linux equivalent: kernel module loader
pub struct ModuleLoader {
    /// Loaded libraries (kept alive for symbols)
    libraries: HashMap<ModuleId, Library>,

    /// Loaded modules
    modules: HashMap<ModuleId, Box<dyn Module>>,

    /// Search paths
    search_paths: Vec<PathBuf>,
}

impl ModuleLoader {
    pub fn new() -> Self {
        Self {
            libraries: HashMap::new(),
            modules: HashMap::new(),
            search_paths: vec![
                // System modules
                PathBuf::from("/usr/lib/reovim/modules"),
                // User modules
                dirs::data_local_dir()
                    .unwrap_or_default()
                    .join("reovim/modules"),
                // Config modules
                dirs::config_dir()
                    .unwrap_or_default()
                    .join("reovim/modules"),
            ],
        }
    }

    /// Load module from file
    /// Linux equivalent: insmod
    pub unsafe fn load(&mut self, path: &Path) -> Result<ModuleId, ModuleError> {
        // Load shared library
        let library = Library::new(path)
            .map_err(|e| ModuleError::LoadFailed(e.to_string()))?;

        // Get entry point
        let entry: Symbol<ModuleEntryFn> = library
            .get(MODULE_ENTRY)
            .map_err(|e| ModuleError::NoEntryPoint(e.to_string()))?;

        // Call entry point to get module
        let module_ptr = entry();
        if module_ptr.is_null() {
            return Err(ModuleError::InitFailed("entry returned null".into()));
        }

        let module = Box::from_raw(module_ptr);
        let id = module.id();

        // Check API version compatibility
        let (major, minor, _) = module.api_version();
        let (our_major, our_minor, _) = crate::api::API_VERSION;
        if major != our_major || minor > our_minor {
            return Err(ModuleError::IncompatibleVersion {
                module: (major, minor),
                kernel: (our_major, our_minor),
            });
        }

        // Store library and module
        self.libraries.insert(id.clone(), library);
        self.modules.insert(id.clone(), module);

        Ok(id)
    }

    /// Unload module
    /// Linux equivalent: rmmod
    pub fn unload(&mut self, id: &ModuleId) -> Result<(), ModuleError> {
        // Check if other modules depend on this one
        for (other_id, module) in &self.modules {
            if module.dependencies().contains(id) {
                return Err(ModuleError::InUse {
                    module: id.clone(),
                    by: other_id.clone(),
                });
            }
        }

        // Call module exit
        if let Some(mut module) = self.modules.remove(id) {
            module.exit()?;
        }

        // Drop library (unloads shared object)
        self.libraries.remove(id);

        Ok(())
    }

    /// Load module by name (searches paths)
    /// Linux equivalent: modprobe
    pub unsafe fn load_by_name(&mut self, name: &str) -> Result<ModuleId, ModuleError> {
        let filename = format!("lib{}.so", name); // Linux
        // let filename = format!("{}.dll", name); // Windows

        for path in &self.search_paths {
            let full_path = path.join(&filename);
            if full_path.exists() {
                return self.load(&full_path);
            }
        }

        Err(ModuleError::NotFound(name.to_string()))
    }

    /// Initialize loaded module
    pub fn init_module(
        &mut self,
        id: &ModuleId,
        ctx: &ModuleContext,
    ) -> Result<(), ModuleError> {
        // Resolve dependencies first
        let module = self.modules.get(id)
            .ok_or_else(|| ModuleError::NotLoaded(id.clone()))?;

        for dep in module.dependencies() {
            if !self.modules.contains_key(&dep) {
                // Try to load dependency
                unsafe { self.load_by_name(&dep.0)?; }
            }
            // Recursively init dependency
            self.init_module(&dep, ctx)?;
        }

        // Init this module
        let module = self.modules.get_mut(id).unwrap();
        module.init(ctx)
    }
}

/// Module entry macro - generates entry point
#[macro_export]
macro_rules! declare_module {
    ($module:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn reovim_module_entry() -> *mut dyn $crate::Module {
            let module: Box<dyn $crate::Module> = Box::new(<$module>::new());
            Box::into_raw(module)
        }
    };
}
```

### 5.3 Language Module Example

```rust
// plugins/languages/rust/src/lib.rs

use reovim_kernel::api::v1::*;
use reovim_drivers::syntax::*;

mod syntax;

pub struct RustLanguageModule {
    driver: Option<syntax::RustSyntaxDriver>,
}

impl RustLanguageModule {
    pub fn new() -> Self {
        Self { driver: None }
    }
}

impl Module for RustLanguageModule {
    fn id(&self) -> ModuleId {
        ModuleId("lang-rust")
    }

    fn name(&self) -> &'static str {
        "Rust Language Support"
    }

    fn version(&self) -> Version {
        Version { major: 1, minor: 0, patch: 0 }
    }

    fn init(&mut self, ctx: &ModuleContext) -> Result<(), ModuleError> {
        // Register with syntax driver registry
        let registry = ctx.drivers.get::<dyn LanguageRegistry>()?;

        registry.register(Box::new(RustDriverFactory));

        Ok(())
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

struct RustDriverFactory;

impl SyntaxDriverFactory for RustDriverFactory {
    fn create(&self) -> Box<dyn SyntaxDriver> {
        Box::new(syntax::RustSyntaxDriver::new())
    }

    fn info(&self) -> LanguageInfo {
        LanguageInfo {
            name: "rust",
            extensions: &["rs"],
            mime_types: &["text/x-rust"],
            comment_tokens: Some(CommentTokens {
                line: Some("//"),
                block_start: Some("/*"),
                block_end: Some("*/"),
            }),
        }
    }
}

// Generate entry point
reovim_kernel::declare_module!(RustLanguageModule);
```

```rust
// plugins/languages/rust/src/syntax.rs

use reovim_drivers::syntax::*;
use tree_sitter::{Parser, Tree, Language};

// tree-sitter is ONLY used here, not in kernel or drivers!
extern "C" { fn tree_sitter_rust() -> Language; }

pub struct RustSyntaxDriver {
    parser: Parser,
    tree: Option<Tree>,
    // Highlight query
    highlights: tree_sitter::Query,
}

impl RustSyntaxDriver {
    pub fn new() -> Self {
        let language = unsafe { tree_sitter_rust() };
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();

        let highlights = tree_sitter::Query::new(
            &language,
            include_str!("../queries/highlights.scm"),
        ).unwrap();

        Self {
            parser,
            tree: None,
            highlights,
        }
    }
}

impl SyntaxDriver for RustSyntaxDriver {
    fn language(&self) -> &str {
        "rust"
    }

    fn parse(&mut self, content: &str) {
        self.tree = self.parser.parse(content, None);
    }

    fn update(&mut self, edit: &SyntaxEdit) {
        if let Some(tree) = &mut self.tree {
            tree.edit(&tree_sitter::InputEdit {
                start_byte: edit.start_byte,
                old_end_byte: edit.old_end_byte,
                new_end_byte: edit.new_end_byte,
                start_position: tree_sitter::Point {
                    row: edit.start_position.0,
                    column: edit.start_position.1,
                },
                old_end_position: tree_sitter::Point {
                    row: edit.old_end_position.0,
                    column: edit.old_end_position.1,
                },
                new_end_position: tree_sitter::Point {
                    row: edit.new_end_position.0,
                    column: edit.new_end_position.1,
                },
            });
        }
    }

    fn highlights(&self, range: Range<usize>) -> Vec<HighlightSpan> {
        let tree = match &self.tree {
            Some(t) => t,
            None => return vec![],
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        cursor.set_byte_range(range.clone());

        let mut spans = vec![];

        for match_ in cursor.matches(&self.highlights, tree.root_node(), |_| "") {
            for capture in match_.captures {
                let node = capture.node;
                let capture_name = &self.highlights.capture_names()[capture.index as usize];

                if let Some(group) = capture_name_to_highlight_group(capture_name) {
                    spans.push(HighlightSpan {
                        range: node.byte_range(),
                        highlight: group,
                    });
                }
            }
        }

        spans
    }

    fn injections(&self) -> Vec<Injection> {
        // Rust doc comments can contain markdown
        // Implementation would scan for /// and /** */
        vec![]
    }

    fn folds(&self) -> Vec<FoldRange> {
        let tree = match &self.tree {
            Some(t) => t,
            None => return vec![],
        };

        let mut folds = vec![];

        // Walk tree looking for foldable nodes
        let mut cursor = tree.walk();
        self.collect_folds(&mut cursor, &mut folds);

        folds
    }

    fn indent_for(&self, _line: usize) -> Option<usize> {
        // Could use tree-sitter to determine indentation
        None
    }
}

fn capture_name_to_highlight_group(name: &str) -> Option<HighlightGroup> {
    match name {
        "keyword" => Some(HighlightGroup::Keyword),
        "keyword.control" => Some(HighlightGroup::KeywordControl),
        "keyword.function" => Some(HighlightGroup::KeywordFunction),
        "type" => Some(HighlightGroup::Type),
        "type.builtin" => Some(HighlightGroup::TypeBuiltin),
        "function" => Some(HighlightGroup::Function),
        "function.builtin" => Some(HighlightGroup::FunctionBuiltin),
        "function.macro" => Some(HighlightGroup::FunctionMacro),
        "variable" => Some(HighlightGroup::Variable),
        "variable.builtin" => Some(HighlightGroup::VariableBuiltin),
        "string" => Some(HighlightGroup::String),
        "string.escape" => Some(HighlightGroup::StringEscape),
        "number" => Some(HighlightGroup::Number),
        "comment" => Some(HighlightGroup::Comment),
        "comment.doc" => Some(HighlightGroup::CommentDoc),
        "operator" => Some(HighlightGroup::Operator),
        "punctuation.bracket" => Some(HighlightGroup::PunctuationBracket),
        "punctuation.delimiter" => Some(HighlightGroup::PunctuationDelimiter),
        _ => None,
    }
}
```

### 5.4 Hot Reload Support

```rust
// lib/kernel/src/module/hot_reload.rs

use notify::{Watcher, RecursiveMode, watcher};
use std::sync::mpsc::channel;

/// Hot reload manager for development
pub struct HotReloadManager {
    loader: Arc<RwLock<ModuleLoader>>,
    watcher: notify::RecommendedWatcher,
    watched_paths: HashMap<PathBuf, ModuleId>,
}

impl HotReloadManager {
    pub fn new(loader: Arc<RwLock<ModuleLoader>>) -> Result<Self, notify::Error> {
        let (tx, rx) = channel();
        let watcher = watcher(tx, Duration::from_secs(1))?;

        // Spawn reload handler
        let loader_clone = loader.clone();
        std::thread::spawn(move || {
            for event in rx {
                if let Ok(notify::Event { kind: notify::EventKind::Modify(_), paths, .. }) = event {
                    for path in paths {
                        Self::handle_change(&loader_clone, &path);
                    }
                }
            }
        });

        Ok(Self {
            loader,
            watcher,
            watched_paths: HashMap::new(),
        })
    }

    /// Watch module file for changes
    pub fn watch(&mut self, path: &Path, id: ModuleId) -> Result<(), notify::Error> {
        self.watcher.watch(path, RecursiveMode::NonRecursive)?;
        self.watched_paths.insert(path.to_path_buf(), id);
        Ok(())
    }

    fn handle_change(loader: &Arc<RwLock<ModuleLoader>>, path: &Path) {
        let mut loader = loader.write();

        // Find module for this path
        // ... lookup and reload logic

        log::info!("Hot-reloaded module: {:?}", path);
    }
}
```

---

## 6. Interface Contracts

### 6.1 Layer Dependency Rules

```
Rule 1: arch/ depends on NOTHING (except std)
Rule 2: kernel/ depends only on arch/
Rule 3: drivers/* depend only on kernel/ (and arch/ transitively)
Rule 4: plugins/* depend on drivers/* and kernel/
Rule 5: runner/ depends on everything

NO EXCEPTIONS.
```

```toml
# Example: lib/drivers/syntax/Cargo.toml

[package]
name = "reovim-drivers-syntax"
version = "0.1.0"

[dependencies]
# ALLOWED: kernel API
reovim-kernel = { path = "../../kernel" }

# FORBIDDEN: tree-sitter (implementation detail of modules)
# tree-sitter = "..."  # DO NOT ADD

# FORBIDDEN: other drivers (use events for communication)
# reovim-drivers-display = "..."  # DO NOT ADD

# ALLOWED: utility crates
parking_lot = "0.12"
arc-swap = "1.6"
```

### 6.2 API Stability Contract

```rust
// lib/kernel/src/api/mod.rs

/// API stability levels
/// Linux equivalent: EXPORT_SYMBOL vs EXPORT_SYMBOL_GPL

/// Stable API - no breaking changes within major version
#[doc(cfg(stable))]
pub mod v1;

/// Unstable API - may change without notice
#[doc(cfg(unstable))]
pub mod unstable;

/// Internal API - for kernel use only, modules should NOT use
#[doc(hidden)]
pub mod internal;
```

### 6.3 Error Handling Contract

```rust
// Error types must be defined per layer

// arch errors
pub enum ArchError {
    Io(std::io::Error),
    NotSupported(&'static str),
    // ...
}

// kernel errors
pub enum KernelError {
    Arch(ArchError),
    BufferNotFound(BufferId),
    InvalidPosition(Position),
    // ...
}

// driver errors
pub enum DriverError {
    Kernel(KernelError),
    NotInitialized,
    // driver-specific errors
}

// module errors
pub enum ModuleError {
    Driver(DriverError),
    LoadFailed(String),
    InitFailed(String),
    // module-specific errors
}
```

---

## 7. Subsystem Deep Dives

### 7.1 Event Flow Refactored

```
Current (problematic):
┌─────────┐     ┌─────────────────┐     ┌─────────┐
│ Terminal│────▶│ Runtime (god)   │────▶│ Screen  │
└─────────┘     │ - buffers       │     └─────────┘
                │ - events        │
                │ - rendering     │
                │ - commands      │
                └─────────────────┘

Proposed (clean):
                              ┌─────────────────────┐
┌─────────┐                   │ Kernel (sched/)     │
│ Terminal│                   │ - Pure event loop   │
│ (arch/) │                   │ - No I/O            │
└────┬────┘                   └──────────┬──────────┘
     │                                   │
     ▼                                   │ EventBus
┌────────────┐                           │
│ Input      │──────KeyEvent────────────▶│
│ Driver     │                           │
└────────────┘                           │
                                         │
┌────────────┐                           │
│ Command    │◀─────CommandEvent─────────│
│ Module     │                           │
└────┬───────┘                           │
     │                                   │
     │ Command                           │
     ▼                                   │
┌────────────┐                           │
│ Motion     │                           │
│ Engine     │                           │
│ (kernel)   │                           │
└────┬───────┘                           │
     │                                   │
     │ Position                          │
     ▼                                   │
┌────────────┐                           │
│ Buffer     │                           │
│ (kernel/mm)│───────EditEvent──────────▶│
└────────────┘                           │
                                         │
┌────────────┐                           │
│ Syntax     │◀──────BufferChanged───────│
│ Driver     │                           │
└────┬───────┘                           │
     │                                   │
     │ HighlightSpans                    │
     ▼                                   │
┌────────────┐                           │
│ Display    │◀──────RenderSignal────────│
│ Driver     │                           │
└────┬───────┘                           │
     │                                   │
     │ ANSI                              │
     ▼                                   │
┌─────────┐
│ Terminal│
│ (arch/) │
└─────────┘
```

### 7.2 Buffer Lifecycle

```
┌────────────────────────────────────────────────────────────────┐
│                      Buffer Lifecycle                          │
│                                                                │
│  ┌────────┐     ┌────────┐     ┌────────┐     ┌────────┐       │
│  │ CREATE │────▶│ LOAD   │────▶│ ACTIVE │────▶│ DIRTY  │       │
│  └────────┘     └────────┘     └────────┘     └────────┘       │
│       │              │              │              │           │
│       │              │              │              │           │
│       │              ▼              ▼              ▼           │
│       │         ┌────────┐     ┌────────┐     ┌────────┐       │
│       │         │ SYNTAX │     │ SYNTAX │     │ SAVE   │       │
│       │         │ PARSE  │     │ UPDATE │     │        │       │
│       │         └────────┘     └────────┘     └────────┘       │
│       │                                            │           │
│       ▼                                            ▼           │
│  ┌────────┐                                   ┌────────┐       │
│  │ CLOSE  │◀──────────────────────────────────│ CLEAN  │       │
│  └────────┘                                   └────────┘       │
└────────────────────────────────────────────────────────────────┘
```

### 7.3 Rendering Pipeline

```
┌────────────────────────────────────────────────────────────────┐
│                    Rendering Pipeline                          │
│                                                                │
│  Stage 1: Layout                                               │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ WindowManager.layout(terminal_size)                     │   │
│  │ → Calculate window bounds                               │   │
│  │ → Handle splits, tabs                                   │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                 │
│                              ▼                                 │
│  Stage 2: Content                                              │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ For each visible window:                                │   │
│  │   Buffer.lines(visible_range)                           │   │
│  │   → Get text content                                    │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                 │
│                              ▼                                 │
│  Stage 3: Syntax                                               │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ SyntaxDriver.highlights(visible_range)                  │   │
│  │ → Get highlight spans                                   │   │
│  │ → Cache results                                         │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                 │
│                              ▼                                 │
│  Stage 4: Decorations                                          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Apply decorations:                                      │   │
│  │ - Diagnostics (LSP)                                     │   │
│  │ - Search highlights                                     │   │
│  │ - Cursor, selection                                     │   │
│  │ - Virtual text                                          │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                 │
│                              ▼                                 │
│  Stage 5: Composite                                            │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Compositor.composite(layers)                            │   │
│  │ - Base content (z=0)                                    │   │
│  │ - Status line (z=10)                                    │   │
│  │ - Completion popup (z=20)                               │   │
│  │ - Notifications (z=30)                                  │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                 │
│                              ▼                                 │
│  Stage 6: Diff & Flush                                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ FrameRenderer.render()                                  │   │
│  │ → Diff with previous frame                              │   │
│  │ → Emit only changed cells                               │   │
│  │ → Swap buffers                                          │   │
│  └─────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────┘
```

---

## 8. Migration Plan

### 8.1 Overview

```
Phase 0: Preparation              [2 weeks]
Phase 1: Architecture Layer       [3 weeks]
Phase 2: Kernel Core              [6 weeks]
Phase 3: Driver Layer             [4 weeks]
Phase 4: Module System            [3 weeks]
Phase 5: Migration & Testing      [4 weeks]
Phase 6: Deprecation & Cleanup    [2 weeks]
───────────────────────────────────────────
Total: ~24 weeks (6 months)
```

### 8.2 Phase 0: Preparation

**Goal:** Set up infrastructure for migration

**Tasks:**
1. Create new crate structure (empty crates)
2. Set up CI for new crates
3. Create feature flags for gradual migration
4. Document migration guide
5. Set up integration tests

**Directory changes:**
```bash
mkdir -p lib/arch/src
mkdir -p lib/kernel/src/{sched,mm,ipc,core,block,api,debug,panic}
mkdir -p lib/drivers/{display,input,syntax,lsp,net,vfs}/src
```

**New Cargo.toml entries:**
```toml
# Workspace Cargo.toml
[workspace]
members = [
    # Existing
    "runner",
    "lib/core",  # Will be deprecated
    "lib/sys",   # Will be deprecated
    "lib/lsp",   # Will be merged into drivers/lsp
    "plugins/*",
    "tools/*",

    # New
    "lib/arch",
    "lib/kernel",
    "lib/drivers/display",
    "lib/drivers/input",
    "lib/drivers/syntax",
    "lib/drivers/lsp",
    "lib/drivers/net",
    "lib/drivers/vfs",
]
```

### 8.3 Phase 1: Architecture Layer

**Goal:** Extract platform abstraction

**Tasks:**
1. Define platform traits in `lib/arch/src/traits.rs`
2. Implement Unix backend
3. Add Windows stubs
4. Migrate crossterm usage from `lib/sys`
5. Update `lib/core` to use `lib/arch`

**Files to create:**
```
lib/arch/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── traits.rs
    ├── error.rs
    └── unix/
        ├── mod.rs
        ├── terminal.rs
        └── signal.rs
```

**Migration pattern:**
```rust
// Before (lib/sys)
pub use crossterm;

// After (lib/arch)
pub trait Terminal { ... }

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::UnixTerminal as PlatformTerminal;
```

### 8.4 Phase 2: Kernel Core

**Goal:** Extract core mechanisms into pure kernel

**Sub-phases:**

**2.1 Memory Management (mm/)**
- Extract `Buffer` struct
- Extract `Rope` implementation
- Extract caching primitives
- Define memory traits

**2.2 IPC (ipc/)**
- Extract `EventBus`
- Extract channel implementations
- Extract `EventScope`
- Define IPC traits

**2.3 Core Primitives (core/)**
- Extract `Motion` and `MotionEngine`
- Extract `TextObject` and engine
- Extract register operations
- Define command traits

**2.4 Block Operations (block/)**
- Extract `UndoTree` with branching support (vim-style)
  - Store cursor position (before/after) in each UndoNode for restore
  - Configurable max_nodes limit (default: 10000)
- Extract `Transaction` for grouping edits atomically
- Extract `History` for change logging with timestamps
- Extract `Snapshot` for buffer state capture
  - Pure Rust accessors: `lines()`, `cursor()`, `timestamp()`
  - Serialization handled by driver layer (kernel purity principle)

**2.5 Scheduler (sched/)**
- Extract runtime loop
- Define task abstraction
- Extract work queue

**2.6 API Definition (api/)**
- Define stable API (v1)
- Define internal API
- Create compatibility layer

### 8.5 Phase 3: Driver Layer

**Goal:** Create driver crates with trait definitions

**Tasks per driver:**

**Display Driver:**
1. Define `DisplayDriver` trait
2. Extract frame buffer
3. Extract compositor
4. Extract window management

**Input Driver:**
1. Define `InputDriver` trait
2. Extract keyboard handling
3. Extract mouse handling
4. Extract clipboard

**Syntax Driver:**
1. Define `SyntaxDriver` trait (NO tree-sitter!)
2. Define `LanguageRegistry`
3. Extract highlight types
4. Create cache infrastructure

**LSP Driver:**
1. Merge `lib/lsp` into driver
2. Define LSP traits
3. Extract diagnostics

**Net Driver:**
1. Extract RPC server
2. Extract transports
3. Define network traits

**VFS Driver:**
1. Define VFS traits
2. Extract file operations
3. Add file watching

### 8.6 Phase 4: Module System

**Goal:** Implement dynamic loading infrastructure

**Tasks:**
1. Create module traits
2. Implement `ModuleLoader` with `libloading`
3. Create `declare_module!` macro
4. Implement hot reload (development)
5. Create module discovery

**Files:**
```
lib/kernel/src/module/
├── mod.rs
├── loader.rs
├── registry.rs
├── dependency.rs
└── hot_reload.rs
```

### 8.7 Phase 5: Migration & Testing

**Goal:** Migrate existing code to new architecture

**Tasks:**
1. Migrate `lib/core` internals to kernel
2. Migrate plugins to use new APIs
3. Convert language plugins to `SyntaxDriver`
4. Update runner to use new architecture
5. Comprehensive testing

**Migration order for plugins:**
1. Core builtin plugins (lowest risk)
2. Simple feature plugins (completion, statusline)
3. Complex feature plugins (telescope, explorer)
4. Language plugins (tree-sitter migration)
5. LSP plugin (driver + UI split)

### 8.8 Phase 6: Deprecation & Cleanup

**Goal:** Remove old code, finalize architecture

**Tasks:**
1. Add deprecation warnings to old APIs
2. Update documentation
3. Remove `lib/core` (after deprecation period)
4. Remove `lib/sys`
5. Final optimization pass

---

## 9. Deprecation Schedule

### 9.1 Deprecation Timeline

```
Month 1-2: lib/arch created
           lib/core uses lib/arch (internal change)

Month 3-4: lib/kernel created
           lib/core re-exports kernel APIs
           Deprecation warnings added to lib/core

Month 5:   lib/drivers/* created
           Deprecation warnings on old driver code

Month 6:   Module system complete
           Language plugins migrated

Month 7:   lib/core marked #[deprecated]
           Migration guide published

Month 8:   lib/core removed
           lib/sys removed
           Clean architecture complete
```

### 9.2 Deprecation Warnings

```rust
// lib/core/src/lib.rs (during transition)

#![deprecated(
    since = "0.8.0",
    note = "Use reovim-kernel and reovim-drivers-* instead. \
            See migration guide at docs/migration/kernel.md"
)]

// Re-export for compatibility
pub use reovim_kernel::api::v1::*;
pub use reovim_drivers_display as display;
pub use reovim_drivers_input as input;
// ... etc
```

### 9.3 Breaking Change Policy

```
Legacy crates (lib/core, lib/sys, plugins):     v0.8.x
New arch crates (lib/arch, lib/kernel, lib/drivers/*): v0.9.0-dev → v0.9.x

v0.8.x: Legacy crates - stable, maintenance only
v0.9.x: New architecture - Phase 2-6 development
v1.0.0: Stable release with new architecture, legacy deprecated
```

---

## 10. Testing Strategy

### 10.1 Test Hierarchy

```
┌─────────────────────────────────────────────────────────────────┐
│                       Test Pyramid                              │
│                                                                 │
│                        ┌─────────┐                              │
│                        │  E2E    │  Full editor tests           │
│                        │ Tests   │  (10%)                       │
│                        └────┬────┘                              │
│                             │                                   │
│                   ┌─────────┴─────────┐                         │
│                   │  Integration      │  Cross-layer tests      │
│                   │  Tests            │  (20%)                  │
│                   └─────────┬─────────┘                         │
│                             │                                   │
│         ┌───────────────────┴───────────────────┐               │
│         │            Unit Tests                  │  Per-module  │
│         │                                        │  (70%)       │
│         └────────────────────────────────────────┘              │
└─────────────────────────────────────────────────────────────────┘
```

### 10.2 Per-Layer Testing

**Architecture Layer:**
```rust
#[test]
fn test_terminal_size() {
    let term = UnixTerminal::new().unwrap();
    let size = term.size().unwrap();
    assert!(size.cols > 0);
    assert!(size.rows > 0);
}

#[test]
fn test_raw_mode() {
    let mut term = UnixTerminal::new().unwrap();
    let _guard = term.enable_raw_mode().unwrap();
    // Guard restores on drop
}
```

**Kernel Layer:**
```rust
#[test]
fn test_buffer_insert() {
    let mut buffer = Buffer::new();
    buffer.insert(Position::default(), "Hello");
    assert_eq!(buffer.line(0), Some("Hello"));
}

#[test]
fn test_motion_word() {
    let buffer = Buffer::from_string("hello world");
    let engine = MotionEngine;
    let cursor = Cursor { position: Position { line: 0, column: 0 }, ..Default::default() };

    let new_pos = engine.calculate(
        Motion::Word { direction: Direction::Forward, boundary: WordBoundary::Word, end: false },
        &buffer,
        cursor,
        1,
    );

    assert_eq!(new_pos, Some(Position { line: 0, column: 6 }));
}
```

**Driver Layer:**
```rust
#[test]
fn test_syntax_driver_trait() {
    // Mock syntax driver
    struct MockDriver;
    impl SyntaxDriver for MockDriver {
        fn language(&self) -> &str { "mock" }
        fn parse(&mut self, _: &str) {}
        fn update(&mut self, _: &SyntaxEdit) {}
        fn highlights(&self, _: Range<usize>) -> Vec<HighlightSpan> { vec![] }
        fn injections(&self) -> Vec<Injection> { vec![] }
        fn folds(&self) -> Vec<FoldRange> { vec![] }
        fn indent_for(&self, _: usize) -> Option<usize> { None }
    }

    let driver: Box<dyn SyntaxDriver> = Box::new(MockDriver);
    assert_eq!(driver.language(), "mock");
}
```

**Module Layer:**
```rust
#[test]
fn test_module_loading() {
    let mut loader = ModuleLoader::new();

    // Test with statically linked module for CI
    let module = TestModule::new();
    loader.register_static(Box::new(module));

    let ctx = create_test_context();
    loader.init_module(&ModuleId("test"), &ctx).unwrap();
}
```

### 10.3 Integration Tests

```rust
// tests/integration/full_flow.rs

#[tokio::test]
async fn test_key_to_render() {
    let runtime = TestRuntime::new();

    // Inject key
    runtime.inject_key(KeyEvent::char('i')).await;
    runtime.inject_key(KeyEvent::char('H')).await;
    runtime.inject_key(KeyEvent::char('i')).await;
    runtime.inject_key(KeyEvent::escape()).await;

    // Wait for render
    runtime.wait_for_render().await;

    // Check buffer content
    let buffer = runtime.active_buffer();
    assert_eq!(buffer.line(0), Some("Hi"));
}

#[tokio::test]
async fn test_syntax_highlighting() {
    let runtime = TestRuntime::new();

    // Open Rust file
    runtime.open_file("test.rs").await;
    runtime.set_content("fn main() {}").await;

    // Wait for syntax parse
    runtime.wait_for_syntax().await;

    // Check highlights
    let highlights = runtime.get_highlights(0..12);
    assert!(highlights.iter().any(|h| h.highlight == HighlightGroup::KeywordFunction));
}
```

### 10.4 Benchmark Suite

```rust
// tools/bench/benches/kernel.rs

use criterion::{criterion_group, criterion_main, Criterion};

fn bench_motion(c: &mut Criterion) {
    let buffer = Buffer::from_string(include_str!("large_file.rs"));
    let engine = MotionEngine;
    let cursor = Cursor::default();

    c.bench_function("motion_word_forward", |b| {
        b.iter(|| {
            engine.calculate(
                Motion::Word { direction: Direction::Forward, boundary: WordBoundary::Word, end: false },
                &buffer,
                cursor,
                1,
            )
        })
    });
}

fn bench_buffer_insert(c: &mut Criterion) {
    c.bench_function("buffer_insert_char", |b| {
        let mut buffer = Buffer::from_string("Hello, World!");
        b.iter(|| {
            buffer.insert(Position { line: 0, column: 5 }, "X");
            buffer.undo();
        })
    });
}

criterion_group!(kernel_benches, bench_motion, bench_buffer_insert);
criterion_main!(kernel_benches);
```

---

## 11. Performance Contracts

### 11.1 Latency Budgets

```
Operation                    Budget      Current     Target
─────────────────────────────────────────────────────────────
Key press → mode update      < 1ms       ~0.1ms      ✓
Key press → cursor move      < 5ms       ~0.8ms      ✓
Key press → char insert      < 10ms      ~0.1ms      ✓
Key press → render           < 16ms      ~1.0ms      ✓ (60fps)
File open (1MB)              < 100ms     ~50ms       ✓
Syntax parse (1MB)           < 500ms     ~200ms      ✓
Syntax incremental           < 10ms      ~5ms        ✓
```

### 11.2 Memory Budgets

```
Component                    Budget      Notes
─────────────────────────────────────────────────────────────
Base runtime                 < 10MB      Without buffers
Per buffer (1MB file)        < 5MB       ~5x file size (rope)
Syntax tree (1MB file)       < 20MB      tree-sitter overhead
Highlight cache              < 10MB      LRU eviction
Frame buffer (200x50)        < 1MB       2 buffers for diff
```

### 11.3 Performance Regression Tests

```rust
// tools/bench/tests/regression.rs

#[test]
fn regression_motion_latency() {
    let result = run_benchmark("motion_word_forward");
    assert!(result.mean_ns < 1000, "Motion latency regression: {}ns > 1000ns", result.mean_ns);
}

#[test]
fn regression_render_latency() {
    let result = run_benchmark("full_screen_render");
    assert!(result.mean_ns < 100_000, "Render latency regression: {}ns > 100µs", result.mean_ns);
}

#[test]
fn regression_memory_baseline() {
    let mem = measure_memory(|| {
        Runtime::new()
    });
    assert!(mem < 10 * 1024 * 1024, "Memory regression: {}MB > 10MB", mem / 1024 / 1024);
}
```

---

## 12. Appendices

### 12.1 Glossary

| Term | Definition |
|------|------------|
| **Arch** | Architecture layer - platform abstraction |
| **Kernel** | Core mechanisms - pure, no I/O |
| **Driver** | Hardware/service adapter |
| **Module** | Loadable plugin implementing policies |
| **Mechanism** | How something works (kernel provides) |
| **Policy** | What to do (modules decide) |
| **EventBus** | Type-erased publish/subscribe system |
| **SyntaxDriver** | Interface for syntax highlighting |
| **LKM** | Loadable Kernel Module (Linux term) |

### 12.2 File Mapping (Old → New)

```
OLD                                      NEW
───────────────────────────────────────────────────────────────────
lib/core/src/runtime/core.rs        →   lib/kernel/src/sched/runtime.rs
lib/core/src/runtime/event_loop.rs  →   lib/kernel/src/sched/executor.rs
lib/core/src/buffer/mod.rs          →   lib/kernel/src/mm/buffer.rs
lib/core/src/event_bus/bus.rs       →   lib/kernel/src/ipc/event_bus.rs
lib/core/src/motion/mod.rs          →   lib/kernel/src/core/motion.rs
lib/core/src/screen/window.rs       →   lib/drivers/display/src/window/mod.rs
lib/core/src/screen/render_*.rs     →   lib/drivers/display/src/frame/
lib/core/src/event/key/mod.rs       →   lib/drivers/input/src/keyboard/
lib/core/src/rpc/                   →   lib/drivers/net/src/rpc/
lib/sys/                            →   lib/arch/
lib/lsp/                            →   lib/drivers/lsp/
plugins/features/treesitter/        →   lib/drivers/syntax/ (traits only)
                                        plugins/languages/* (implementations)
```

### 12.3 Crate Versions

```toml
# Target versions for v1.0.0

reovim-arch = "1.0.0"
reovim-kernel = "1.0.0"
reovim-drivers-display = "1.0.0"
reovim-drivers-input = "1.0.0"
reovim-drivers-syntax = "1.0.0"
reovim-drivers-lsp = "1.0.0"
reovim-drivers-net = "1.0.0"
reovim-drivers-vfs = "1.0.0"
```

### 12.4 References

**Linux Kernel:**
- [Linux Kernel Development (Robert Love)](https://www.amazon.com/Linux-Kernel-Development-Robert-Love/dp/0672329468)
- [Understanding the Linux Kernel](https://www.amazon.com/Understanding-Linux-Kernel-Third-Daniel/dp/0596005652)
- [Linux Device Drivers](https://lwn.net/Kernel/LDD3/)
- [kernel.org documentation](https://www.kernel.org/doc/html/latest/)

**Clean Architecture:**
- [Clean Architecture (Robert C. Martin)](https://www.amazon.com/Clean-Architecture-Craftsmans-Software-Structure/dp/0134494164)
- [Hexagonal Architecture](https://alistair.cockburn.us/hexagonal-architecture/)

**Rust Patterns:**
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [The Rust Performance Book](https://nnethercote.github.io/perf-book/)

---

## Signatures

**Proposed by:** ds1sqe - ds1sqe@mensakorea.org
**Date:** 2026-01-11

---

*"Perfection is achieved, not when there is nothing more to add, but when there is nothing left to take away."*
— Antoine de Saint-Exupéry

*"The Linux philosophy is 'Laugh in the face of danger'. Oops. Wrong One. 'Do it yourself'. Yes, that's it."*
— Linus Torvalds
