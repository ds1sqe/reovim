# Kernel Runtime

Executes the [Interface](../interface/README.md) caller contract with bounded
request custody, granted access and retained provider ownership.

```rust
let mut runtime = Runtime::new(epoch, Limits::default())?;
let imports = runtime.bind(&[])?;
// Construct provider with imports.caller() as its actual import context.
let source = runtime.register(b"/source", provider, Some(&imports))
    .map_err(|(error, _provider)| error)?;
let grant = Grant {
    resource: source, access: 1, create_access: 0, retain_loans: false,
};
let caller = runtime.bind(&[grant])?.caller();
runtime.start()?;
// Assembly drives Runtime; consumers use Caller through Interface/ABI adapters.
```

Paths default to ASCII. Assembly may explicitly allow non-ASCII names before
wiring; Resource contents are independent of namespace naming policy.

`engine/` owns admission, execution and cleanup. Input references become
narrowed loans in the provider's import binding. Loans and bound aliases expire
after provider finish. The originating grant's `retain_loans` permission allows
explicit capture; a borrowed token itself never becomes owned output. Output
access moves into reserved caller custody before the provider result owner is
released. Caller adoption is explicit; discarded results clean their access.
Malformed output retains its cleanup owner and reports incomplete shutdown.
Caller result polling never drives provider execution. Unknown effects retain
their original request and code through reconciliation.

Factory calls reserve identity and access capacity before provider entry.
Primary and auxiliary staging reserve identities and provisional tokens.
The primary checks its declared API; output references check the caller's
`create_access` ceiling. Acceptance installs returned Resources and access
together; unreturned auxiliaries drain under the same request owner.
Failure drains unpublished providers; Unknown retains its original staging.
Anonymous created Resources drain after their final access/request owner.

Native factory providers use the same staging owner through the SDK adapter.
Byte reads and writes use the same request custody, including zero-length
no-ops, positional bounds and recovery of known transfer counts. Per-open
provider state, packets, namespace mutations, Edit, Buffer/History and Sessions
are still being migrated onto this owner. Uncompiled older modules supply donor
behavior.

Info copies exact subject identity, held access, declarations and optional I/O
limits. Native namespaces supply CONTAINER/EMPTY through a driven flag query;
other providers have neither flag. Metadata runs with zero activity, needs no
READ right, and does not enumerate children. Invalid flags retain uncertain
custody; retries and reconciliation use the original observation. No Host clock
is supplied, so internal times remain unavailable. Clock/activity stamping and
separately identified external metadata remain migration work.

Global module activation constructs inert private state through Interface and
drives its Starting phase. The scoped Registrar rejects construction-time and
late staging. Ready publishes the complete required batch after revalidation;
failure fences exports and retains cleanup. Instance and mount identities are
Resources. The SDK imports native module entry/context/driver records into this
same activation path and retains instance data through dependent providers.
The reference Host loads native artifacts through that SDK admission. Production
server/Client integration, registry selection, joined activation and individual
unload remain open.

This crate uses `no_std + alloc`. Native loading belongs in Host adapters;
Client input and presentation belong in Clients.

Composition can grant an exact module publication scope with
`Runtime::grant_publication`: activation mount identity plus Global/Session
owner. Runtime records membership during staging; paths do not confer it.
Current and later exports become acquirable within that ceiling. These grants
retain the scope owner, while opened access retains each acquired subject.
Exact subject grants take precedence; existing access ceilings do not change.
Module import contexts copy this explicit authority and remain frozen after
registration. Ordinary and publication grants share the configured grant bound.

`Runtime::register_namespace` installs a read-only Kernel publication directory.
Staging records existing direct-parent identities and revalidates them before
installation. Entries requires held READ and returns copied byte names/IDs;
listing grants no child access. Info reports explicit CONTAINER/EMPTY flags.
This implementation returns a complete page (cursor/next zero), or Pressure if
its byte/node budget cannot hold it. It allocates the required output size.
External namespace tables, multi-page enumeration, aliases, mutations and lazy
provider-key materialization are not implemented by this built-in view.

Native namespace listing now runs as an ordinary driven Entries request. Runtime
validates the owned name/key page and returns interned ResourceIds. Aliases with
the same key in one provider instance share an identity; other instances and
replacement keys remain distinct. Cold key records use the Resource budget and
weak provider ownership; they confer no grants or implementation leases.
Original receipt retries do not repeat provider listing. Acquire by listed ID
joins one materialization attempt, stages its provider and installs the same
identity before returning independent zero-right access. Explicit publication
grants supply the ceiling; parent READ grants no child rights. Unknown retains
the original shared attempt for reconciliation. Child ownership retains its
source context through cleanup; replacement keys never retarget captured IDs.
Native path traversal captures names, drives lookup, joins the same key work
and retains intermediate owners through cleanup. Exact grants take precedence;
existing granted children can be located without publication authority. Cold
objects require an explicit publication grant. Recovery resumes the original
lookup/materialization; replacement cannot redirect captured keys.

Delete requires held parent WRITE, captures a lookup key, and drives removal
under the same original request. Providers revalidate the key and reject nonempty
containers; removal changes names without closing existing subject access.
Link retains the opened source and passes its key only after verifying both
subjects share a mounted namespace. The provider rejects an occupied name or
stale source; no copied subject or expanded grant is created. Source close waits
for original link cleanup, including UNKNOWN recovery. Create/rename remain open.
