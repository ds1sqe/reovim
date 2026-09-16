# Kernel Interface

Public Resource access and provider contracts, independent of the
[Runtime implementation](../runtime/README.md).

- `caller`: bound calls, request reservation, recovery and result custody.
- `resource`: instance-qualified identities, access tokens and operation rights.
- `declaration`: canonical API definitions and typed value validation.
- `call`: owned values, outcomes and pending provider work.
- `provider`: declarations/callbacks, I/O, namespace, creation and Registrar.
- `module`: inert construction context and owned instance execution.
- `driver` and `notify`: execution progress and wake registration.
- `abi`: C-compatible provider, state, I/O and caller records for adapters.

Consumers use Caller and declarations; composition supplies concrete providers.
A provider receives selected operations and captured values. Factories receive
Creation staging; Runtime owns installation and cleanup. Native ABI records do
not by themselves establish implemented adapter or Runtime support.

Typed namespace flags, lookup/list, materialization, removal and linking
execute through the native adapters.
Keys are opaque bytes; aliases may return the same live key. Materialization
uses Creation custody and retains the originating native context through child
cleanup. Runtime consumes list through Caller Entries and interns provider keys.
Acquire by listed identity or native path joins shared materialization and
returns zero-right access under explicit grants. Path lookup retains its original
query/key through recovery; aliases share the mounted provider key space.
Listing alone grants no access or implementation lease. Delete captures and
revalidates the selected key under parent WRITE. Link retains a visible source
and supplies its key within the same mounted namespace. Create/rename remain open.

[Design](../../Documentation/Design/README.md) owns the contracts; Runtime and
adapter tests establish supported behavior. Older unexported source supplies
donor input for Buffer/History, observation and publication migration.
Module-specific schemas and typed helpers belong with their module.

This crate uses `no_std + alloc` and has no dependency on Runtime or Clients.
