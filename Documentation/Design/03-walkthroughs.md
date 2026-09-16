# 03 — Walkthroughs

Three journeys against `00-core.md` and `01-core-runtime.md` alone. Each step names the
core mechanism it uses. Anything a journey needs that the core lacks is
listed at its end. Packages named here (`text`, `history`, `publish`,
`lsp`, `mesh`, `action`) are declarations outside the Kernel.

Common setup: `fs` Module mirrors a directory lazily under `/module/fs`.
A Module's root is Kernel-served, so a Module that wants to be asked for
something registers an rd for it at start; `/module/text/open` below.

## 1. Text: open, edit, undo, save

`text` registers `/module/text/open` offering `text.open(source: Rd)
-> Rd` WRITE. It returns the existing Buffer for that source or makes one.

```text
Client                                  Kernel / Driver
lookup 0 "module" → 2, "fs" → 5, …      fs.lookup registers lazily
… "notes.md" → rd 40
open(/module/text/open, WRITE) → h1
call(h1, H_open, open, {source: 40})    text: open(40, READ), read bytes
                                        register(d, /module/text,
                                          "notes.md") → 41
                                        register(d, 41, "source") → 42
                                          content = 40      (pointer rd)
                                        report(41, RW, [H_text, H_history,
                                          H_publish])
              ← Ready(41)               Create on /module/text
open(41, READ|WRITE) → h2
c = cursor; read(h2); since(c)          coherent first view
call(h2, H_text, replace, …)            text: node r1, head = r1
              ← Ready(Ok)               changed(41, Modify)
call(h2, H_history, undo, ())           head = r0
              ← Ready(Ok)               changed(41, Modify)
call(h2, H_history, redo, ())           head = r1
call(h2, H_publish, save, ())           text: open(40, WRITE), write
              ← Pending(t)              fs: changed(40, Modify)
ask(t)        ← Ok                      done(t, Ok); published = r1
close(h2)                               text counts opens; keeps 41
```

Used: seven slots, `register`, `report`, `changed`, `done`, two rights,
Create and Modify, Pending, a pointer rd, the cursor rule.

- Save is a call on the Buffer rd. The `source` child is a Read pointer;
  the Driver opens the source itself with WRITE.
- `text` watches 40 and sees its own save as Modify. It ignores the one
  it expects. Driver policy.

## 2. LSP: rename a symbol across five files

`lsp` registers `/module/lsp/actions` at start. After the language
server's handshake it registers `/module/lsp/actions/rename` offering
`lsp.rename(buffer: Rd, at: U64, name: Utf8) -> Result` WRITE and
`action.run` for callers that know nothing. `lsp` watches every Buffer it
has seen, feeds the server each Modify, and remembers the `history`
revision it sent.

```text
Client                                  Kernel / Driver
watch(/module/lsp/actions)              Create rename → key enabled
open(rename, WRITE) → h3
call(h3, H_lsp, rename,
  {buffer: 41, at: 120, name: "b"})     lsp: asks the server
              ← Pending(t)
                                        server answers: 5 files
                                        for each file i, in parallel:
                                          call(text.open, {source: f_i})
                                            → rd_i   (existing or new)
                                          open(rd_i, WRITE)
                                          call(H_text, replace_many,
                                            {edits, expected: rev_i})
                                        four Ready(Ok), one Err(Stale):
                                          the user typed in file 3
                                        done(t, Ok{applied: 4,
                                          stale: [rd_3]})
ask(t)        ← Ok{…}                   Modify on 41 and three others
                                        rd_3 untouched
```

Used: Create as capability discovery, Pending across an external
process, five parallel calls to one Driver, per-rd precondition through
`history`, Stale, `done` with a domain result.

- No cross-rd transaction. `lsp` is the coordinator: it reports what
  applied and what did not. Undoing the four on Stale is its policy,
  done with four `history.undo` calls.
- Server crash: `unregister(rename)`. Handles get Gone, watchers get
  Delete, the key is disabled. No Kernel state remembers the server.
- `cancel(t)` is forwarded; `lsp` answers Cancelled or, if too late, the
  real outcome.

## 3. Mesh: sculpt, derive, render

`mesh` opens `/module/fs/…/model.obj`, registers Buffer rd 60 offering
`mesh` and `history`, and keeps vertices in its own memory, GPU or not;
the Kernel sees a Read/Write rd. A `subdiv` Module opens 60 as source
and registers derived rd 61 with a `source` pointer, recomputing on
Modify. A GUI Client renders 61; a TUI Client shows counts from 60.

```text
GUI Client                              Kernel / Driver
open(60, READ|WRITE) → h4
open(61, READ) → h5
c = cursor; read(h5, at, buf); since(c)  vertex bytes, positional
brush stroke: 200 vertex moves
call(h4, H_mesh, move_many, [...])      mesh: one node r7, head = r7
              ← Ready(Ok)               changed(60, Modify)
                                        subdiv: since → Modify on 60
                                          read(60) → recompute
                                          changed(61, Modify)
since → Modify 61; read changed range   GUI redraws with its own camera
TUI: `u` on 60                          Vim Plugin: history.undo
                                        mesh: head = r6; Modify 60, 61
import 4 GB scan
call(h4, H_mesh, import, {source})      mesh: over its own limit
              ← Err(NoSpace)            Kernel held no size
```

Used: batch as one node, several Drivers over one source, pointer rd,
positional read, Modify fan-out, `history` shared by hash between a Vim
Plugin and a mesh Driver that never met, Driver-enforced pressure.

- Camera, selection and viewport are Client state. No rd, no revision.
- If a `gpu` Module serves a device as `/module/gpu/0` with a `compute`
  api, `mesh` opens and calls it like any rd. The Kernel has no GPU word.
- `mesh` may publish `/module/mesh/<buf>/size` as a Read rd, or offer
  `pressure`, for a manager above. Optional.

## Result

Three journeys, nothing added to the core. Two conventions surfaced,
both package-level: a Module's entry rd such as `/module/text/open`, and
a coordinator's own result shape for partial multi-rd work.
