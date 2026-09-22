# 02 — Client

A Client is a Driver on the far side of the wire. It uses the tree with
the same ops a Module uses, and it may serve rds with the same seven
slots. One protocol, both directions.

## Connect

`connect(credential, session?)`. The Kernel attaches to the named
Session or makes one, makes `/session/<id>/client/<cid>`, and answers
`(session id, cid)`. Ids 0 and 1 are known; everything else is `lookup`.

`load <path>` and `unload <name>` are commands on this layer.

## Capability

The Client registers what it wants to expose under its own Container:

```text
/session/<id>/client/<cid>/
  capability     Read rd; bytes the Client's Plugins understand
  display        optional; present if a Module may push frames
```

The Kernel stores and serves these bytes. It never parses them.

## Input

Raw keys never cross the wire. A Plugin turns keys into `call`s. A
Module sees calls, never keys. Plugin state, rendering and input
resolution live in the Client; the capability a Plugin reads is its own
Client's, read locally.

## Resolver

A resolver turns input into editing decisions: mode, counts, pending
operators, bindings and continuation state are its own. The Client
hosts it, and five declared records (the `resolver` api in
clients/core/api) are what crosses between them. None of them is the
text.

- It takes an `Input`: one `Press`, the `origin` saying whose key it
  is — the user's, a macro's, a `.`'s — and `released` saying whether
  a failed prefix let that key go.
- It hands back `Fed`, which is keys and not an outcome: a macro's
  run, or the keys `.` answered. The host pushes them onto its own
  queue as `Input`s, so a resolver feeds itself nothing.
- It answers its host's outcomes, of which one is declared: a
  `Command`, a line for the shell to run whole.
- It receives a `Reply`: the clipboard's text for a put that waits on
  it, or the operation that was refused and the `error` that stopped
  it, under the continuation the wait was made with.
- It exposes a `State`: the `mode`, the `showcmd` a count and a
  pending operator spell (`3d`), and `waiting`, the continuation
  still unanswered. A frame reads it once.

A `Command`'s line is `NAME [--bang] [--address ADDR] REST`. Vim's
own grammar is read off the front of a `:` line and put back on as
flags, so `:5,10s/a/b/` is `s --address 5,10 /a/b/`, and a line whose
name is none of Vim's is itself: `:ls /module | cat`. The host runs
it whole through the shell and holds no parser. Names resolve on
PATH, where Vim's Ex names and every abbreviation nvim takes stand
first as rds under `/self/vim/bin`, each offering the declaration its
own shape needs; a name that reads an address takes it as text and
resolves it through the engine's one resolver, so a range is read in
one place and the shell learns no line number.

A mode is the Client's. Normal, Insert, Command and the visual ones
are numbers in `State.mode`, which the api's own `mode` module names,
read by the Client that draws the frame and by `/self/vim/mode`,
which spells the name back. `:` is the change to Command mode and
nothing more; the line typed after it is composed there. No mode
reaches the Kernel, a Module or a Driver.

The host performs the outcomes and owns what they reach: handles, the
input queue, the replay budget, the shell, and the tree a name
resolves over. It reads no field of the resolver. The search view is
its own too: one view per stay, opened over the draft at the first
search and sought at every key after through the search Module's
`seek`, so a growing line reads one view's `matches` and no key opens
a second.

What a resolver decides beyond a line for the shell is its own, in
its own types: Vim's edits, opens, jumps and searches are enums of
plugins/vim's that its host matches, and nothing is declared here for
a consumer that does not exist. A second resolver brings its own
declaration with it. The boundary is the crate split — the engine one
crate, its host another, `Vim` with no `pub` field, the readings it
offers the only doors — and a `compile_fail` doc test in the host
keeps it.

A `Press` is never invented. A key no person typed — a macro's, a
repeat's, one a prefix let go — carries the press of the key that made
it, so a recorded stream replays as it was and no clock and no source
is made up.

One numbering answers every wait, and it only grows. An outcome that
waits carries a continuation, a search's own epoch drawn from the same
counter, the reply carries it back, and a number that is not the one
waited for is stale and means nothing. The host echoes the number and
never reads what it stands for.

The keymap is the resolver's data: keys as a person types them and the
line they run. Which binding a prefix may still become, and letting
those keys go again when none does, is the resolver's; the host runs
what it is handed back. Nothing of a keymap crosses the wire, and a
Plugin that needs no modes is given no mode system.

## Wire

Client as user: `lookup`, `info`, `parent`, `children`, `open`, `read`,
`write`, `call`, `watch`, `since`, `unwatch`, `close`, `ask`, `cancel`.

Client as Driver: the seven slots down; up, only `done` and
`changed`. A Client's own Modules register under its `self`, and the
Kernel mirrors those rows as somebody asks for them, so no
`register`, `name`, `report` or `unregister` crosses. The mirror is
for everybody else: a Client that meets its own `client/<cid>/self`,
by that path or as a child of its own client rd, answers on its own
root and sends nothing, an op forwarded back to its sender being one
that waits on the thread that sent it. `changed`
carries four kinds where a Module's carries two: `Modify` and
`Attrib` are events; `Gone` retires the mirrored row with its
subtree; `Create` unmarks its listing, so the next `children` asks
the Client again.

Every call carries the per-connection sequence from Reply. On reconnect
the Client presents the credential, repeats unanswered sequences, and
resumes each watch with `since(cursor)`.
