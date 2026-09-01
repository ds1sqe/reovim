# Fidelity and Conformance Proof

[Interaction, Updates, Protocols, and View](../02-Interaction-Updates-Protocols-and-View.md)

## Fidelity and conformance proof

The v0.16 production proof attaches a faithful byte/hex Representation and a
Text Representation to one partially resident byte Resource and one Buffer.
Client-owned Views present both. A hex Intent updates Text where decoding
permits; a Text Intent produces exact source-Unit Operations; invalid byte
sequences remain visible and unchanged unless explicitly edited. An edit that
cannot preserve unexpressed state rejects or the form remains read-only.

The proof exercises Intent, Outcome, and Update seams through every required
in-process or supported Protocol composition. The release matrix names exact
contract/adapter versions, fixture digest or generator, and fidelity,
differential, outcome, coordinate, and resynchronization oracles. Adjacent
transport evidence does not qualify a protocol composition.

Each Presentation Adapter family supplies a deterministic sink or golden harness
for bounded writes, short/failed handoff, input decoding, close/restore, and
frame/result ordering. Client journeys run against that harness and every
supported production adapter.

See [Source Dependency Graph and Edges](../../01-Layer/01-Roles-Descriptors-and-Dependencies/03-Source-Dependency-Graph-and-Edges.md)
and [Resource, Snapshot, Buffer, and Transaction](../../02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md).
