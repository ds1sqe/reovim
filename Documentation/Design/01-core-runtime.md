# 01 — Core: runtime

What happens over time on the tree that [00-core.md](00-core.md)
defines. Same Kernel, same rules.

## Buffer

There is no Buffer type. A Buffer is versioned state: a tree of
restorable states, a head, and an optional published pointer.

```text
r0 ─ r1 ─ r2 ─ r3 ─ r4
      ▲              ▲
   published        head     draft = state at head
                             not-applied = head ≠ published
```

- The draft is the state at head. Modified is head ≠ published. A
  scratch Buffer has no published pointer and no source.
- `history` is an optional api a Driver offers. Revision is a `history`
  concept, not an rd field. A call may carry the revision it expects;
  a stale one is rejected.
- Every accepted edit is a child of head; an edit after undo branches.
  States are stored, not edit commands, so any api may produce a node.
- A batch of edits in one `call` is one node. Across rds there is no
  transaction; a Module above coordinates with per-rd preconditions.
- Each content domain is its own Driver with state in its natural form.
  Rope, compression, spill to disk are hidden. The tree and `goto` step
  are SDK library code a Driver may embed.
- Source is a handle the Driver keeps on another rd, shown as a Read
  child `source` if it wants. Several Drivers may open one source; each
  has its own tree, and the source rd's `watch` is their only meeting.
- `watch` on the rd is the only notification. There is no Buffer event.

## Reply

Every op answers in the same call: `Ready(Ok)`, `Ready(Err)` or
`Pending(token)`. Accept and reject are the Driver's, including any
precondition. The Kernel checks rights and declaration, then forwards;
it never queues or serializes calls.

The Kernel holds one table, token to outcome, with three operations:
Driver `done(token, outcome)`, caller `ask(token)`, caller `cancel(token)`
forwarded to the Driver. The table survives a caller's disconnect, so a
reconnected caller asks and gets the answer. No `done` ever is Unknown,
and Unknown never authorizes a retry.

`Err` carries one of nine causes, closed: Denied, Unsupported, Invalid,
Stale, Busy, NoSpace, Closed, Gone, Cancelled.

Every call carries a per-connection sequence. The Kernel keeps replies
for a bounded window of sequences and answers a repeat from memory, so
a lost reply is asked again, never re-executed.

Five edits on five rds are five calls and five tokens; each goes straight
to its Driver, and parallelism is the Driver's choice.

## Execution

The Kernel calls the vtable from its own worker threads. A vtable call
may block. A Driver may make the seven Kernel calls from any thread,
including inside a vtable call, except `register` or `unregister` of
the rd being served by that call. `done` may come from any thread.

## Watch

An event is `(seq, rd, kind, child rd?)`. Seven kinds, closed:

```text
Modify            content changed                Driver reports
Attrib            flags, times, apis             Driver reports
Create   child    child registered under rd      Kernel emits
Delete   child    child removed under rd         Kernel emits
Move     child    child's name changed under rd  Kernel emits
Gone              rd removed; handles dead       Kernel emits
Overflow          log trimmed past the cursor    Kernel emits
```

No name, diff or revision. A watcher reads the rd, or `history.tree()`.

User side, Client or upper Module, on a handle:

```text
watch(h)             start receiving for h's rd
since(cursor)        events after cursor, in order; advance cursor
unwatch(h)           stop; close(h) implies it
```

One cursor per watcher. A Client keeps it across reconnect and resumes
with `since`; if the log no longer holds that position it gets
`Overflow` and re-reads what it watches. A coherent first view is: take
the current cursor, read, then `since(cursor)`.

Inner side: the Driver calls `changed(rd, Modify | Attrib)` after an
accepted change, and may use the `watch(rd, on)` hint to start or stop
an external source. The Kernel emits the tree kinds from `register`,
`unregister` and `name`, appends every event once to one bounded,
numbered log, pokes live watchers of that rd, and answers `since` from
the log.

## Log

`/sys/log` is one bounded ring the Kernel serves, Read and Write. An
entry is `(seq, time, mark, level, bytes)`. A Module opens it and
writes; the Kernel stamps `mark` from the caller, the Module name or
`kernel`, so it cannot be forged. Read at a `seq` returns entries from
there; the oldest are trimmed; `watch` gives Modify. The Kernel stamps
every `call` at dispatch and at reply and logs the pair with rd, api and
method under `kernel`, so the Kernel-to-Module round trip is measured
with no extra surface.

The declaration hash is a fast 64-bit non-cryptographic hash, so
dispatch is one table probe. The Kernel keeps the bytes and refuses a
collision at `register`.

## Pressure

A Module owns all Driver memory; the Kernel owns only bounded metadata:
rd table, declaration bytes, Pending table, watch log, log ring. So the
Kernel holds no size and no quota. A Driver enforces its own limit and
answers `Err(Busy)` or `Err(NoSpace)`. Pending has a fixed cap per
handle; past it the Kernel answers `Err(Busy)`. A slow watcher gets
`Overflow`. A Driver that wants to be measured or asked to shrink offers
it as a Read rd or an optional api, the same way as `history`.
