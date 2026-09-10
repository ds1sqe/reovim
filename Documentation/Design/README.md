# Editing system design

Start with [one editing journey](00-overview.md), then follow the public calls
through state, Clients, execution and modules. Text/Markdown supplies the first
example; the Kernel also supports other state models.

1. [Resources](01-resources.md)

   - [Resource tree](01.1-resource-tree.md)
   - [Read/write access and results](01.2-resource-access.md)
   - [Interrupted requests and retained work](01.3-requests.md)

2. [Buffer](02-buffer.md)

   - [History, Undo and Redo](02.1-history.md)

3. [Observation](03-observation.md)

   - [Representation and semantic values](03.1-representation.md)
   - [Editing and atomic installation](03.2-editing.md)
   - [Save, export and rename](03.3-publication.md)

4. [Clients](04-clients.md)

   - [Sessions and participation](04.1-session.md)

5. [Runtime execution](05-runtime.md)

   - [Composition and wiring](05.1-composition.md)

6. [Kernel modules](06-modules.md)

   - [Resource and semantic providers](06.1-providers.md)
   - [modprobe and unload](06.2-modprobe.md)
   - [Semantic navigation, commands and Client help](06.3-module-composition.md)
   - [Native modules and service adapters](06.4-native-modules.md)

7. [Further Resource examples](07-examples.md)

   - [Modules, Sessions and two Clients](07.1-module-journey.md)
   - [Language services and peer observation](07.2-service-workflows.md)
   - [Optional Client Resource surface](07.3-client-resources.md)

8. [References](08-references.md)

Each topic owns its types, behavior and remaining questions. Examples use Rust
syntax; inline comments identify proposed SDK APIs. Snippets assume their
stated inputs and an enclosing function; `.await?` uses an async function
returning `Result`. Typed helpers use the common Resource request path. Exact
native/service adapter layouts remain open where indicated.

[Product goals](../README.md) own release scope. Runtime implementation and
tests supply evidence; design prose alone does not establish availability.
[Debugging](../debugging.md) and [performance](../performance.md) describe
diagnostics and measurement.
