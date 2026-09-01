# Rendering Capabilities

[Client contract index](README.md)

## 7. Rendering is an explicit capability

Rendering is negotiated through an explicit **Rendering Capability**. It is
never inferred from a Client Family name.

A Family Contract may expose bounded contribution schemas such as cell, image,
raster, pixel, vector, video, canvas, or structured CLI output. A separately
admitted unsafe/native surface may expose only an opaque scoped lease under its
own trust, lifetime, synchronization, loss, and qualification contract.

Encoded terminal commands, DOM/window/native handles, codecs, rasterizers, GPU
API objects, queues, command buffers, device handles, and other platform
mechanisms remain Presentation Adapter-private. They do not enter the Kernel,
CCM, common Client contract, or ordinary module API.

### 7.1 Rendering Capability Descriptor

Each versioned **Rendering Capability Descriptor** names:

- capability identity/version and Presentation Adapter incarnation;
- accepted contribution and surface families;
- applicable cell, pixel, raster, image, vector, video, canvas, color, alpha,
  and coordinate rules;
- extent, damage, frame-rate, and stream bounds;
- CPU/GPU execution, memory, upload, readback, and copy limits;
- command, record, byte, queue, and in-flight bounds;
- ordering, fence, completion, presentation handoff, and frame-drop behavior;
- cancellation, backpressure, privacy, and shutdown;
- terminal, surface, capability, and device loss/recreation;
- required and optional behavior, fallback, and degraded readiness; and
- applicable performance workloads and evidence identity.

Naming a format or example does not select a terminal protocol, codec,
rasterizer, GPU API, dependency, backend, or implementation.

### 7.2 Contribution and installation

Every contribution and frame binds:

- Rendering Capability identity/version;
- Presentation Adapter and target/surface/device incarnation;
- Client, View, layout, render, and frame generations; and
- any opaque scoped-surface lease generation.

The Family Core validates bounded public contributions and routes them through
the Family Presentation Port. Client computation and completion installation
follow the Client State Lane contract. The Presentation Adapter owns target I/O
and handoff and receives no editing authority.

A Client Module declares required and optional Rendering Capabilities before
readiness. Unsupported behavior is a typed outcome.

### 7.3 Loss, recovery, and fallback

Capability, terminal, surface, or device loss:

1. stops affected admission;
2. advances adapter and capability fences;
3. resolves or visibly discards bounded in-flight work;
4. rejects stale submissions; and
5. re-enters negotiation.

Recovery creates a new incarnation and never revives prior-incarnation work.
Loss of a required capability removes the affected module or Client
contribution from `Ready`. Loss of an optional capability permits only the
descriptor-declared degraded readiness state.

A fallback that changes fidelity, format, coordinates, or semantics requires
explicit acceptance under a new descriptor. It is never a silent substitution.

### 7.4 Rendering and CCM

CCM synchronizes admitted semantic, collaboration, or module state. It does not
synchronize rendered cells, pixels, frames, video, GPU commands, or device
state as cross-family truth.

Media shared as source state uses a separately admitted Resource, Domain, and
Representation contract. Each receiving Client still presents that state
through its own negotiated Rendering Capability.
