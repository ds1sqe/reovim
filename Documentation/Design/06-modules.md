# 06 — Kernel modules and SDKs

A Kernel module contributes services/providers through public Kernel and
component contracts. Kernel Runtime owns its activation and execution, whether
embedded or served remotely. This chapter's Module API and modprobe describe
Kernel modules. A Client module is an extension under its own
[Client contracts](04-clients.md), with a separate loader/lifecycle. It can
logically depend on Kernel services through [composition](05.1-composition.md).

| Term | Meaning |
| --- | --- |
| Package | Distribution containing code and passive declarations |
| Declaration | Name/version, role, imports, exports, state schemas and bounds |
| Module instance | Activated implementation owned by one Kernel Runtime |
| Provider | Implementation of one public contribution contract |
| Catalog | Authorized Resource naming admitted declarations/implementations |
| modprobe | Resolve dependencies, load and activate a named Kernel module in scope |

Code loading, execution, state scope and namespace are distinct. A host can
load code once for multiple instances with independent state and grants.
Declarations, catalogs, instances and retained state use Resource identity.
The [resource tree](01.1-resource-tree.md) names them without owning their
lifetimes. [Composition](05.1-composition.md) owns imports and wiring.

## Scope and state

Types: [ResourceId](01-resources.md), [caller access](01.2-resource-access.md)
and [DriverOps](05-runtime.md).

```rust
// Path: kernel/interface/src/module.rs; module: interface::module
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleScope {
    Global, // Shared in this Kernel Runtime; no process-wide singleton.
    Session(ResourceId), // Authorized Session reference in this caller scope.
}
pub struct ModuleContext<Imports> {
    pub instance: ResourceId, // This exact live instance, not a catalog alias.
    pub scope: ModuleScope,
    pub namespace: ResourceId, // Its namespace subject; this ID grants nothing.
    pub imports: Imports, // Granted caller access and required public services.
}
```

`Imports` is a module-defined struct of granted access and optional helpers.
Runtime supplies it after dependency/version/grant checks. The common SDK
also retains request recovery.
IDs and metadata cannot manufacture authority. Activation cannot depend on
looking up its own unpublished exports.

```text
global state   -> shared settings/services within this Kernel Runtime
Session state  -> this editing space's declared module values
live state     -> private implementation objects, tasks and caller access
// A module can request both Session state and explicitly granted global data.
// Reusing code does not share mutable instance state or grants.
```

[Session](04.1-session.md) owns retained context; its schema owner defines
meaning and sharing policy. A Session-scoped Kernel module still executes in
Kernel Runtime. Client extensions may separately access retained Session data
through public APIs. Remote access uses authorized proxies; raw IDs cannot
cross connection namespaces.

State schemas are versioned and separately owned by each module. Adding a
module must not add fields to a concrete Kernel Session struct. Retention
across code unload requires a data form whose storage/destruction needs no
module code; live private objects can instead keep the implementation loaded.
Restoration needs schema compatibility or explicit migration. Retention is
bounded and does not imply disk/crash persistence.

## Namespace

An illustrative layout within one authorized Runtime root:

```text
modules/catalog/markdown               // Passive admitted declaration.
modules/instances/markdown-1            // Exact activated instance.
modules/treesitter/{info,parse}         // Alias to a selected live instance.
global/modules/markdown/settings       // Shared state in this service.
sessions/work/modules/language/state   // Session-scoped Kernel module data.
sessions/work/client-state/vim         // Client-owned retained schema.
sessions/review/client-state/emacs     // Independent Client-owned data.
resource/notes                        // Source Resource.
buffers/draft/history                 // Buffer's editing history.
buffers/draft/representations/source   // Semantic access, not a rendered view.
buffers/draft/representations/preview  // Interpreted data for Client rendering.
```

Names are conventions/aliases, not new identity types or ABI dispatch paths.
A loaded implementation's code lives in its execution host; these Resources
expose declarations, instances or explicitly retained state. Ordinary private
fields need no tree entry. The scopes above apply independently of these names.

Kernel modules execute under Kernel Runtime; Client extensions have separate
owners even when they access server-retained Session state. Each Runtime has
its own authorized root; another owner's subjects require explicit
proxy/resolution and grants. A [future Client surface](07.3-client-resources.md)
can name optional exports under clients/{id} and participation under Sessions.
Module-owned namespace access permits only declared registrations/state;
removing a name neither closes shared data nor unloads code.

