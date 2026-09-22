# 00 — Core Concept

## Rule

- All the Kernel/Server/Client is Fixed.
- Concrete Domain only live at extensional point.
- A **Module** lives in the server's memory and is loaded by the Kernel.
  A **Plugin** lives at the Client. The two words are never exchanged.
- The Kernel has no domain word: not text, file, buffer, line, cursor.

## Resource

Everything is a Resource.

- Filesystem's file
- NetworkConnection
- Kernel's system variable
- Client's state, io
  - Display
  - Input

### ResourceDescriptor(rd)

> A it self only have a ID.
> To get more info, should ask at the kernel

- Name (optional; UTF-8; no NUL, no `/`). A path never enters the
  Kernel; an unnamed rd is reached by id only.
- Flags ( Read / Write / Container )
  Container: can add child
- Time-info ({c/m/a} time). Stamped by the Kernel on its own clock
  when the Driver reports: `read` → a, Modify → m, Attrib → c.
- Parent/Child
- Offered apis (declaration hashes)

The Kernel owns the ID. Everything else is reported by the Driver and
cached by the Kernel. A stale cache is normal; `watch` says it moved.
`info(rd)` and `parent(rd) -> rd` read that cache and never reach a
Driver; `children(rd) -> [rd]` asks the Container's Driver to list
once and answers from the cache from then on. All three take an rd,
not a handle: metadata needs no right, like stat by inode.

## Module

A Module is a unit of loading. One artifact, one `start`/`stop`, one
copy in memory, one retention count. It contains zero or more Drivers.
It is not typed by content: "File module", "Vim module" are names.

A Module depends on declarations, never on another Module.
`load <path>` and `unload <name>` are connection-level commands beside
connect and session selection, not tree ops.

## Driver

A Driver is a unit of serving. It lives inside a Module. The Kernel maps
every rd to exactly one Driver; any number of Drivers may open that rd.

- Owns hidden state behind its rds. The Kernel never inspects it.
- Answers the fixed ops through one C vtable. The vtable never grows.
- Has its own ops only through `call`. Like ioctl, but typed by a
  declaration. Anything not covered by the fixed ops is a `call`.
- May register new rds and may hold handles on other Drivers' rds.

```text
Module (one copy in memory)
 ├─ Driver "file"      → rd 7, rd 12, rd 31
 └─ Driver "directory" → rd 3, rd 8
```

## Fixed ops

```c
struct rv_driver {
    uint32_t abi;
    rv_reply (*lookup)(ctx, rd, name, rd *out);      /* lazy child   */
    rv_reply (*open)  (ctx, rd, rights, handle *out);
    rv_reply (*read)  (ctx, handle, at, buf, len *);
    rv_reply (*write) (ctx, handle, at, buf, len);
    rv_reply (*call)  (ctx, handle, api, method, in, in_len, out *);
    rv_reply (*watch) (ctx, rd, on);                 /* hint only   */
    rv_reply (*close) (ctx, handle);
};
```

Seven. A Driver may leave some empty. Seven calls face the Kernel from
a Module, and they never grow either:

```text
register(driver, parent, name?) -> rd   create a Resource
name(rd, name?)                     set or clear its name; parent gets Move
report(rd, flags, apis)             change its shape; watchers get Attrib
unregister(rd)                      remove it and its subtree; Gone
changed(rd, Modify | Attrib)        after an accepted change
done(token, outcome)                finish a Pending reply
open(rd, rights) -> handle          use any Resource, own or not
```

Lower and upper are positions, not kinds; a Driver does both.

```text
   upper: Vim, Markdown editor, Client presentation
        │  open(rd) → read / write / call / watch / close
   ─────┼─────────────────────────────────────────────
        │  Kernel: rd tree · flags · access · dispatch
   ─────┼─────────────────────────────────────────────
        │  register(driver, parent, name?) → rd
   lower: file, memory, socket, variable, Display, Input
```

- Children are lazy: resolving a name asks the Container's Driver
  `lookup`, which registers the child then. An empty name means
  "register all children now"; the Kernel asks a Driver-owned
  Container once before the first `children` on it. A Module's home
  is the Kernel's own row and is never asked to list: it holds what
  the Module registered under it and nothing else.
- A row flagged `CONTAINER` holds only the names it registered, so a
  name it has no child of, and whose Driver answers `Unsupported` for
  (no `lookup` of its own), is `Invalid` — on any Container a Driver
  owns, and on a Module's home the same way, a missing name there
  going to the Module's first Driver (`DriverId(0)`, a home carrying
  no `DriverId` of its own). A row that is no Container keeps
  `Unsupported`, and a Driver with a `lookup` of its own still
  answers for itself.
