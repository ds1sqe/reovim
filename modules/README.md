# modules

Modules the Kernel loads. Each Module holds one or more Drivers.

- `clipboard`: the clipboard Driver: the system clipboard as one
  rd, read and written through the host's tool.
- `coreutils`: the Module that provides the Drivers under `/bin`:
  `stat`, `ls`, `tree`, and `cat`.
- `fs`: the fs Driver: a directory on disk mirrored under one rd,
  as Containers and byte rds.
- `hello`: a C example Driver, showing the module ABI from outside
  the workspace.
- `text`: the text Driver: a Buffer over any rd, with edit history
  and a `publish.save` back to the source.
