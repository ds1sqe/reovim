# Descriptors and Dependency Admission

[Roles, Descriptors, and Dependencies](../01-Roles-Descriptors-and-Dependencies.md)

## Descriptor and dependency discipline

Every Foundation opaque-carrier family owns a versioned Carrier Safety
Descriptor. Before construction, copy, transfer, downcast, sharing, drop, or
invocation, it makes ownership/lifetime, type/schema mismatch, affinity and
concurrency, panic/unwind behavior, and the unsafe implementation/audit boundary
inspectable. An undeclared property is unsupported.

`InstanceBound` and `IncarnationBound` are logical identity scopes. A Rust
borrow protects referenced storage but does not prove lifecycle liveness,
readiness, or admission; a live capability use also compares the applicable
current identity and generation.

Kernel Interface exposes the complete admitted Foundation vocabulary through
neutral `bound`, `carrier`, `contract`, and `identity` modules. Public consumers
can therefore construct and interpret returned descriptors without a direct
Foundation dependency. These are exact aliases, not a second semantic
definition; focused contract modules may retain convenient aliases.

Every capability or operation family claiming a bound owns a Bound Descriptor.
It declares units and hard maxima for per-object and aggregate counts, owned
bytes, payload, retained state, concurrency, queues, and completion waits as
applicable. It also declares checked arithmetic, overflow, zero-limit behavior,
accounting ownership, validation order before traversal/allocation, and typed
exhaustion.

A third-party Cargo package enters or remains in the repository graph only with
a machine-readable exact admission naming its resolved identity and metadata-
resolved consumers/profiles, plus a retained Dependency Review Record. Another
third-party mechanism requires its own mechanism-specific exact pin/admission
and retained review; Cargo metadata does not describe it. These laws apply to
product roles and Repository Tooling independently. Package name, popularity, a
lockfile entry alone, or a permissive-looking license is not blanket approval.

Metadata resolution proves declared graph participation, not actual compiler
units, source inputs, helper reads, package membership, or public accessibility.
A Product closure additionally requires the mechanism-specific evidence owned
by the Conformance Product Proof Profile.

The Kernel owns Domain, Representation, Projection, and semantic-coordinate
contracts plus generic coordination. It does not own concrete Text, byte, ELF,
QEMU, structured-data, or media policy merely because that policy implements a
Kernel contract.
