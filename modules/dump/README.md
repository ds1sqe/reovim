# Finite Resource dump

The native `dump` module reads a granted subtree using `resource.bytes` v1 and
writes captured bytes through a separately granted Host directory Resource.
It performs no source writes and changes no Buffer backing or saved status.

```text
--refs SOURCE,DESTINATION /dump NAME
```

`command.dump` v1 has two borrowed input references: READ on the source and
WRITE on the destination. Input is a UTF-8 export name, one path component.
The ordinary info child advertises this default invocation. No imports or
module-specific CLI branch are needed. NAME must not already exist.

Each Resource gets its own output directory. Its finite bytes become
`NAME/value.bin`; child `foo` becomes `NAME/children/foo/value.bin`, recursively.
This represents a Resource that has both a value and children without filename
collisions. Native names are preserved losslessly or the operation reports an
error. Directory traversal and symlink following are rejected by the Host.

Only `resource.bytes` v1 is selected: no guessed domain serialization, command
writes, subscription creation, or reference serialization. Unsupported reads
are reported, while supported descendants can still export. Discovery/capture
is bounded to 64 nodes, depth 16, 16 KiB per value and 8 KiB of combined paths;
exhaustion rejects before creating the export directory.

After each write, dump re-reads that source value and children. A difference
reports `changed`; this is sampled validation, not a transactional snapshot of
the entire tree. Bytes already written remain the captured value. Unsupported,
unavailable, changed, incomplete and unknown outcomes remain visible per node.
The Message result starts with `complete` or `partial`, followed by TSV rows:
status, percent-escaped relative path, detail. Paths escape percent/control
bytes; an empty path identifies the source root. Unknown request evidence is
preserved in the result detail and the underlying retained work remains owned.

## Host output contracts

The destination supports writes with no reference slots:

- `host.export.directory` v1: one UTF-8 component; creates without overwrite.
- `host.export.file` v1: relative UTF-8 path, NUL, complete file bytes. Creates
  missing intermediate directories and a new file; never replaces a file.

Replies contain status byte (0 complete, 1 incomplete, 2 unknown durability),
little-endian u64 bytes written and little-endian u64 native error code.
Directory success uses zero bytes/error. A failed file operation can leave
created directories or partial bytes; sync failure reports unknown durability.
External effect status belongs to this Message, separately from delivery
recovery. Requests execute on bounded Host workers, outside Runtime execution.

The Linux reference server accepts an explicit `--export-root DIRECTORY` grant.
Each retained client gets a private output directory exposed at `host/files`.
Without that option, no filesystem output Resource is installed. Kernel and
portable module code contain no filesystem handles or I/O.
