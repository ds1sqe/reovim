# External Semantic Protocols

[Interaction, Updates, Protocols, and View](../02-Interaction-Updates-Protocols-and-View.md)

## External semantic protocols

**External Protocol Binding** is outside the Kernel and owns semantic
translation. **Protocol Adapter** separately owns encoding, framing, connection,
and carrier I/O.

For example, an LSP binding:

- resolves `DocumentUri` under authority to a Resource/Buffer/Text
  Representation binding without treating URI as Resource Identity;
- maps LSP document version and negotiated position encoding to exact Resource,
  Buffer, Projection, and Text coordinate-space stamps;
- translates line/character locations and edits through the Text
  Representation contract;
- keeps workspace and other protocol values binding-owned or uses separately
  negotiated orchestration contracts;
- uses public SDK/service APIs only; and
- returns explicit stale, encoding, mapping, gap, and resynchronization outcomes.

LSP diagnostics are LSP/Text semantic data, not Editor Event Stream records
merely because the protocol calls them diagnostics.

The v0.16 foundation proves translation shape with mutually incompatible Text,
structured, and intrinsic-image coordinate fixtures. A full external protocol
implementation requires separate release admission. Neovim parity admits only
the subset it actually requires; it does not imply full LSP.
