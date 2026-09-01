# Execution-Tool Reviews

This directory retains the human-auditable semantic review behind each entry
in `conformance/execution-tool-reviews.json`. The registry separates a stable
logical review from an exact host-environment binding; neither a tool name nor
a version range grants admission.

## Inventory

| Review ID | Logical role | Retained record | Active binding |
| --- | --- | --- | --- |
| `git-repository-proof` | Git repository identity and inventory proof | [Git repository proof](git-repository-proof.md) | `linux-x86-64-git-2-55-0` |

## Update discipline

An update must re-establish the exact executable path, bytes, size, raw version
output, ELF interpreter, and direct `DT_NEEDED` inputs. It must also re-audit
the complete argv/config/environment grammar and the bounded output parsers in
`tools/conformance/src/git.rs`. Finalize the retained record first, then bind
its SHA-256 digest in the registry and validate both schema and parser.

The listed ELF files are the direct loader inputs of the admitted executable,
not a claim that every file read by Git, libc, the dynamic loader, or the
kernel is enumerated. A new Product consumer or Product proof profile requires
a separate review; this inventory admits Repository Tooling only.
