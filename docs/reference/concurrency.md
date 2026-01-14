# Concurrency Reference

This document describes the Rust/Tokio concurrency patterns used in the reovim server. It covers the threading model, lock-free patterns, and synchronization strategies.

## Overview

The reovim server uses a multi-threaded async architecture:

- **Tokio runtime** with work-stealing scheduler
- **Task-per-client** model for concurrent handling
- **Lock-free patterns** for hot paths (session lookup, ID generation)
- **RwLock/Mutex** for mutable state with clear lock hierarchy

## Tokio Runtime Model

### Work-Stealing Scheduler

```
┌─────────────────────────────────────────────────────────────┐
│                 TOKIO MULTI-THREADED RUNTIME                │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Worker Threads (1 per CPU core)                      │   │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐    │   │
│  │  │Worker 0 │ │Worker 1 │ │Worker 2 │ │Worker N │    │   │
│  │  │Local Q  │ │Local Q  │ │Local Q  │ │Local Q  │    │   │
│  │  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘    │   │
│  │       │           │           │           │          │   │
│  │       └───────────┴─────┬─────┴───────────┘          │   │
│  │                         ▼                            │   │
│  │              ┌──────────────────┐                    │   │
│  │              │   Global Queue   │                    │   │
│  │              └──────────────────┘                    │   │
│  │                                                      │   │
│  │  Work Stealing: Empty worker steals from others     │   │
│  │  LIFO Slot: Wake-up tasks run immediately           │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

**Key Properties:**

| Property | Description |
|----------|-------------|
| `rt-multi-thread` | Enables work-stealing scheduler |
| Worker threads | 1 per CPU core (default) |
| Task size | ~64 bytes (green threads) |
| Work stealing | Empty workers steal from others' local queues |
| LIFO slot | Wake-up tasks execute immediately |

### Task Spawning

```rust
// Spawn a task for each client connection
tokio::spawn(async move {
    handle_client(stream, session_registry).await
});
```

Tasks are lightweight and can be moved between threads. Requirements:

- Task must be `Send` (can move between threads)
- All state across `.await` must be `Send`
- Use `tokio::task::spawn_local` for `!Send` types (single-threaded)

## Server Threading Model

### Task-per-Client Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    REOVIM SERVER                             │
│                                                              │
│  Main Task (tokio::spawn)                                   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ TCP Listener                                         │   │
│  │  └─► accept() loop                                   │   │
│  │       └─► spawn client task for each connection      │   │
│  └─────────────────────────────────────────────────────┘   │
│                         │                                   │
│     ┌───────────────────┼───────────────────┐              │
│     ▼                   ▼                   ▼              │
│  ┌──────────┐     ┌──────────┐        ┌──────────┐        │
│  │Client 1  │     │Client 2  │        │Client N  │        │
│  │Task      │     │Task      │        │Task      │        │
│  │          │     │          │        │          │        │
│  │ read()   │     │ read()   │        │ read()   │        │
│  │ process  │     │ process  │        │ process  │        │
│  │ write()  │     │ write()  │        │ write()  │        │
│  └──────────┘     └──────────┘        └──────────┘        │
│        │                │                   │              │
│        └────────────────┼───────────────────┘              │
│                         ▼                                   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Shared State (Arc<...>)                              │   │
│  │  ├── SessionRegistry                                 │   │
│  │  │    └── ArcSwap<HashMap<SessionId, Arc<Session>>> │   │
│  │  └── Each Session                                    │   │
│  │       └── RwLock<SessionState>                      │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Shared State Patterns

```rust
// Shared state wrapped in Arc for multi-task access
let session_registry = Arc::new(SessionRegistry::new());

// Clone Arc for each spawned task
let registry = Arc::clone(&session_registry);
tokio::spawn(async move {
    // Task owns its Arc clone
    let session = registry.get(&session_id);
});
```

## Lock-Free Patterns

### Existing Patterns in Reovim Kernel

The kernel already uses these lock-free patterns:

| Pattern | Component | File | Purpose |
|---------|-----------|------|---------|
| **ArcSwap (RCU)** | EventBus handlers | `lib/kernel/src/ipc/event_bus.rs:117` | Lock-free handler dispatch |
| **ArcSwap** | LineCache | `lib/kernel/src/mm/cache.rs:66` | Lock-free cache reads |
| **AtomicU64** | SubscriptionId | `lib/kernel/src/ipc/subscription.rs:58` | ID generation |
| **AtomicBool** | Saturator shutdown | `lib/kernel/src/mm/saturator.rs:88` | Graceful termination |
| **Condvar** | EventScope | `lib/kernel/src/ipc/scope.rs:90` | Wait for completion |

### ArcSwap (RCU Pattern)

ArcSwap provides lock-free reads with copy-on-write updates:

```rust
use arc_swap::ArcSwap;
use std::sync::Arc;
use std::collections::HashMap;

