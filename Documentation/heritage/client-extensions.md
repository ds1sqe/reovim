# Client extension proposal

Historical design for #468 (which-key) and #469 (command-line UI), following
the server/client separation in Epic #465. The later Client Layer Model
(CLM) v6.3 replaced `ClientExtension` with `ClientModule`.
The [heritage index](README.md) explains how to read the full original.

## Component and state ownership

| Component | Role in this proposal |
| --- | --- |
| Server module | Editing policy through `Module`; dynamically loaded with `declare_module!`. |
| Client extension | Platform-specific UI or local behavior; compiled into TUI or bundled for web. |
| Feature | User-visible capability combining a module and zero or more client extensions. |
| Session extension | Typed mutable state stored by `TypeId` in an `ExtensionMap`. |

The proposal kept editing state on the server, including content, cursors,
modes, and selections. Clients owned frame buffers, compositors, DOM,
scroll state, and the rendering of overlay data.

The resolver split (#474) distinguished two server-side storage scopes:

| Map | Scope | Examples |
| --- | --- | --- |
| `shared_extensions` | All clients in a session. | Keymap registrations, completion sources. |
| `client_extensions` | One client within a session. | `VimSessionState`, `CmdlineState`, `OperatorPendingState`. |

Per-client command input and pending operators had to stay isolated: one
client's `:` prompt or motion count must not change another's state.
A session extension was storage, not a separately loaded module.

## State bridge and protocol

`ExtensionStateBridge` adapted typed state to JSON. It declared `kind`,
`scope`, `snapshot`, and `is_active`; the runner passed the map selected by
its Shared or Client scope. `CmdlineBridge` used the requesting client's map.

```text
Server module -> SessionExtension in the selected ExtensionMap
                                      |
                                      v
                             ExtensionStateBridge
                                      |
                                      v
                             JSON over gRPC v2
                                      |
                                      v
                            Client extension -> UI
```

Bridges belonged in `server/lib/drivers/session/src/bridges/`, beside the
state they adapted. The server registered them with its gRPC handlers.
This required adding JSON serialization to the session driver.

| Proposed endpoint/message | Purpose |
| --- | --- |
| `ExtensionService.GetState(kind, client_id)` | Return activity and a JSON snapshot. |
| `ExtensionService.ListExtensions` | Discover kinds and push/query support. |
| `ExtensionUpdatedPayload` | Notify clients with kind, JSON data, and client ID. |
| `KeymapService.GetBindings(prefix, mode, client_id)` | Return matching keys, descriptions, categories, and command IDs. |

Clients would pull initial or requested state, then receive pushed updates.
The existing `StateChanges -> build_notifications -> emit_notification`
pipeline would also carry extension updates.

JSON avoided a protocol-message definition for every extension, at the cost
of schema validation and parsing on both sides. Each bridge needed a documented
payload shape; an unchanged outer proto did not establish payload compatibility.

## Registration and routing

A `defaults` package selected concrete extensions and their deserializers.
The client engine called its factory and used extension contracts without
naming individual features.

```text
Build: defaults factory -> extensions + deserializers -> router
Route: gRPC event -> deserialize once -> topic lookup -> subscribers
```

Extensions declared `subscriptions()`. The router built a topic-to-subscribers
table, delivered events in registration order, and could `rebuild_routes()`
after subscription changes. Topics were opaque keys to the engine; dispatch
did not branch on extension IDs or concrete types.

For TUI, the deserializer registry produced `ClientEvent` values with
`Any + Send + Sync` payloads. An extension recovered a typed reference through
`downcast_ref::<T>()`, which could return `None`. Web used decoded `unknown`
payloads and extension-specific type guards. Both designs decoded once per
event rather than once per subscriber.

This resembled the kernel EventBus conceptually, but client topic routing
used string keys and explicit route rebuilding. It did not inherit EventBus
priorities, event consumption, or drop-based subscriptions.

## Optional capabilities and lifecycle

The TUI design used bridge methods on one `ClientExtension` trait:

| Capability | Discovery | Default |
| --- | --- | --- |
| Rendering | `composable()` / `composable_mut()` | `None`. |
| Input participation | `wants_input()` | `false`. |
| Key handling | `handle_key()` | Unhandled. |
| Activity | `is_active()` | Reported by the extension. |

A rendering extension implemented `Composable` on its concrete type and
returned `Some(self)` through the bridge. This avoided forcing render methods
onto background extensions or downcasting to concrete extension types.
Web used `getRenderer()` with `DomRenderer` for the same capability choice.

`wants_input()` opted into router selection; `Composable::captures_keyboard()`
controlled propagation to lower compositor layers. A status indicator could
render without input, and a macro recorder could handle input without rendering.
The proposed router selected the last registered active input participant.

```text
Server: discover -> probe -> init -> running -> exit -> unload
Client: create -> index subscriptions -> receive events -> destroy
```

Server initialization could defer and retry. Client extensions managed their
own active/inactive transitions from events; hidden extensions still received
subscribed events. The engine queried activity and capabilities, then passed
active composables to the compositor. Disconnect ended their lifecycle.

## Representative features

**Which-key (#468):** a client-only popup over existing server keymap data.
After a prefix timeout, `keymap.timeout` triggered a `GetBindings` query.
The extension displayed the results, then hid on the next key or a
`mode.changed` event. No new server policy module was needed.

**Command line (#469):** server-owned `CmdlineState` paired with a client
renderer. For `:wq<Enter>`, the server entered command-line mode, updated
prompt/input/cursor state, and emitted `mode.changed` and `cmdline.updated`.
The extension displayed those values; input returned through the server's
`TextInputSink`. Execution and return to normal mode hid the prompt.

| Later feature idea | Server side | Client side |
| --- | --- | --- |
| Completion | Sources, completion state, and bridge. | Menu and selection interaction. |
| Diagnostics | LSP diagnostics data. | Gutter and inline presentation. |
| Explorer | VFS and tree state. | File-tree panel. |

`shared/clients/model/` was intended to share positioning, z-order, and
interaction logic, while each platform rendered independently. Simple
`LogicalOverlay` descriptors could cover tooltips; interactive prompts and
menus could use dedicated extensions.

## Tradeoffs and limits

Compiled-in client extensions avoided introducing another dynamic-library
boundary. A replaceable defaults package made custom builds possible, but
adding extensions still required rebuilding or bundling the client.
Client-side rendering also meant platform-specific implementations and more
state synchronization traffic.

The proposed server changes were additive to `Module`, `declare_module!`,
`ServiceRegistry`, and `ExtensionMap`. The intended first implementation was
CmdlineBridge plus its protocol path, followed by TUI, which-key, web, and
completion. These were implementation intentions, not completion evidence.

The sketch left gaps: its example router silently dropped unknown or
undecodable events, and its notification discussion did not fully specify
per-client filtering or recovery after missed updates. Activity transitions
alone would also miss content changes while a prompt stayed active.
Current requirements belong to the [client contract](../Design/04-clients.md).
