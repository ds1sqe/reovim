# 08 — Behavioral references and reuse evidence

These sources explain the design questions, not compatibility or feature-parity
promises. The implications below are Reovim's interpretations; the owning
contracts state Reovim's behavior and keep the relevant rationale in context.

## Editing products

- Blender's mesh and UV editors use related but different coordinates, with
  optional selection synchronization. Its operators distinguish continuing
  interaction, cancellation and completion. This motivates explicit mapping
  and interaction lifetime, not universal coordinates or copying its executor.
  [UV selection][blender-uv]; [operators][blender-ops].
- GIMP Quick Mask exposes selection as an editable grayscale channel. Its
  image-owned Undo differs from display-only zoom and may group interactions.
  This motivates ownership by meaning, not treating every selection as a
  Client cursor. Reovim deliberately keeps alternative History branches where
  GIMP documents losing redo after a new edit.
  [Quick Mask][gimp-mask]; [Undo][gimp-undo].
- VS Code custom editors can own non-text documents and several views of one
  model. Notebooks separate serialization, execution and output rendering
  contributions. This motivates composable models and modules rather than
  requiring a new editor for each representation.
  [Custom editors][vscode-editor]; [notebooks][vscode-notebook].
- Neovim composes operators with motions/text objects, separates Buffers from
  windows, groups Undo and retains branches. Reuse those mechanisms without
  adopting text-shaped coordinates as Kernel types.
  [Motions][nvim-motion]; [windows][nvim-window]; [Undo][nvim-undo].
- Emacs active keymaps provide contextual command composition with defined
  precedence. Reovim's Client resolver needs explicit conflict rules without
  making point, overlays or text properties universal state.
  [Active keymaps][emacs-keymaps].
- Writer's paragraph, character, frame and page styles are authored content,
  even though they affect appearance. They motivate separating authored style
  from computed layout, not placing both under drawing policy. [Styles][writer].

## Controlled access and live protocols

Tree-sitter provides language-specific syntax nodes with source extents and
structural queries. Reovim's lang-tree adapter supplies the common function/
block meanings; navigation consumers use its public positional contracts.
This selects no universal parser-node schema or compiler-level semantics.
[Parser objects][treesitter-objects]; [queries][treesitter-queries].

Linux [sysfs][sysfs] routes attribute reads/writes to show/store callbacks.
For example, writing 1 to fbcon/rotate requests clockwise console rotation
under [fbcon's interface][fbcon]. Reovim uses that direct-write idea with whole
request/result contracts; it adopts no universal text or byte-file format.
Lookup and observation never activate settings or hardware.

Linux [filesystem registration and file operations][vfs] motivate separate
provider registration, caller access and implementation lifetime. Reovim's
[composition](05.1-composition.md) uses typed contracts without requiring
every Resource to behave like a byte file or device node.

[UDP][udp], [Linux GPIO][gpio] and [WebSocket][websocket] expose different
message, sample and connection lifetimes. Their concrete implications and
failure cases belong to [live examples](07-examples.md). A connection being
inspected is not the transport carrying Reovim's own service requests.

## Existing code

Current implementation evidence is deliberately narrower than this design:

- [Resource journeys](../../apps/server/tests/resources.rs) exercise dynamic
  registration, namespace isolation, recovery and two distinct state models.
- [Core installation](../../kernel/runtime/src/history.rs) changes immutable state
  and History under one authority. This is not a measured lock-cost claim.
- [Native loading](../../apps/server/tests/native.rs) builds a C Resource
  provider after Runtime starts and invokes it through the common API.
  [Native models](../../apps/server/tests/native_model.rs) exercise edit/Undo;
  [C callers](../../apps/server/tests/c_caller.rs) use the caller table.
  Complete lifecycle and compatibility conformance remain incomplete.
- [Diagnostics](../../kernel/runtime/src/diagnostics.rs) records bounded
  request transitions. It is not yet a live log Resource/Buffer adapter.

A log Resource must preserve stream identity, visible gaps, redaction and
independent bounded readers. It must not make an unbounded shadow copy or
generate its own content indefinitely. Startup/freeze diagnostics must remain
available without Buffer readiness. [Debugging](../debugging.md) owns that
contract; [performance](../performance.md) owns measurement.

Prior code is a behavior donor, never the authority for ownership or domain
shape. A selected migration must retain useful fixtures, reject invalid text
coupling and prove the new public path. Source inspection alone cannot claim
runtime support, public API compatibility or a completed migration.

[blender-uv]: https://docs.blender.org/manual/en/4.5/editors/uv/selecting.html
[blender-ops]:
  https://developer.blender.org/docs/features/interface/operators/
[gimp-mask]:
  https://docs.gimp.org/3.0/en/gimp-image-window-quick-mask-button.html
[gimp-undo]: https://docs.gimp.org/3.0/en/gimp-concepts-undo.html
[vscode-editor]:
  https://code.visualstudio.com/api/extension-guides/custom-editors
[vscode-notebook]: https://code.visualstudio.com/api/extension-guides/notebook
[nvim-motion]: https://neovim.io/doc/user/motion/
[nvim-window]: https://neovim.io/doc/user/windows/
[nvim-undo]: https://neovim.io/doc/user/undo/
[emacs-keymaps]:
  https://www.gnu.org/software/emacs/manual/html_node/elisp/Active-Keymaps.html
[writer]: https://books.libreoffice.org/en/WG252/WG2509-WorkingWithStyles.html
[sysfs]: https://docs.kernel.org/filesystems/sysfs.html
[vfs]: https://docs.kernel.org/filesystems/vfs.html
[fbcon]: https://docs.kernel.org/fb/fbcon.html
[udp]: https://www.rfc-editor.org/rfc/rfc768.html
[gpio]: https://docs.kernel.org/userspace-api/gpio/gpio-v2-get-line-ioctl.html
[websocket]: https://www.rfc-editor.org/rfc/rfc6455.html
[treesitter-objects]:
  https://tree-sitter.github.io/tree-sitter/using-parsers/1-getting-started.html
[treesitter-queries]:
  https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html
