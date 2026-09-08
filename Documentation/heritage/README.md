# Heritage

These edited historical notes retain decisions, failure lessons, and migration
maps from earlier Reovim architectures. For current contracts, start with the
[documentation index](../README.md).

| Document | Retained value |
| --- | --- |
| [Architecture proposal](clean-architecture-proposal.md) | Why v0.9 separated kernel, drivers, and modules; limits of the design. |
| [Kernel phase history](project-kernel-phases.md) | Epic #150 milestones and issue references. |
| [Legacy lessons](legacy-memorial.md) | Rendering, latency, synchronization, and cache failures in v0.8.x. |
| [Phase 5 extraction](phase-5-extraction.md) | How legacy behavior informed fresh implementations. |
| [Phase 8 migration](phase-8-migration.md) | Historical source and crate mappings after the runner split. |
| [Buffer provider proposal](buffer-provider-proposal.md) | Unresolved virtual-buffer ownership and lifecycle questions. |
| [Client extensions](client-extensions.md) | The pre-CLM design for state bridges and client capabilities. |
| [Runner overview](runner-overview.md) | The former runner's orchestration and event flow. |

Paths, API names, completion reports, and measurements describe their recorded
era. Proposals do not establish what shipped; historical results do not
qualify the current implementation. Each subject is explained in its owning
note, with links for related detail.

The full originals remain in Git at `518179f1`, including omitted code
sketches and planning detail. Read a file from before this editorial pass
with `git show`, for example:

```sh
git show 518179f1:Documentation/heritage/clean-architecture-proposal.md
```
