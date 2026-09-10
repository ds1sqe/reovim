# 05 — Runtime execution

Runtime owns live contexts, execution and lifecycle.
[Resource calls](01.2-resource-access.md) submit work to it; its owner drives
execution independently of callers reading their results.

```text
construct -> start dependencies -> Ready -> admit and execute calls
close     -> stop admission -> settle owned work -> Stopped
// Ready means required services are usable; Idle only means no runnable work.
// A closing Runtime still permits result recovery and cleanup.
```

## Source ownership

```text
assembly selects and binds implementations
server/client orchestration -> Interface
Kernel Runtime -> Core -> Interface -> Foundation
```

| Source home | Responsibility |
| --- | --- |
| kernel/foundation | Core-only checked arithmetic and value invariants |
| kernel/interface | Public values, typed tables and provider contracts |
| kernel/core | Atomic neutral state/History installation |
| kernel/runtime | Live contexts, admission, execution and lifecycle |
| resource/, domain/, representation/ | Access, meaning and interpretation |
| host/, protocol/, presentation/ | Environment, transport and drawing |
| clients/, sdk/, extensions/ | Client policy and author-facing APIs |
| apps/ composition | Construct and bind concrete providers |

Consumers import Interface; composition may import implementations. Each layer
needs a distinct purpose; do not duplicate public catalogs. Foundation stays
core-only; portable Kernel/applicable Rust Client code uses `no_std + alloc`.

[Composition](05.1-composition.md) owns construction and provider wiring.
Runtime's compiled type cannot enumerate dynamically loaded providers.
Client Runtime executes [input and presentation](04-clients.md). Kernel may
retain opaque versioned Session data and provide semantic observations.

## Driver and readiness

```rust
// Path: kernel/interface/src/driver.rs; module: interface::driver
pub enum Lifecycle {
    Constructed, // Providers are inert.
    Starting,    // Activation is provisional.
    Ready,       // Required dependencies are usable.
    Closing,     // No new ordinary work; results/cleanup remain accessible.
    Closed,
    Failed(Error), // Failure alone does not prove rollback/cleanup finished.
}
pub enum Progress {
    More,    // Schedule another bounded work turn.
    Idle,    // No runnable work; pending operations may still exist.
    Stopped, // Teardown, including failed-boot rollback, has finished.
}
pub struct DriverOps<Context> {
    pub status: fn(context: &Context) -> Lifecycle,
    pub advance: fn(context: &mut Context) -> Result<Progress, Error>,
    pub close: fn(context: &mut Context) -> Result<(), Error>,
}
// Context is private implementation state; the owner pairs it with this table.
// Ordinary modules receive no driver for Kernel or their owning Runtime.
```

The owner installs Host wake/readiness sources before serving calls.
Admission, provider completion and deadlines wake its loop. Arm readiness,
recheck work, then sleep. Embedded hosts drive the same loop; a server may own
it continuously. Result reads never run providers; SDK waiting cannot block
the only executor. A configured work quantum does not bound trusted
synchronous provider time.

Assembly constructs providers, Host grants and contexts. Runtime activates
dependencies and publishes Ready only after success. Earlier ordinary calls
reject; lifecycle/diagnostics remain usable. Failed boot rolls back
provisional ownership and reports unfinished cleanup.

Close is idempotent for that lifetime. It fences admission, then drains or
transfers queued work, effects and leases to an explicit live owner before
Stopped. Driver release also waits for Stopped. Timeout proves no rollback
and cannot permit unsafe unload. Provider work and teardown run outside Core
commit authority.

Kernel Runtime also drives its [module instances](06-modules.md) through
this lifecycle contract. Runtime pairs the constructed state with its declared
driver; modules receive no driver for the containing Runtime. Instance
readiness and public contribution publication remain separately checked.

## Host and remote access

Host grants apply separately to Kernel, Resource and Client consumers.
Adapters own native handles and OS behavior. A custom host supplies the
memory, timing, wake/progress and I/O its selected graph consumes; `no_std`
compilation alone proves no runtime support. Missing grants never fall back to
ambient I/O.

A remote SDK owns its connection, codec and correlation state. The server
authenticates calls and submits them to the same Runtime boundary. Reconnect
creates a new access lifetime while retained outcomes remain recoverable; it
proves no prior failure. Native pointers/handles do not cross the wire.
