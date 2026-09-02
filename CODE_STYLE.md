# Reovim Coding Style

Status: repository-wide contributor rule.

## Authority

Reovim adopts the maintainability intent of the
[Linux kernel v4.10 coding style][linux-style]. Language-native formatters,
the Rust type system, Reovim's safety rules, and normative product contracts
still govern where C-specific mechanics do not apply.

[linux-style]: https://www.kernel.org/doc/html/v4.10/process/coding-style.html

## Hard readability limits

- Active Rust, shell, TOML, and YAML source lines are at most 80 columns. The
  governed suffixes are `.rs`, `.sh`, `.bash`, `.zsh`, `.toml`, `.yml`, and
  `.yaml`; an extensionless file needs a separately admitted consumer first.
- Rust block nesting is at most three levels. Extract a named helper or redesign
  the control flow before adding a fourth level.
- Clippy's block-counting model is the executable nesting definition. Inline
  modules, `impl` bodies, function bodies, and ordinary blocks participate in
  that count; the configured threshold is three and the workspace lint is
  `forbid`, so a local attribute cannot weaken it.
- The repository-root `target/`, `tmp/`, lockfiles, heritage, and retained
  history are outside the active source-width gate. A checked-in generated file
  needs an exact, provenance-reviewed path admission; generated-looking names
  and directories receive no exemption.
- One invocation traverses at most 16,384 non-excluded repository entries and
  accepts at most 8,192 governed files, 4 MiB per file, and 64 MiB in
  aggregate. A governed path must be one regular non-symlink file. Limit
  exhaustion and input uncertainty fail with stable `STYLE_*` diagnostics.
- In shell, TOML, or YAML, an exact unbreakable token or a string whose bytes
  must remain searchable may exceed 80 columns only when the immediately
  preceding line is its one-use marker:

  ```text
  # reovim-style: allow-long-line searchable-string
  ```

  The closed reasons are `exact-token` and `searchable-string`. Unknown, stale,
  duplicate, and nonadjacent markers fail the gate. Rust has no line-width
  marker: use a named constant, helper, or adjacent literals through `concat!`.
- Alignment is not logical nesting. Continuation alignment may exceed three
  indentation widths while the enclosing control structure remains within the
  three-level limit.
- Tabs advance to eight-column stops. Printable ASCII occupies one column;
  every non-ASCII scalar is conservatively charged two columns so full-width
  source cannot bypass the limit.
- Active Markdown prose is wrapped to 80 columns where practical. Tables and
  exact link destinations are documentation-layout exceptions. Heritage and
  retained historical evidence are not reformatted as current source.

## Structure and naming

- A function does one thing. Its size decreases as its control flow and state
  become more complex.
- Prefer early typed exits and small named helpers over deep nesting.
- Keep state and names in the smallest useful scope. Public and cross-module
  names are descriptive; short local names are acceptable only when clear.
- Avoid tricky expressions, hidden control flow, multiple operations presented
  as one statement, and type information encoded in names.
- Split files by one owned subject before navigation or review becomes hard.
  A split must preserve one authority rather than duplicate it.

## Comments and diagnostics

- Make the code show how it works. Comments record purpose, invariants,
  non-obvious constraints, safety, or why a rejected alternative is unsafe.
- Public Rust APIs use Rust documentation where their contract is load-bearing.
- Diagnostics are concise, correctly spelled, unambiguous, and tied to the
  responsible mechanism.
- Do not add editor modelines or repository-local editor overrides to source
  files.

## Rust mapping

- `rustfmt` owns lexical Rust layout with an 80-column width and four-space
  indentation. Do not replace it with C tabs or C brace placement.
- Prefer explicit domain types and enums to aliases that hide meaning.
- Use `Result` and typed errors for fallible actions; predicates return `bool`
  only when two states are the complete contract.
- Use ownership and RAII for cleanup. A guard is disarmed only after the owned
  effect is reconciled.
- Prefer functions and standard/project helpers to function-like macros.
- Let the compiler decide ordinary inlining. Forced inlining needs measured,
  retained evidence.
- Keep conditional compilation at module or whole-item boundaries where
  practical, with typed stubs or unavailable outcomes for unsupported targets.

## Verification

`./scripts/check.sh` owns the executable style gate. It checks Rust formatting,
the three-level Rust nesting limit, line width for governed human-authored
files, trailing whitespace, and the repository's existing strict lints. The
gate retains failing four-level nesting and unformatted-Rust mutants alongside
its scanner, bound, marker, encoding, and path-scope mutants.