```rust
// Proposed common SDK; snapshot is an exact source Resource, never live bytes.
// parse_contract is the exact contract from the module's public declaration.
let parse = kernel.resolve("modules/treesitter/parse").await?;
let reply = kernel
    .write(parse, &Message::reference(parse_contract, snapshot))
    .await?;
// info is passive. A compile endpoint uses the same form if offered.
// Composition can supply this endpoint without hard-coding treesitter.
```

## Implementation contract

`State` is the module's private live implementation type, not Session data.
Both `State` and `Imports` are declared generic parameters here. This is the
same-build Rust factory surface; [native modules](06.4-native-modules.md)
require its binary adapter.

```rust
// Path: kernel/interface/src/module.rs; module: interface::module
pub struct ModuleImplementation<State, Imports> {
    pub construct: fn(
        context: ModuleContext<Imports>,
    ) -> Result<State, (ModuleContext<Imports>, Error)>,
    pub driver: DriverOps<State>, // Runtime pairs this with constructed State.
}
```

Construction creates an inert instance. On rejection, it returns the unchanged
activation context and leaves no installed contribution or external effect.
Fallible external initialization belongs to the driven Starting phase.
The owning Runtime retains State, its declared driver and required code;
the module receives no driver for the containing Kernel Runtime.

```text
construct -> Constructed       // Imports retained, exports not visible.
advance   -> Starting          // Bounded activation/progress outside Core.
status    -> Ready             // Runtime may publish ready contributions.
close     -> Closing          // Fence admission; settle owned work.
advance   -> Stopped           // Cleanup complete; release can finish.
// Module lifecycle reuses DriverOps; modprobe owns the activation request.
// Failure retains cleanup ownership. Result polling does not execute modules.
```

[modprobe](06.2-modprobe.md) owns activation ordering, failure and unload.
[Providers](06.1-providers.md) owns registration and provider/data lifetimes.
One module may register several authorized contracts. First-party and
third-party modules use these same entry points.
Runtime checks the declared required contributions as well as instance
readiness before publication; a module's Ready report alone is insufficient.

## Contributions and Client boundary

```text
Kernel module -> Resource / State Model / Domain / Representation providers
              -> semantic commands, language services and shared state schemas
Client module -> its Client's input, commands, views and rendering
// Client extension APIs/lifecycle are outside the Kernel module contract.
// A companion Kernel service is a separate instance with granted public APIs.
```

Kernel modules can consume granted Session state and, in the
[future Client Resource surface](07.3-client-resources.md), Client exports.
Kernel routes their typed access and enforces lifetime/grants. The consuming
service interprets the selected schema; Kernel needs no framebuffer, keymap
or viewport type. Exporting Client data leaves input/presentation ownership
with that Client. [Service workflows](07.2-service-workflows.md) shows examples.

[Clients](04-clients.md) owns essential Vim/Emacs behavior and resolver rules.
[Semantic module composition](06.3-module-composition.md) connects their
navigation dependencies and the read-only vim-motion-helper.
[Release scope](../README.md) owns the separate SDK/family obligations.

## Pairing, authority and SDKs

A distribution may package Kernel modules with companion Client modules.
Logical service requirements can cross their execution owners. Each owner
uses its own activation API; Kernel catalog activation edges select Kernel
instances. Composition coordinates required access across owners without
assuming shared loaders, ABIs or atomic cross-owner activation.
Clients without a local extension system can still use the public service API.

Declarations name role/family, compatible imports/exports, required/optional
dependencies, state scopes/schemas, trust and consumed bounds. Registration
and inspection execute no module code. Recheck grants before protected
access and acceptance. Replacement fences the exact old instance and calls.

Editing commands submit semantic edits through
[Representation](03.1-representation.md); only the authorized provider path
supplies prepared state to Runtime/Core. Accepted edits remain in History
after a command module unloads. No callback, wait or cleanup runs under
Buffer/Client state authority.

Code homes: `extensions/` for contributions, component APIs beside their
owners, `sdk/` for common language access and owning Runtimes for activation.
Module-specific wrappers belong to the module's API package; common SDKs
never enumerate every module surface. Consumers import the contracts they use.
[Native/service adapters](06.4-native-modules.md) own binary/language details,
ownership and isolation limits. Portable applicable Rust uses `no_std + alloc`;
native I/O stays in separately granted Host adapters.

The [complete journey](07.1-module-journey.md) shows separate module owners,
shared editing and unload blocked by retained state.
