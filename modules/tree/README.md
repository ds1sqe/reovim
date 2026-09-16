# Resource tree

`tree [PATH]` lists the hierarchy without invoking provider reads or writes.
Omitted PATH selects CWD; `/` selects the granted root. The first line identifies
the selected path so the location stays clear:

```text
/env
├── CWD
│   └── info
└── MY_OPTION
```

Nodes are Resources: a callable Resource can also have children. The command
does not classify them as filesystem files/directories or guess an execute flag.
It uses public CONTAINER/EMPTY inspection to avoid listing leaf/empty subjects.
Those flags describe the caller-visible children, independently of data size;
older access adapters without inspection retain the ordinary listing path.
Control characters in names are escaped. Unavailable children are reported.
Traversal is sampled, bounded to 256 nodes, depth 16 and 16 KiB of output;
an explicit marker identifies truncation. Cycles are reported without recursion.

The native declaration is `tree` schema 1; invocation is `command.tree` v1.
An ordinary `info` child advertises the optional argument and byte output.
Load with explicit import grants or pass a source as a borrowed reference:
`--refs SOURCE /tree @0`. The same command can be mounted at any path and
requires no CLI/Kernel changes.
