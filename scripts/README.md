# scripts

The contributor checks, and the end-to-end runner.

- `check.sh`: the local gate. It selects the components that changed
  and their dependents, reusing toolchain-bound results through
  `check-cache.sh`; `--full` runs everything.
- `ci.sh`: the same matrix GitHub CI runs, on the toolchains pinned in
  `ci.json`. It installs what it needs, so it wants the network.
- `check-style.sh`: line width and trailing whitespace over the
  governed Rust, shell, TOML and YAML sources.
- `check-documentation.sh`: local links and readability bounds for the
  active documentation.
- `e2e.py`: server and CLI scenarios against already-built binaries;
  the scenarios themselves are in `e2e_cases`.
