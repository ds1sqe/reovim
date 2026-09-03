# Prepared Root and Bounded Execution

[Product v2 Proof](../05-Product-v2-Proof.md)

## Prepared execution authority

Product proof requires a Linux prepared execution root. The parent checker
creates and holds an initially empty canonical owner-only `0700` mountpoint
outside the repository. It starts a dedicated, initially single-threaded same-
binary setup subprocess in a new parent-visible process group. An exact
reviewed Repository Tooling dependency supplies safe namespace, mount, chroot,
resource-limit, signal, wait, and bounded Netfilter-netlink calls while first-
party source remains unsafe-free; no external-command fallback exists.

Setup creates and verifies the user namespace and one UID/GID map first, then
private mount, network, and PID namespaces plus private propagation. A second
same-binary child becomes namespace PID 1, arms and verifies a parent-death
signal, completes a parent-owned pipe handshake, mounts `/proc`, and reaps
adopted descendants. Namespace identities and mount state are verified rather
than inferred from process success.

Setup mounts one fixed-size/fixed-inode tmpfs with
`mode=0700,nosuid,nodev,noexec`. It constructs a minimal prepared root there.
A versioned exposure manifest binds each visible mount's exact source,
destination, filesystem/type, flags, purpose, and bounded visible inventory.
Each retained inventory contains its sorted relative paths, closed entry kinds,
raw modes, logical sizes, file-content digests or device identity, totals, and
aggregate digest. The standalone verifier recomputes that digest and joins
file entries to repository, executable, runtime, or compiler-substrate bytes.
The target-libdir directory inventory must equal the selected substrate file
set. An opaque, merely well-formed inventory digest is not proof.

The Product registry has one protocol-fixed prepared exposure at
`/repo/conformance/product-closures.json`. It is the same regular,
non-symlink raw file admitted by preflight, mounted read-only and
non-executable, and retained in the exposure manifest and repository
inventory. No caller-selected path, alternate registry, writable copy, or
second serialization is admitted.

The v2 workload request carries the registry's exact raw byte count and
SHA-256 identity, never the registry body. The host and prepared child each
perform their own bounded raw read, duplicate-key and malformed-JSON
rejection, typed registry parse, and exact identity joins. Those joins bind the
caller's typed value, preflight file identity, prepared exposure inventory,
request identity, child-observed raw bytes, and child-parsed registry. A
missing, retargeted, changed, over-limit, or semantically different registry
fails before its observations can enter evidence.

The existing 64 KiB environment/argument carrier remains a control plane for
bounded identities, limits, and setup selection; the registry body does not
enter it or enlarge its bound. A host-only acceptance inspection measures the
complete encoded configuration without creating a namespace, mount, or child.
A transport/request-shape change retains that inspection's actual byte count,
limit, and positive headroom in its verification record before a live retry.
These acceptance values are not Source Role Map fields, and a Product report
makes no per-run headroom claim.

Production applies the same encoder and 64 KiB ceiling at both hops. Tests
require byte-identical hop payloads for the same typed configuration.
First-hop overflow fails before setup; second-hop overflow fails before PID 1
starts, and setup cleanup remains mandatory.

Host-backed system runtime, compiler substrate, checker, and repository views
are read-only; Product source is also non-executable. Host home, host temporary
directories, ambient Cargo caches, and unlisted writable mounts are absent.
Runtime-library resolution canonicalizes host default-search directory aliases
before choosing prepared destinations. It preserves the ELF interpreter's
literal path separately. A host `/lib*` symlink must not become a different
real directory inside the prepared root and thereby change loader semantics.
The only device exposure is the exact read-only `/dev/null` character device.
It is `nosuid,noexec` and intentionally not `nodev`; every other exposure is
`nodev`, and no host `/dev` directory is visible.

The network namespace has no host or external carrier. Its closed interface,
route, socket, and policy state is verified and reported; a private namespace
alone is not evidence that no interface exists.
The current Linux profile separately retains the two exact kernel-generated
unreachable IPv6 baseline rows on `lo`; they are not external routes. A route
with any other fields or interface is external-carrier evidence and fails.

All Cargo homes/temp variables, target/build directories, wrapper traces,
dep-info, package staging, and normalized staging use the one tmpfs. Mount or
containment failure never falls back to the underlying directory. The scratch
authority cannot publish directly to the durable evidence authority.

Read-only binds plus pre/post source identities do not defend against hostile
same-authority host mutate-and-restore. That concurrent host action, root
intervention, and a compromised admitted toolchain are outside this proof and
cannot inherit its claim.

## Bounds and execution-tool admission

The tmpfs size and inode options hard-cap allocated scratch filesystem blocks
and inode count. They do not cap heap/RSS, pipe buffers, process count, kernel
objects, or sparse-file logical length. Exact per-process limits bound address
space, CPU, descriptors, output-file logical size, and core dumps. Capped pipes,
path/file/unit/trace/dep-info/message/logical-byte counts, JSON depth/items, and
deadlines cover their declared dimensions. Post-observation counts and syntax-
visit ceilings do not claim preventive aggregate memory or parser-peak bounds.

`reovim-execution-tool-reviews/v1` separates environment-independent semantic
reviews from exact environment bindings. A review fixes logical role,
invocation families, required semantics, consumers/profiles, upstream/license,
assumptions, invalidators, and retained record. A binding fixes one environment
identity, host and prepared paths, executable/version/runtime-input bytes, and
the review it realizes. The invocation selects one environment ID explicitly.

There is no PATH/basename lookup, version range, wildcard or alternative digest,
runtime-generated admission, or automatic enrollment. Candidate mode may
describe a proposed binding but cannot admit it. The report identifies each
invoked binding and its pre/post identity. Missing, substituted, drifted,
unreviewed, or environment-mismatched tools fail before execution/publication.
The linked namespace dependency is admitted through dependency-reviews, not
duplicated as an executable binding. Cargo/rustc/compiler substrate retain their
exact Source Role Map identities; Git and any other independently executed
helper require execution-tool admission.

## Descendant ownership and cleanup

One outer supervision deadline owns group signaling, namespace PID 1,
descendant termination, leader reap, bounded reader joins, namespace exit,
mount disappearance, mountpoint identity, and empty underlying directory.
Overflow, timeout, setup failure, malformed result, or coordinator failure
triggers whole-group termination through the reviewed signal authority.
Namespace teardown owns descendants that fork, change group/session, or retain
a pipe.

Evidence is accepted only after all owned processes, readers, mounts, and
scratch entries are proved gone and source/tool identities remain unchanged.
Any indeterminate signal, process, reader, namespace, mount, identity, or
cleanup state aborts before report publication.
