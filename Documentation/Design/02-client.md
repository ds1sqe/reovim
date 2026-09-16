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

## Wire

Client as user: `lookup`, `info`, `parent`, `children`, `open`, `read`,
`write`, `call`, `watch`, `since`, `unwatch`, `close`, `ask`, `cancel`.

Client as Driver: the seven calls up, the seven slots down.

Every call carries the per-connection sequence from Reply. On reconnect
the Client presents the credential, repeats unanswered sequences, and
resumes each watch with `since(cursor)`.
