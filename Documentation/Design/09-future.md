# Future design directions

This section records researched directions whose detailed interfaces are still
open. Its examples are pseudocode, not implemented SDK signatures. Existing
chapters retain ownership of their contracts; [product goals](../README.md)
own release obligations and the language/Client support matrix.

Required Client modules, configured startup and TUI/CLI/Web support remain in
the release scope. These pages explore their authoring surface and possible
extensions; placing that research here does not make required behavior optional
or promise additional runtimes, languages or devices.

## Reading order

| Topic | Owner |
| --- | --- |
| One Client module across Clients, schema exports, commands and hosting | [Client modules](09.1-client-modules.md) |
| Settings, keymaps, Client filters and user languages | [User configuration](09.2-user-configuration.md) |
| Optional remotely accessible Client services and observations | [Client Resource exports](07.5-client-resources.md) |

The module chapter explains what configuration can inspect and invoke. The
configuration chapter explains how users select targets, supply values and
connect input to those exported operations. Remote Client Resource exports
are an optional access path with their own unfinished protocol; they are not
a prerequisite for local module discovery or keymaps.

## Connect the research to existing owners

Start with the [Map journey](07.6-map-journey.md) for a complete worked example:

```text
.map file Resource -> Map factory -> Buffer + History
    -> immutable Map snapshot -> map-client context and presentation
    -> semantic object edit -> accepted Buffer state
    -> explicit Save -> conditional publication to the file Resource
```

Camera movement and zoom belong to the Client context in that journey. The
new chapters describe how one map-client module exports those behaviors and
settings for CLI, TUI and Web, with further interaction methods possible.

[Domain APIs](07-domain-apis.md) explain how an ordinary Kernel module supplies
Mesh/Map operations. [Kernel modules](05.2-modules.md) define provider
contributions; [state providers](06.3-state-providers.md) define the common
Resource/Buffer extension interfaces. The Map module's private read_map and
write_map functions handle its file format within those existing interfaces.

[Clients](04-clients.md) own input and presentation, including server-hosted
rendering. [Composition](05.1-composition.md) owns dependencies and granted
access; [declarations](06-declarations.md) own API identity and checked values.
Future Client schemas build on those boundaries. Detailed Client registration,
invocation, configuration and language adapter records remain to be specified
at their respective API owners before implementation.
