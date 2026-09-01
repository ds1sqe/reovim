# Syn Closure Admission

Status: conditionally admitted only for the non-shipping Reovim conformance
checker.

## Exact need and feature selection

The checker parses bounded first-party Rust files and walks imports, target
attributes, unsafe syntax, and public type syntax. The direct dependency is:

```toml
syn = { version = "=3.0.4", default-features = false, features = ["full", "parsing", "visit"] }
```

Present in Cargo metadata resolve on all three committed profiles:

- `syn 3.0.4` — MIT OR Apache-2.0;
- `proc-macro2 1.0.107` — MIT OR Apache-2.0;
- `quote 1.0.47` — MIT OR Apache-2.0;
- `unicode-ident 1.0.24` — (MIT OR Apache-2.0) AND Unicode-3.0.

## Safety, build, and runtime review

The metadata-resolved closure declares no `links` and contains no packaged native source.
`proc-macro2` declares a build script. The reviewed `syn`, `proc-macro2`, and
`unicode-ident` crate sources contain unsafe-related Rust source; the reviewed
`quote` source had no `unsafe` token. This is a source observation, not proof
that a particular target compiles every occurrence. The surface is contained
in RepositoryTooling; all first-party target roots forbid unsafe code and the
checker rejects first-party unsafe syntax.

Declared MSRVs are no higher than Rust 1.71, below Reovim's Rust 1.92 floor.
The selected versions were non-yanked and upstream repositories active at the
review date. Syn 3 is a recent major line and publication ownership is
concentrated, so updates require compatibility, ownership, and maintenance
review.

`cargo-audit 0.22.2` found no matched advisory or warning on 2026-09-01 using
RustSec database revision
`72f8b23d78ea6c4c9ded301a4c6ec4260e8b4c27`.

## Proof boundary and decision

Syn is syntax evidence only. It does not expand or resolve `cfg`, declarative
or procedural macros, includes, generated output, aliases, glob re-exports, or
types. Phase-B v1 therefore rejects every Product owner and every shipping
root with `PRODUCT_BOUNDARY_PROOF_UNAVAILABLE`; the parser's boundary checks
are capability tests, not claimed Product conformance. Its
`syntax_item_expr_type_visits` limit counts selected post-parse Item/Expr/Type
visitor callbacks, not every Syn node; bounded source bytes do not by themselves
prove a hard parser-time peak-memory bound.

The same exploratory JSON + Syn probe described in the sibling record is
nonconforming and unretained. It selects no performance budget.

Decision: admit this exact closure conditionally for bounded tooling syntax
checks. A future Product-boundary claim requires bounded compiler/package
evidence and a renewed dependency/performance review.

Primary references: [`syn 3.0.4`](https://docs.rs/syn/3.0.4/syn/),
[Cargo external tools](https://doc.rust-lang.org/cargo/reference/external-tools.html),
and [RustSec review revision](https://github.com/RustSec/advisory-db/commit/72f8b23d78ea6c4c9ded301a4c6ec4260e8b4c27).
