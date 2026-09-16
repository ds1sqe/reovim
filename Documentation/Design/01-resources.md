# Resource identity and opened access

This chapter owns Resource identity, acquisition and access lifetime.
[Namespace](01.1-namespace.md) owns names and naming operations;
[operation selection](01.2-operations.md) owns supported calls and outcomes.

## Terms

- Resource: a subject with stable identity and optional supported interfaces.
- ResourceId: a copied identity scoped to one Kernel instance. It grants and
  retains nothing. It never retargets a replacement subject or restarted Kernel.
- Namespace entry: one parent/name association with a Resource. Several names
  can refer to one subject; the subject need not have a current name.
- ResourceHandle: owned granted access. Typed API wrappers retain that same
  access with a checked binding, not another Resource identity or Kernel kind.
- k: caller-bound SDK context. It owns services for acquisition, requests and
  Buffer use; the Runtime implementation remains behind Interface/ABI.

Buffer, History, immutable snapshot access, watches, devices and commands are
Resources. Internal state values and numeric revisions need no Resource
identity. Capabilities describe support; Access flags describe authority
requested/held. ResourceId contains a Kernel epoch[16] and nonzero serial:u64.
Epochs distinguish Kernel instances; a serial is never reused within its epoch.

A Resource's backing implementation owns private context and optional operation
interfaces. Context may represent a file, USB drive, memory region, GPU, NPU or
another subject; Kernel assumes no byte storage or backend-kind enum. It may
refer to an existing external object: assigning Resource identity does not
create that object. One module can supply many Resources sharing private state.
[Provider contributions](05.2-modules.md) bind implementations to these generic
contracts; [composition](05.1-composition.md) owns their extension boundary.

## Acquisition surface

All calls below return Result<_, RequestError>; async is omitted only in prose.
Path examples are absolute in the caller's namespace. Session CWD/PATH policy
remains above this common namespace boundary; command lookup does not follow
CWD.

```rust
impl Kernel {
    async fn open(&self, path: &[u8]) -> Result<ResourceHandle, RequestError>;
    async fn resolve<T: ResourceInterface>(&self, path: &[u8])
        -> Result<T, RequestError>;
    async fn acquire(&self, id: ResourceId)
        -> Result<ResourceHandle, RequestError>;
}
```

There is one acquisition operation underneath these conveniences:

| SDK entry | Requested access | Expected interface |
| --- | --- | --- |
| open(path) | None | No typed API qualification |
| acquire(id) | None | No typed qualification |
| resolve<T>(path) | T's declared minimum, via set_access | T |

open/acquire return an owned handle with no operation rights, after checking
visibility and acquisition authority. resolve combines acquisition, set_access
and checked binding; no callable wrapper escapes if any step fails.
Acquisition may initialize per-open provider state; it creates no subject and
invokes no data or executable operation. resolve uses that same acquisition.
Discovery through entries/info is separate.

```rust
let source = k.open(b"/files/notes.md").await?;
source.set_access(Access::READ).await?;
let output = k.open(b"/files/export.txt").await?;
output.set_access(Access::WRITE).await?;
```

ResourceHandle has no implicit Clone. A new acquisition of its ID creates a
separate access with its interface's own per-open state. A typed qualification
does not duplicate that state. Stream receiving may still consume one shared
endpoint; opening again does not promise a cloned cursor or fan-out.

## Data, inspection and access release

```rust
impl ResourceHandle {
    fn id(&self) -> ResourceId;
    fn access(&self) -> Access;
    async fn set_access(&self, flags: Access) -> Result<(), RequestError>;
    async fn info(&self) -> Result<ResourceInfo, RequestError>;
    async fn entries(&self) -> Result<Vec<ResourceEntry>, RequestError>;
    async fn read(&self, output: &mut [u8]) -> Result<ReadResult, RequestError>;
    async fn write(&self, input: &[u8]) -> Result<usize, RequestError>;
    async fn read_at(&self, offset: u64, output: &mut [u8])
        -> Result<ReadResult, RequestError>;
    async fn write_at(&self, offset: u64, input: &[u8])
        -> Result<usize, RequestError>;
    async fn capture(&self, rights: Access)
        -> Result<ResourceHandle, RequestError>;
    async fn close(&self) -> Result<(), RequestError>;
}
```

access returns this opened handle's last confirmed flags. set_access requests
exactly the supplied flags within caller grants and the opened implementation's
support. A captured access or request loan also keeps its delegated grant
ceiling; set_access cannot widen that ceiling. Empty flags are valid; rejected
requests leave flags unchanged. This changes only held access, not Resource-wide
permissions. It creates no new open state and does not reopen the provider.

Bindings sharing this access observe its changed flags; independently opened
handles and captures keep theirs. Changes govern subsequent admission. Admitted
requests retain captured access under [request custody](01.3-requests.md);
revoking the caller's underlying grant remains a separate authority check.
Closing/Closed access cannot regain rights through set_access. Uncertain changes
retain their original request; new admission through this access waits for its
confirmed mask. Access changes are serialized on that shared access owner.
These bookkeeping operations do not themselves require held READ/WRITE/EXEC;
they enforce the caller's acquisition and delegation grants.

read/write use the opened stream interface; read_at/write_at use its positional
interface. Each requires support under the [operation rules](01.2-operations.md).
[I/O](01.4-io.md) owns transfer and ReadResult semantics. Packet,
execution and control wrappers qualify this same access through declarations.

capture explicitly retains selected opened access with narrowed rights and an
independent close fence. It retains any checked binding and shares opened
provider state/cursor, unlike a fresh acquisition; it creates no subject and
calls no provider open. Closing one access does not close its captures.
The opened implementation drains after its
last access/request owner. Bind only qualifies one existing access and shares
its fence. Capturing a request loan additionally needs the grant's retention
permission; without it capture returns Denied. Copying a token never extends its
lifetime.

ResourceInfo contains identity, supported interfaces, held Access and
[Resource metadata](01.5-metadata.md). entries returns the
[namespace's owned listing values](01.1-namespace.md), acquiring no children.

## Handle state and request custody

| State | New subject operations | close/recovery |
| --- | --- | --- |
| Open | Admitted with rights/liveness checks | close starts fencing |
| Closing | Rejected Closed | Join/retry the existing close cleanup |
| Closed | Rejected Closed | close is idempotent; recovery via k.requests |

Unknown is an operation outcome, not a fourth handle state. An uncertain remote
close is treated as fenced locally until its original outcome is recovered.
Rejected close before fencing leaves Open. Once fenced, Busy or failure keeps
Closing and its cleanup owner; it must not restore permission to submit work.

close takes shared access so it can fence a handle while a read future exists.
Queued input and borrowed output remain safe: native providers never retain the
caller slice. The adapter owns transfer buffers; completed reads copy back only
while their caller borrow exists. Closing requests cancellation where supported
and settles admitted effects; it does not convert uncertain writes into retries.

[Request custody](01.3-requests.md) retains admitted work and its result after
handle close. Recovery uses the caller context; it does not require the closed
subject handle.

Last-owner Drop is nonblocking and requests close without claiming completion.
Bindings share that access-close state.
Acquisition reserves bounded cleanup bookkeeping before returning
a handle; allocation pressure rejects new access rather than losing that owner.
The execution owner drains or explicitly reports incomplete shutdown. Device
loss/revocation can end usable access without destroying retained state/code.

Runtime reclaims a subject only when its publication/lifecycle owners, accesses
and requests no longer require it. Removing its last name is one such ownership
change, not a provider-unload callback. Provider/reference cycles require
explicit bounded owners and cleanup; no diagram implies arbitrary Rc cycles
reclaim.