pub struct SessionRegistry {
    // Lock-free reads via ArcSwap
    sessions: ArcSwap<HashMap<SessionId, Arc<Session>>>,
    next_client_id: AtomicU64,
}

impl SessionRegistry {
    /// Lock-free read (hot path) - O(1)
    pub fn get(&self, id: &SessionId) -> Option<Arc<Session>> {
        // load() returns a Guard, deref to access HashMap
        self.sessions.load().get(id).cloned()
    }

    /// RCU update (cold path) - copy-on-write
    pub fn insert(&self, session: Arc<Session>) {
        self.sessions.rcu(|current| {
            // Clone current state
            let mut new = (**current).clone();
            // Modify clone
            new.insert(session.id().clone(), session.clone());
            // Return new state (atomically swapped)
            new
        });
    }

    /// Remove session
    pub fn remove(&self, id: &SessionId) {
        self.sessions.rcu(|current| {
            let mut new = (**current).clone();
            new.remove(id);
            new
        });
    }
}
```

**When to use ArcSwap:**

- Read-heavy workloads (many readers, few writers)
- Hot paths where lock contention matters
- Data that changes infrequently

### Atomic Primitives

```rust
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};

pub struct SessionRegistry {
    next_client_id: AtomicU64,
    shutdown: AtomicBool,
}

impl SessionRegistry {
    /// Generate unique client ID - lock-free
    pub fn next_client_id(&self) -> ClientId {
        // fetch_add is atomic, returns previous value
        ClientId(self.next_client_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Check shutdown flag
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    /// Signal shutdown
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
    }
}
```

**Memory Ordering:**

| Ordering | Use Case |
|----------|----------|
| `Relaxed` | Counters, statistics (no synchronization needed) |
| `Acquire` | Reading shared state (pairs with Release) |
| `Release` | Writing shared state (pairs with Acquire) |
| `SeqCst` | When in doubt (strongest guarantee, slowest) |

## Lock Hierarchy

Locks must be acquired in a consistent order to prevent deadlocks:

```
┌─────────────────────────────────────────────────────────────┐
│                  LOCK HIERARCHY                              │
│                                                              │
│  Level 0 (Lock-Free):                                       │
│  ├── ArcSwap<HashMap<SessionId, Arc<Session>>>  (registry) │
│  ├── AtomicU64 (client ID generation)                       │
│  └── AtomicBool (shutdown flag)                             │
│                                                              │
│  Level 1 (Per-Session):                                      │
│  └── tokio::sync::RwLock<SessionState>                      │
│       ├── Multiple readers (state queries)                  │
│       └── Single writer (key processing)                    │
│                                                              │
│  Level 2 (Per-Client):                                       │
│  └── tokio::sync::Mutex<WriteHalf>                          │
│       └── Serialize responses to client                     │
└─────────────────────────────────────────────────────────────┘
```

**Rule:** Always acquire locks in order: Level 0 → Level 1 → Level 2

## Synchronization Strategy

### std::sync vs tokio::sync

| Type | Use When | Notes |
|------|----------|-------|
| `std::sync::Mutex` | Lock not held across `.await` | Blocks thread, but short |
| `tokio::sync::Mutex` | Lock held across `.await` | Async-aware, yields to runtime |
| `std::sync::RwLock` | Read-heavy, no `.await` | Multiple readers |
| `tokio::sync::RwLock` | Read-heavy, with `.await` | Async-aware, fair scheduling |
| `parking_lot::Mutex` | Performance-critical | Faster than std, no poisoning |

**Example - Correct Usage:**

```rust
// GOOD: Short critical section, no await
let data = {
    let guard = std_mutex.lock().unwrap();
    guard.clone()
};

// GOOD: Lock held across await
let mut guard = tokio_mutex.lock().await;
guard.async_operation().await;
drop(guard);

// BAD: std mutex held across await (blocks thread!)
let guard = std_mutex.lock().unwrap();
some_async_op().await; // DON'T DO THIS
```

### RwLock for Concurrent Reads

```rust
use tokio::sync::RwLock;

pub struct Session {
    state: RwLock<SessionState>,
}

impl Session {
    /// Multiple concurrent readers allowed
    pub async fn get_mode(&self) -> ModeId {
        let state = self.state.read().await;
        state.app.current_mode().clone()
    }

