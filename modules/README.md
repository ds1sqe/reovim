# modules

Modules the Kernel loads. Each Module holds one or more Drivers.

- `coreutils`: the Module that provides the Drivers under `/bin`:
  `stat`, `ls`, `tree`, `cat`, and `grep`.
- `fs`: the fs Driver: a directory on disk mirrored under one rd,
  as Containers and byte rds.
- `hello`: a C example Driver, showing the module ABI from outside
  the workspace.
- `sysutils`: the Module that provides the Drivers under `/bin` that
  face the Kernel itself: `log`, `dmesg`, `call`, `watch`, `date`,
  `uptime`, `capture`, and `keys`.
- `text`: the text Driver: a Buffer over any rd, with edit history
  and a `publish.save` back to the source.