- `read` fills `len`; 0 with `Ready(Ok)` is EOF. A stream Driver
  ignores `at`. Where a Driver has frames, one `read` or `write` is one
  frame; per-frame metadata is a package api, not a Kernel op.
- A Module cannot unload while any handle on any of its rds is open.
  The Kernel refuses; it never revokes.
- Relations between rds of different Drivers are not Kernel state. A
  Driver that wants one visible publishes it as a child rd.

## Tree

```text
/                 id 0   Kernel   Container
  sys/            id 1   Kernel   modules/<name>, decls/<hash>, var
  module/<name>/         Kernel   only Module <name> registers below
  session/<id>/          Kernel   common; made when a Client connects
    client/<cid>/        Kernel   capability, heard, self
    <name>/…             Module   per-session state, owner-only below
  var/                   Kernel   common, global
```

Ids 0 and 1 are the only well-known ids; the rest is `lookup` from 0.
`register` is allowed when the parent is served by the caller's Module,
or is a Kernel Container flagged Write: `/session/<id>` and `/var`.
`name` and `unregister` follow the child's Module.

A Session is a shared editing space. A Client connects naming one or
gets a new one; two Clients in one Session share it. A Module is loaded
once; its per-session state is what it registers under `/session/<id>`.
Unregistering a Container unregisters its subtree, deepest first; a
deleted Session takes its Module state with it. An rd never changes
parent: moving is unregister, then register with a new id. Relations
across roots are pointer rds, never registrations in another owner's
tree. A pointer rd is a Read rd whose content is the target rd id.

A Client's own rds stand under `client/<cid>/self`, mirrored row by
row as somebody asks for them: the metadata is the mirror's and costs
no message, while every op that acts is forwarded and the Client's
answer is the truth. The Client announces its side with `changed`,
which carries four kinds over the wire where a Module's carries two:
`Modify` and `Attrib` are events; `Gone` retires the mirrored row
with its subtree; `Create` unmarks that row's listing, so the next
`children` asks the Client again.

## Declaration

A declaration is data, not code: an api id, a version, per method a
number, a name, an input type, an output type from a closed tag set, and
the right it needs, and the records the api names, each with its fields
in the order declared. Its canonical bytes are identical from any
language; their hash is its identity.

```text
api "history"  id=…  ver=1
  0  tree  ()        -> Tree{nodes:[{id,parent,label}],head,published}  READ
  1  goto  (rev:u64) -> ()                                             WRITE
  2  undo  ()        -> ()                                             WRITE
  3  redo  ()        -> ()                                             WRITE
```

- A Driver brings the bytes with `report(rd, flags, apis)`, after
  `register` or any time later. The Kernel checks bounds, stores each once,
  records the hashes on the rd and serves the bytes at `/sys/decls/<hash>`.
  The Kernel publishes no declaration.
- A caller calls by hash and method number. The Kernel checks the rd
  offers that hash, the method exists, the handle holds its right and
  the input fits, then calls the vtable. No match returns Unsupported
  before the Driver runs.
- The records are hashed with the methods, so a reshaped record is a new
  identity even where no method's type names it: an api of records and
  no method at all is its records.
- Methods never change at runtime. New bytes are a new hash, and an rd
  may offer old and new. What changes at runtime is which rds exist and
  which apis an rd offers; watchers see Create, Delete, Attrib.
- Modules never link each other. A Vim Module and a Mesh Driver share
  only the `history` bytes; either may be replaced without recompile.
- Types: Unit, Bool, U8, U32, U64, I64, F64, Bytes(max), Utf8(max),
  Record, Sequence(max), Array(n), Choice, Rd. Rd is a u64 rd id; the
  callee opens it under its own rights. Bounds: 64 KiB per declaration,
  256 methods, depth 16.
- Language helpers derive from the bytes: Rust `declare!` gives a trait
  and a typed wrapper, C a header, Python builds methods at import.

## Rights

Two rights, READ and WRITE, on handles. `open(rd, rights)` asks; the rd's
flags cap. There is no Execute: `call` is the only way to run Driver
code with arguments, so the method's declared right is all there is.

```text
read, watch, lookup       READ
write                     WRITE
call                      the right the declaration names for the method
register a child          WRITE on the Container, by its owner
```
