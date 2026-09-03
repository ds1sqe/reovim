# Claims, Unavailability, and Verification

[Product v2 Proof](../05-Product-v2-Proof.md)

## Exact claim

A passing current profile proves, for its named compiler, target, and prepared
environment:

- the exact repository Product source/module inputs reported by rustc dep-info;
- conservative supplied `--extern` dependency edges;
- Cargo-selected member paths and tracked bytes for both shipping packages;
- the supported source/public syntax boundary; and
- compilation by both separate public-only nonshipping library consumers.

It does not prove every compiler/sysroot/runtime/helper file read, expanded
Rust, item-level cfg selection, arbitrary helper or proc-macro reads, generator
provenance, general resolved public API, Cargo-staged or archive bytes, linked
executables, a release bundle, runtime behavior, performance, support,
qualification, certification, or a malicious-toolchain sandbox. The executed
Kernel Interface integration test is separate contributor evidence; this
library-closure proof does not claim its executable or runtime behavior.

Every unsupported mechanism has a stable specific unavailable result. Unknown
or ambiguous behavior fails closed and is never assigned to the nearest
supported category.

These chapters define the current `authored-std-lib/v2` target. They do not
assert that the registry transport is implemented, that an attempted producer
passed, or that any revision is qualified. The claim above exists only after
committed code and independently verifiable retained evidence satisfy every
current v2 requirement; until then, exact Product proof remains unavailable.

## Generated source and helpers

Compiler dep-info establishes that rustc consumed a generated file; it does not
establish which input, program, environment, subprocess, or network response
produced those bytes. Cargo build-script events and `rerun-if-changed` do not
enumerate arbitrary reads. Proc-macro compilation/supply does not prove
invocation, emitted tokens, or runtime inputs.

Generated Product therefore returns `PRODUCT_GENERATOR_PROOF_UNAVAILABLE`
until one concrete generator relation has complete exact helper/input/
environment/process authority and proves output production as well as compiler
consumption. An upstream digest alone is insufficient. Build scripts and proc
macros have their own unavailable results and are rejected before execution in
the initial profile.

## Stable basis and standalone verification

The current profile uses stable Cargo metadata format 1, Cargo JSON messages,
`RUSTC_WRAPPER`, explicit rustc dep-info, and Cargo package list. Its private
workload request/result protocol is v2 and fixes the four ordered stages; v1's
single Foundation pair is not reinterpreted. Cargo unit graph/build plan,
expanded source, and rustdoc JSON are unstable and cannot enter the Rust 1.92
proof path.

The private workload observation and coordinator validate that chronological
order before report construction. Source Role Map arrays use canonical identity
order rather than chronology. Standalone report verification reconstructs the
exact direct-edge and fresh-provider partial order, but it does not prove that
Foundation validation ran before Kernel Interface shipping.

Standalone report verification validates schema/version dispatch, canonical
identity/order, result consistency, input references, and closure accounting.
It cannot recreate semantic proof without the exact source, policy, topology,
reviews, tools, compiler substrate, prepared environment, and raw captures
named by the report.

Frozen v1 schemas and a separate v1 semantic verifier remain available only for
historic report verification. Active generation uses the exact v2 schema tuple.
An unknown or mixed tuple fails before Cargo/tool execution; no optional-field
default upgrades one version into another.

The mechanism definitions and limitations are retained in the official
[rustc command-line documentation](https://doc.rust-lang.org/rustc/command-line-arguments.html),
[Cargo external-tool JSON contract](https://doc.rust-lang.org/cargo/reference/external-tools.html),
[Cargo dep-info contract](https://doc.rust-lang.org/cargo/reference/build-cache.html#dep-info-files),
[Cargo package command](https://doc.rust-lang.org/cargo/commands/cargo-package.html),
and [Cargo build-script contract](https://doc.rust-lang.org/cargo/reference/build-scripts.html).
