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
linked test executable is not shipping proof. Consumer compilation is static
accessibility and type-checking evidence, not public behavioral conformance,
semantic completeness, ABI stability, runtime behavior, linker evidence, or
qualification. A separately registered and executed public-boundary test may
provide behavioral evidence; it never replaces the shipping or static
consumer proof.

## Closed public surface

Every Product source/module input reported by rustc dep-info receives the closed
syntax/type checks in the Product profile. The consumer must compile without a
private-module, shared-source, generated-source, or Repository Tooling bypass.

Each public-consumer relation owns a closed `public_items` path/kind set. A
conforming checker derives that set from the compiler-selected shipping source
and requires exact registry equality before considering consumer evidence.
`variant` is a distinct public-item kind: every public enum variant has its own
entry in addition to its enum's `type` entry. Each active relation contains the
complete source-derived set and no unregistered item.

Required public trait methods use the existing `function` kind. The Kernel
Interface set contains these required methods:

- `clock::MonotonicClock::descriptor`;
- `clock::MonotonicClock::observe`;
- `diagnostic::DiagnosticOutput::descriptor`;
- `diagnostic::DiagnosticOutput::submit`;
- `event::StartupEventAdmission::admit`;
- `event::StartupEventAdmission::descriptor`; and
- `event::StartupEventAdmission::scope`.

The current profile admits only required function members on a public trait.
Public struct fields, trait default bodies, associated constants and types, and
every other associated trait-member form are unavailable. Public trait
generics and supertraits are also unavailable.

The variant set closes externally resolvable variant names, not variant payload
shape or meaning. Likewise, a registered function name and a compiling item
witness do not close the function's full signature or semantic behavior. Enum
payload and function-signature semantic completeness, ABI stability, and
runtime behavior remain outside this proof.

Shipping value-expression paths have a separate closed guard. A path rooted at
`std` is unavailable in value-expression position, so source cannot introduce a
concrete standard-library mechanism through a qualified expression. The exact
type-level `std::error::Error` trait path remains admitted for the existing
shipping error implementations; that exception grants no `std` value access.
The only owned-container type exception is the exact
`::std::boxed::Box<[u8]>` spelling in the Kernel Interface Diagnostic Output
source. It admits no alias, other payload, other source, or `std` value path.

The current profile permits directly authored public declarations/modules and
the exact closed type-path/compiler-attribute sets. It also permits one narrow
source-derived re-export class required by the Kernel Interface consumer:

- the consumer is the exact registered edition-2024
  `reovim-kernel-interface` shipping closure and public relation;
- syntax is only an extern-prelude-anchored direct
  `pub use ::reovim_foundation::module::Type` or flat terminal type group;
- Cargo proves the exact unconditional normal edge to the registered
  `reovim-foundation` shipping closure;
- the provider is an authored public external module containing one direct
  public struct or enum definition; and
- Foundation and Kernel Interface relations contain the same source-derived
  item kind and complete built-in derive set under their respective paths.

Re-exporting an enum also derives its direct provider variant-name set beneath
the public alias. Those alias variant paths enter the same registry and
consumer equalities; their payloads retain the limits stated above.
A same-path canonical Foundation facade also derives direct public inherent
function and constant names. Contextual aliases do not duplicate those member
paths; their provider relation remains explicit.

Product registry v2 makes `reexports` required for every public relation. An
entry names `public_type`, `provider_relation`, and `provider_type`.
Foundation's set is empty. Kernel Interface contains the complete registered
set of direct Foundation mappings. The set is a closed equality target, never
authority for discovering source aliases.

The static resolver derives mappings from edition-2024 extern-rooted syntax,
Cargo metadata, digest-bound source, and the exact provider definition. It
requires equality with the registry after all Product source is inspected.
Deleting an alias, replacing it with a same-shaped local type, changing its
provider, or adding an unregistered alias fails closed.

Resolution work is cached and charged against the committed import, source-
file, aggregate-byte, and syntax-node bounds. Unqualified paths, `extern
crate`, globs, renames, `self`, nested groups, aliases, provider re-exports,
other packages or relations, private or conditional modules/items, qualified
or unsupported derives, duplicate names, and unresolved paths fail closed.
Public aliases, cfg-selected or macro-generated public items, and types from
an unproved external package or Repository Tooling remain rejected.

Static mapping proves only the bounded source and metadata facts above. Exact
Product proof separately binds the alias to its compiler-selected unit, the
proved Foundation public surface and `rmeta`, and the external consumer
compilation. Static success cannot replace that exact evidence.

The Source Role Map retains the resulting public item and derive sets, not a
second copy of the alias-origin mappings. Its standalone verifier compares
both complete reported sets with the embedded authoritative registry. It does
not independently rediscover alias origin from report bytes; that claim
depends on the exact source proof and compiler evidence named by the report.

## Exact consumer witnesses

Each validation library provides compiler-checked witnesses for its complete
registered surface. Public type witnesses use the exact extern-prelude-rooted
path, including `::reovim_foundation` or
`::reovim_kernel_interface`, as the explicit argument to an admitted derive
helper. Public free or inherent function, constant, and variant witnesses use
exact extern-prelude-rooted expression paths. A module witness is derived only
from a registered witnessed child whose exact path traverses that module. A
public trait uses an exact extern-prelude-rooted trait path and an exact
implementation whose required method-name set equals the registered `function`
entries.

The checker requires equality, not occurrence. A missing, duplicate,
wrong-kind, shortened, imported, renamed, qualified-self, wrong-crate, or
unregistered witness fails closed. Merely importing a public name, sharing its
source, or spelling the same local identifier is not consumer evidence.

An admitted built-in derive is compiler-generated behavior. The registry names
the exact derive, every affected public type, and the consumer operation/trait
bound that type-checks its intended public availability. An unlisted derive or
procedural derive fails. Consumer compilation still does not prove derive
behavioral correctness.

The three Diagnostic Output carrier-owning types have an exact empty derive
set and exact external `Send + Sync` witnesses. The fixed shipping profile
rejects authored or derived `Clone`/`Copy` implementations for those types.
This closes duplication authority for the selected source; it is not a general
negative-trait proof facility.

Consumer macros remain unavailable except one parenthesized
`format!("{value:?}")` call in the exact Diagnostic consumer source. The
checker requires exactly one string literal with that decoded value and no
additional macro argument. Debug payload-redaction behavior can therefore
execute without admitting hidden expressions or another macro surface.

Compiler success plus no non-sysroot shipping dependency excludes external
package types only for this narrow profile. Separate semantic checks still
reject concrete filesystem, environment, Resource/Host adapter, Domain,
transport, text, Client, presentation, and extension mechanism types from
Foundation and Kernel public surfaces.
