# Public Surface and Consumer Evidence

[Product v2 Proof](../05-Product-v2-Proof.md)

## Nonshipping external consumer

Public accessibility is proved by a separate Product-owned nonshipping library
consumer. Its source use is `Test`; it compiles as `lib`/`rlib` through one
allowed inward Product-role edge, names every registry-declared intended public
path through the external crate boundary, and is absent from shipping compiler
and package closures. Repository Tooling never imports it or the shipping
Product crate.

A dev dependency cannot smuggle the consumer into the shipping package. A
linked test executable is not shipping proof. This compile-only check is static
accessibility/type-checking evidence, not public behavioral conformance,
semantic completeness, ABI stability, runtime behavior, linker evidence, or
qualification.

## Closed public surface

Every Product source/module input reported by rustc dep-info receives the closed
syntax/type checks in the Product profile. The consumer must compile without a
private-module, shared-source, generated-source, or Repository Tooling bypass.

The initial profile permits directly authored public declarations/modules and
the exact closed type-path/compiler-attribute sets. It rejects public re-
exports, public aliases, unresolved paths, cfg-selected or macro-generated
public items, and types from an external package or Repository Tooling.

An admitted built-in derive is compiler-generated behavior. The registry names
the exact derive, every affected public type, and the consumer operation/trait
bound that type-checks its intended public availability. An unlisted derive or
procedural derive fails. Consumer compilation still does not prove derive
behavioral correctness.

Compiler success plus no non-sysroot shipping dependency excludes external
package types only for this narrow profile. Separate semantic checks still
reject concrete filesystem, environment, Resource/Host adapter, Domain,
transport, text, Client, presentation, and extension mechanism types from
Foundation and Kernel public surfaces.
