# Entrypoint Review: `scripts/check-style.sh`

## Scope

- Kind: script
- Exact content digest:
  `sha256:f4db854aa98c3249b52ac870e01841c8cb4776006806d08d3dbb90acb9c57d5d`
- Review mechanism: exact content and retained review

## Direct local invocations

None. The script reads governed repository source and executes only hosted
command-line tools, including the selected Rustfmt and Clippy driver.

## Reviewed behavior

- The active set is regular non-symlink source with one of the exact Rust,
  shell, TOML, or YAML suffixes below the resolved repository root.
- Version-control metadata, the repository-root build output, working notes,
  lockfiles, heritage, and retained history are excluded by a closed path
  policy. Generated-looking names are not exempt. Scope mutants exercise each
  exclusion and reject the unused extensionless-script profile.
- Traversal stops at 16,384 non-excluded entries. Governed input stops at
  8,192 files, 4 MiB per file, or 64 MiB aggregate. Nonregular paths, stat/read
  uncertainty, concurrent size change, invalid UTF-8, and every limit have
  stable `STYLE_*` failures and retained mutants.
- Core Perl UTF-8 decoding makes scalar width independent of the hosted `awk`
  implementation. Tabs advance to eight-column stops, printable ASCII is
  charged once, and every non-ASCII scalar is charged twice. Accent and CJK
  mutants bind that rule. Trailing whitespace always fails.
- Only shell, TOML, and YAML admit `exact-token` and `searchable-string`
  markers. A marker is local to the immediately following overlong line and
  consumed once. Rust markers are forbidden. Embedded mutants reject Rust,
  unknown, stale, duplicate, nonadjacent, wrong-comment, reused, and overlong
  markers.
- A four-level Rust specimen must fail through Clippy's excessive-nesting lint
  with `CLIPPY_CONF_DIR` bound to the repository. An unformatted Rust specimen
  must fail through Rustfmt using the repository configuration.

## Assumptions

- `/usr/bin/env` resolves a Bash implementation supporting the constructs used
  by these exact bytes.
- `BASH_SOURCE` resolves this checked-out script so `ROOT` selects the reviewed
  repository.
- The hosted GNU/Linux Perl core provides `utf8::decode`; `find`, `stat`,
  `mktemp`, `mkdir`, `ln`, and `rm` provide the exact reviewed forms.
- The selected toolchain supplies the `clippy-driver` and `rustfmt`
  executables resolved by the script. Rustfmt supports `--check` and
  `--config-path`; Clippy supports the configured excessive-nesting lint.

## Decision

Admit these exact bytes with an empty direct-local-invocation set for the
repository source-style gate. Any byte, governed-source boundary, marker
reason, tool assumption, or invocation edge change reopens this review.