    /// Single writer, blocks readers
    pub async fn process_key(&self, key: KeyEvent) -> CommandResult {
        let mut state = self.state.write().await;
        // Exclusive access to state
        state.process_key(key)
    }
}
```

### Channel-Based Actor Pattern

Alternative to shared state with locks:

```
┌─────────────────────────────────────────────────────────────┐
│  Client Tasks                    Session Task               │
│  ┌──────────┐                   ┌──────────────────┐       │
│  │Client 1  │──── Request ────►│                  │       │
│  └──────────┘                   │  Session Actor   │       │
│  ┌──────────┐                   │                  │       │
│  │Client 2  │──── Request ────►│  - Owns state    │       │
│  └──────────┘                   │  - Processes     │       │
│  ┌──────────┐                   │    sequentially  │       │
│  │Client N  │──── Request ────►│  - No locks!     │       │
│  └──────────┘                   └──────────────────┘       │
│       ▲                                  │                  │
│       └────────── Response ◄─────────────┘                  │
└─────────────────────────────────────────────────────────────┘
```

```rust
use tokio::sync::{mpsc, oneshot};

enum SessionCommand {
    ProcessKey {
        key: KeyEvent,
        reply: oneshot::Sender<CommandResult>
    },
    GetMode {
        reply: oneshot::Sender<ModeId>
    },
}

async fn session_actor(mut rx: mpsc::Receiver<SessionCommand>, mut state: SessionState) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            SessionCommand::ProcessKey { key, reply } => {
                let result = state.process_key(key);
                let _ = reply.send(result);
            }
            SessionCommand::GetMode { reply } => {
                let _ = reply.send(state.app.current_mode().clone());
            }
        }
    }
}
```

**Pros:** No lock contention, sequential processing, clear ownership
**Cons:** Extra task overhead, channel latency, more complex API

### Sharding for High Contention

If a single lock becomes a bottleneck, shard it:

```rust
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

const NUM_SHARDS: usize = 16;

pub struct ShardedRegistry {
    shards: [RwLock<HashMap<SessionId, Arc<Session>>>; NUM_SHARDS],
}

impl ShardedRegistry {
    fn shard_index(id: &SessionId) -> usize {
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        hasher.finish() as usize % NUM_SHARDS
    }

    pub async fn get(&self, id: &SessionId) -> Option<Arc<Session>> {
        let shard = &self.shards[Self::shard_index(id)];
        shard.read().await.get(id).cloned()
    }
}
```

## Performance Targets

| Operation | Target | Pattern | Notes |
|-----------|--------|---------|-------|
| Session lookup | <100ns | ArcSwap lock-free read | Hot path |
| Client ID gen | <10ns | AtomicU64 fetch_add | No contention |
| Key processing | <1ms | RwLock write | Sequential per-session |
| State query | <100µs | RwLock read | Concurrent |
| RPC dispatch | <10µs | Direct method call | No allocation |

### Pattern Selection Guide

```
Is the operation read-heavy?
├── Yes: Does data change frequently?
│   ├── Yes → RwLock (or shard if contention)
│   └── No → ArcSwap (lock-free reads)
└── No: Is it a simple counter/flag?
    ├── Yes → Atomic (AtomicU64, AtomicBool)
    └── No: Is lock held across await?
        ├── Yes → tokio::sync::Mutex/RwLock
        └── No → std::sync::Mutex (or parking_lot)
```

## Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "net", "io-util", "sync", "macros"] }
arc-swap = "1.8"           # Lock-free RCU (already in kernel)
parking_lot = "0.12"       # Fast mutexes (already in arch)
```

## References

### Tokio & Async Runtime

- [tokio::runtime Documentation](https://docs.rs/tokio/latest/tokio/runtime/index.html)
- [Spawning Tasks](https://tokio.rs/tokio/tutorial/spawning)
- [Shared State in Tokio](https://tokio.rs/tokio/tutorial/shared-state)
- [Inside Tokio's ThreadPool](https://medium.com/@theopinionatedev/inside-tokios-threadpool-how-rust-handles-millions-of-tasks-with-pinning-and-polling-84a6fa25ea11)
- [tokio::sync::RwLock](https://docs.rs/tokio/latest/tokio/sync/struct.RwLock.html)

### Lock-Free & Concurrency

- [Crossbeam GitHub](https://github.com/crossbeam-rs/crossbeam)
- [Crossbeam Crate Guide](https://generalistprogrammer.com/tutorials/crossbeam-rust-crate-guide)
- [Lock-freedom without garbage collection](https://aturon.github.io/blog/2015/08/27/epoch/)
- [Fearless Concurrency: Lock-Free Structures](https://www.ardanlabs.com/blog/2024/12/fearless-concurrency-ep7-lock-free-structures-and-channels-for-scalable-rust-code.html)
- [Arc and Mutex Guide](https://dev.to/ietxaniz/rust-concurrency-explained-a-beginners-guide-to-arc-and-mutex-13ca)
