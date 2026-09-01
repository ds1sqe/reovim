# Client, View, and Presentation

[Interaction, Updates, Protocols, and View](../02-Interaction-Updates-Protocols-and-View.md)

## Client and View

A Client owns every View it instantiates. View state includes Viewport and
visible-range policy, layout, clipping, focus, scroll, Client-local cursors/
selections, decoration, hit testing, input mapping, drawing, and frame
scheduling. A typed no-View Client path owns its declared output state and does
not invent View geometry.

A device or presentation coordinate is mapped to a stamped Semantic Location
before Intent submission. Carrying that location in one request does not promote
persistent View state into Kernel authority.

Shared, persistent, collaborative, or command-semantic state moves inward only
through a separately accepted contract defining authority, identity/version,
bounds, lifecycle, fencing, privacy, failure, and conformance. The CCM may
synchronize only such explicitly admitted state; it owns no View, rendered
frame, Buffer, or transport state.

A Client does not write storage, mutate Buffer/Projection internals, or retain a
second authoritative content buffer. It submits Intents against observed state.

A Presentation Adapter is a concrete Client-side target mechanism behind a
Presentation Port. It owns terminal/window/display/rasterizer/GPU/device I/O and
presentation handoff. For every instantiated View, the Client owns Viewport,
layout, input mapping, drawing/theme policy, frame scheduling, and negotiated
Rendering Capabilities. The adapter receives no editing authority.

Rendering is negotiated, not inferred from family name. A TUI may negotiate
cells, raster/pixels, video, or GPU acceleration. CLI, Web, and future families
negotiate their own output sets. Concrete device handles and encoded target
commands remain Presentation Adapter-private.
