# Lifecycle and modules

This is a target contract. Names below describe operations and states, not an
existing SDK. [Goals and release scope](../README.md) own language/family scope.

## Owners and boot

Assembly selects concrete components and constructs an inert dependency graph.
Missing or ambiguous selection is a typed error before activation. Assembly
may call root startup; it never writes another owner's lifecycle state.

Kernel Runtime owns Kernel-side activation and Server Module attachments.
Each Client Runtime independently owns its family, modules, input, views, and
presentation. A session joins these lifetimes; attaching a client does not
transfer Kernel ownership, and losing one client does not shut down others.
The outside-Kernel process supervisor owns child launch/reap mechanics, not
module state. Protocol adapters own carriers, not semantic readiness.

Each owner follows this lifecycle with an incarnation/generation:

```text
Constructed -> Starting -> Ready -> Closing -> Closed
                  |                  |
          ActivationFailed   IncompleteShutdown
```

Starting validates contracts, capabilities, grants, limits, and dependencies,
then provisionally attaches children in dependency order. Ready publishes only
when every required child predicate and initial synchronization succeeds.
Optional failure permits only a declared degraded result naming unavailable
capabilities and remaining safe operations. Construction alone grants no use.

Activation failure exposes no partial contribution. The owner fences that
generation, cancels provisional work, and rolls back children with dependants
before dependencies. Failure reports rollback outcomes, including unresolved
work; `ActivationFailed` never implies that unfinished execution disappeared.

Boot example: assemble resource/domain providers and Host grants; start the
Kernel; negotiate a client's session; initialize its input/presentation and
required modules; obtain the initial semantic snapshot; publish client Ready.
If its required renderer fails, roll back that client's provisional subtree.
The already-ready Kernel and unrelated clients retain their own lifetimes.

## Operation and shutdown semantics

Attach returns an owned session/instance with negotiated scope or a typed
denied, unsupported, unavailable, not-ready, or exhausted result. Failed attach
publishes nothing. Requests validate readiness and generation at their defined
acceptance point; old instances cannot submit or install into replacements.

Close/detach is idempotent. It stops admission, publishes Closing, fences late
installation, requests cancellation, and boundedly drains/discards/reconciles
owned work. It removes subscriptions, contributions, and memberships before
closing dependencies and releasing resources. Detaching a client does not
implicitly save, discard, or close shared Buffer state.

Every phase has an outer completion bound. A timeout ends a wait, cancellation
requests cooperation, and fencing rejects stale results: none proves execution
stopped. Closed requires owned execution to be terminal, or the isolation owner
to prove its containing process reaped. Otherwise retain ownership and report
IncompleteShutdown; do not unload code still executing. Queued-result discard
does not by itself prove its producer stopped.

External effects retain their actual certainty and reconciliation owner across
disconnect or shutdown. A lost response never authorizes blind mutation retry.
Replacement uses new module/session/handle generations, completes required
readiness, and switches routing without accepting old-generation completions.
Shutdown callbacks never run while an authoritative state lock is held.

## Module registration and capability grants

A package may provide Server Modules, family-specific Client Modules, or a
pair. A descriptor declares identity/version, compatible public contracts,
role/family, activation conditions, contribution points, dependencies, required
and optional capabilities, requested scope, trust/process class, bounds, and
lifecycle. Discovery and descriptor validation execute no module code.

Registration is provisional until activation commits. Runtime contributions
must be a typed subset of the descriptor; undeclared authority or contributions
reject activation and roll back. A grant binds an instance/session, capability,
scope, authority generation, and lifetime. Identity or registration alone grants
nothing. Validate authority before any lookup that could reveal protected data.

Contribution points name their schemas, invocation/reentrancy rules, errors,
completion, cancellation, delivery/pressure limits, and removal behavior:

- Server: semantic commands, Domain/Representation providers, diagnostics, and
  explicit shared-state schemas. Resource adapters retain their own boundary.
- Client: keymaps/resolvers, mode, view/layout, decoration, output, and
  negotiated render contributions through the family's public contract.

Modules request edits through the normal semantic/transaction path. They never
mutate storage or own Buffer/History, transport, Host, or presentation handles.
Each server/client instance has separate grants, queues, generations, and
lifecycle even when a package, carrier, or process is shared.

Required server/client pairing publishes aggregate readiness only after both
owners and any required synchronization scope are ready. Optional pairing names
its degraded behavior. A coordinator invokes public attach/rollback/close; it
cannot write child state. Pair data uses a versioned shared schema and named
authority, not an arbitrary private editing channel.

Example: a Markdown package declares a Server command and a GUI Client preview
contribution. The command submits an edit through its grant; the preview uses
stamped Representation updates. Unload removes command/keymap/view registrations
and subscriptions, cancels owned work, and fences late results. Accepted
edits remain in History; unloading does not undo them.

## SDK and execution boundary

Rust is the first trusted in-process SDK. Public-only Rust journeys establish
the shared semantics, lifecycle/failure behavior, and measurements before later
language SDKs. A service/production-transport reference then proves negotiation,
ordering, bounds, cancellation, reconnect, and process-failure behavior before
Python and TypeScript bindings. A GUI implementation language does not change
the Module binding matrix or bypass the service boundary.

Python and TypeScript extension services are out of process. Their adapters
map the versioned language-neutral contract into idiomatic APIs; neither adds
an embedded interpreter to Kernel or Client dependencies. No engine, native
ABI, sandbox, additional language, or Neovim plugin compatibility is selected.
Protocol adapters own encoding and transport. Language adapters preserve typed
outcomes, revisions, callback identity/lifetime, and capability limits.

Request identity is scoped to the live session. Reusing an ID with a changed
payload or target is invalid; identical replay follows declared duplicate
handling, never a second untracked edit. Bound outcome retention and expiry.
An expired outcome requires reconciliation, not an assumed failed mutation.

Trusted in-process code shares process crash and memory-failure risk. Grants
are not a sandbox. Out-of-process containment claims require tested isolation
and termination. Never invoke or wait for module code while holding Buffer or
Client state locks; validate returned work again before state installation.
Slow modules have bounded queues/work and cannot stall unrelated consumers.
Panic must not unwind across a non-unwind-safe public boundary.

Private intra-tree APIs may change with coherent caller updates. Public v0.16
SDK/service APIs remain Experimental, with explicit compatible versions,
fixtures, and migration guidance for breaking changes. Stable compatibility
starts only at a separately admitted full SDK stability release.

## Author and failure checks

Every offered binding needs a public-only runnable example and a concise guide
for registration, grants, invocation, events, debug, testing, and unload.
A small local extension may use visible conservative descriptor defaults;
defaults must not infer extra authority. Avoid a new generator or registry
framework until that author journey needs it.

Test failed boot rollback, denied activation with no published contribution,
required-pair failure, unload during work, stale callbacks after replacement,
disconnect with accepted edits, and incomplete shutdown without unsafe unload.
Third-party review covers trust, dependencies, compatibility, maintenance, and
measured cost. First-party modules use the same public APIs and grants.
