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
hosts it, and four declared records (the `resolver` api in
clients/core/api) are the whole exchange.

- It takes an `Input`: one `Press`, the origin saying whose key it is,
  and whether a failed prefix let that key go.
- It answers an `Outcome`: a kind, and the record that kind carries.
- It receives a `Reply`: what the host found out, carrying the
  continuation the outcome waited on.
- It exposes a `State`: the mode, the status word, the command a key
  has begun and the continuation still unanswered. A frame reads it
  once.

The host performs the outcomes and owns what they reach: handles, the
input queue, the replay budget, and the tree a name resolves over. It
reads no field of the resolver. `State` and the readings the resolver
offers are the whole of what it may read, and another resolver in the
same place needs none of the first one's internals.

A `Press` is never invented. A key no person typed — a macro's, a
repeat's, one a prefix let go — carries the press of the key that made
it, so a recorded stream replays as it was and no clock and no source
is made up.

One numbering answers every wait. An outcome that waits carries a
continuation, the reply carries it back, and a number that is not the
one waited for is stale and means nothing. The host echoes the number
and never reads what it stands for.

The keymap is the resolver's data: keys as a person types them and the
line they run. Which binding a prefix may still become, and letting
those keys go again when none does, is the resolver's; the host runs
what it is handed back. Nothing of a keymap crosses the wire, and a
Plugin that needs no modes is given no mode system.

## Wire

Client as user: `lookup`, `info`, `parent`, `children`, `open`, `read`,
`write`, `call`, `watch`, `since`, `unwatch`, `close`, `ask`, `cancel`.

Client as Driver: the seven calls up, the seven slots down.

Every call carries the per-connection sequence from Reply. On reconnect
the Client presents the credential, repeats unanswered sequences, and
resumes each watch with `since(cursor)`.
