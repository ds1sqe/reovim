//! Dependency-graph probe engine for the reovim workspace.
//!
//! The engine enumerates workspace crates, classifies each into the
//! source categories of `Documentation/01-Architecture/02-Project-Layout-and-DAG.md`
//! §1, and verifies every manifest edge against the allowed-edge tables:
//! §2 category edges, §6 foundation sub-DAG, §7 composition catalog, §8
//! transitional allowlist. The probes under `tests/` run as part of plain
//! `cargo test` and fail closed (DAG1..DAG3, DAG5, DAG6).
//!
//! **DAG4** is spec-asserted: the probe reads manifests only, so runtime
//! install layouts (`$ROOT/{module,driver,capability}/...`) can never grant
//! a Cargo dependency edge. The probe does not reach outside `Cargo.toml`
//! files, so install-path inputs simply do not exist in the probe's data
//! model.
//!
//! **DAG6** (zero-std): `run_dag6_probe` walks every product crate for
//! `#![no_std]` at the crate root and `std`/`alloc` usage in source files;
//! `check_panic_profiles` verifies the workspace `[profile.dev]` and
//! `[profile.release]` both set `panic = "abort"`.  Bootstrap-state
//! exclusions are enumerated as an explicit const, not pattern-matched.

pub mod toml;

use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Path, PathBuf},
};

// ── DAG6 bootstrap exclusions ────────────────────────────────────────────────
//
// Crates listed here are exempt from the zero-std walk (DAG6 §5 step 6).
// These are the two tracked bootstrap states from spec 1.2 §10.  The list is
// an explicit const — no pattern matching — so additions require a spec edit.
//
// Bootstrap state 1 (libtest links std in test builds) is handled by skipping
// `tests/` directories and `#[cfg(test)]` blocks during the source sweep; the
// crate-root `#![no_std]` check still applies to state-1 crates EXCEPT those
// in this exclusion list.
//
// Bootstrap state 2 (ground tooling uses std): `lib/depgraph` and `scripts/`
// are excluded here.  `scripts/` is not a Cargo crate so only `lib/depgraph`
// appears.
const DAG6_BOOTSTRAP_EXCLUSIONS: &[&str] = &[
    "lib/depgraph", // bootstrap-state-2: std ground tooling (spec 1.2 §10)
];

/// Source categories per spec 1.2 §1. `archive/*` is excluded from
/// enumeration rather than classified.
///
/// ```rust
/// use reovim_depgraph::{Category, classify, default_category_table};
///
/// let table = default_category_table();
/// let cat = classify("arch", &table).expect("arch is Foundation");
/// assert_eq!(cat, Category::Foundation);
/// assert_eq!(format!("{cat}"), "foundation");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    /// `arch/`, `lib/*`, `uapi/*`
    Foundation,
    /// `editor/lib/subsys/*`
    ServerContracts,
    /// `editor/lib/core/*`
    EditorCore,
    /// `editor/lib/server/*` — Framed-protocol and dispatch glue.
    ServerRuntime,
    /// `client/lib/subsys/*`
    ClientContracts,
    /// `editor/{modules,drivers,providers,domains}/*`
    ServerExt,
    /// `client/{platforms,drivers,modules,capabilities}/*`
    ClientExt,
    /// `apps/*`
    Apps,
    /// `tools/*`
    Tools,
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Foundation => "foundation",
            Self::ServerContracts => "server-contracts",
            Self::EditorCore => "editor-core",
            Self::ServerRuntime => "server-runtime",
            Self::ClientContracts => "client-contracts",
            Self::ServerExt => "server-ext",
            Self::ClientExt => "client-ext",
            Self::Apps => "apps",
            Self::Tools => "tools",
        };
        f.write_str(name)
    }
}

/// The §1 category table. Pattern grammar: `/`-separated path
/// components; a trailing `*` matches one or more further
/// components; a pattern without `*` matches the exact path or any
/// path beneath it.
///
/// ```rust
/// use reovim_depgraph::{Category, default_category_table};
///
/// let table = default_category_table();
/// // The table is non-empty and contains the arch → Foundation entry.
/// assert!(!table.is_empty());
/// assert!(table.iter().any(|(p, c)| p == "arch" && *c == Category::Foundation));
/// ```
#[must_use]
pub fn default_category_table() -> Vec<(String, Category)> {
    [
        ("arch", Category::Foundation),
        // arch/sys-* = per-target raw mechanism crates carved out of reovim-arch.
        // Floor tier, same as arch. The pattern grammar's `*` only matches a
        // whole trailing component (`pattern_matches`), so each crate path is
        // listed explicitly rather than via a `sys-*` suffix glob.
        ("arch/sys-linux-x86-64", Category::Foundation),
        ("arch/sys-linux-aarch64", Category::Foundation),
        ("arch/sys-none-aarch64", Category::Foundation),
        ("arch/sys-none-x86-64", Category::Foundation),
        // platform/linux-native = the hosted provider: canonicalizes
        // NATIVE→kabi/platform and installs the vtable. Floor tier (it
        // implements the down-face contract, structurally peer to arch), so it
        // classifies Foundation; the firewall counts it a floor-family name no
        // product may import (`is_floor_crate_name`).
        ("platform/linux-native", Category::Foundation),
        // platform/stub-none = the bare-metal selftest handle scaffold: the
        // freestanding -ENOSYS/sentinel vtable the selftest/bootcore fixtures
        // install on *-unknown-none. Same floor tier as linux-native (it
        // implements the same down-face contract), so it classifies Foundation
        // and `is_floor_crate_name` counts it a floor-family name no product may
        // import. Superseded by the World system-kernel bridge.
        ("platform/stub-none", Category::Foundation),
        // arch/floor-* = per-target language-floor lang-item crates:
        // `_start`, `#[panic_handler]`, `rust_eh_personality`, the mem
        // intrinsics, the `entry!`/`reovim_arch_main` seam. Floor tier, same as
        // arch and arch-sys. The pattern grammar's `*` only matches a whole
        // trailing component, so each crate path is enumerated explicitly
        // (mirroring the `arch/sys-*` rows above).
        ("arch/floor-linux-x86-64", Category::Foundation),
        ("arch/floor-linux-aarch64", Category::Foundation),
        ("arch/floor-none-aarch64", Category::Foundation),
        ("arch/floor-none-x86-64", Category::Foundation),
        ("lib/*", Category::Foundation),
        // kabi/* = down-face contract tier; classifies as Foundation
        // because the coarse matrix expresses tier relationships; the finer
        // airlock invariants (kabi is a leaf, arch → kabi is a granted edge) are
        // enforced by the dedicated probes, not by minting a new category variant.
        ("kabi/*", Category::Foundation),
        // system/lib/kernel = the World system-kernel bridge: common boot data
        // shaping and console/FDT/splash policy between uapi-facing code above
        // and kabi/provider facts below. Foundation tier; products still must
        // not import it directly.
        ("system/lib/kernel", Category::Foundation),
        // system/* = the remaining system/ tree (the future World sovereign kernel
        // + any sibling subsystem crates); Foundation until an interior lands and a
        // real category is needed. The explicit system/lib/kernel row above takes
        // precedence for the lifted library; this glob covers everything else.
        ("system/*", Category::Foundation),
        // uapi = product-facing facade namespace. The physical leaf crates live
        // under uapi/inner/* so Cargo still exposes domain-specific Foundation
        // edges, while upper/product crates can name `reovim-uapi`.
        ("uapi", Category::Foundation),
        ("uapi/*", Category::Foundation),
        // ── Settled tree paths ───────────────────────────────────────────────
        // The editor/** and client/** rows classify the settled crate paths; the
        // old server/lib/*, ext/server/*, and ext/client/* rows are intentionally
        // absent because they match no crate after the rename.
        ("editor/lib/subsys/*", Category::ServerContracts),
        // editor/lib/core (exact) + wildcard for sub-crates.
        ("editor/lib/core", Category::EditorCore),
        ("editor/lib/core/*", Category::EditorCore),
        // editor/lib/server* and editor/lib/runtime* → ServerRuntime.
        // Wildcard suffix matches server/runtime crates one or more levels deep.
        ("editor/lib/server", Category::ServerRuntime),
        ("editor/lib/server/*", Category::ServerRuntime),
        ("editor/lib/runtime", Category::ServerRuntime),
        ("editor/lib/runtime/*", Category::ServerRuntime),
        // editor swap-set dirs → ServerExt.
        ("editor/modules/*", Category::ServerExt),
        ("editor/drivers/*", Category::ServerExt),
        ("editor/providers/*", Category::ServerExt),
        ("editor/domains/*", Category::ServerExt),
        // client paths.
        ("client/lib/subsys/*", Category::ClientContracts),
        ("client/platforms/*", Category::ClientExt),
        ("client/drivers/*", Category::ClientExt),
        ("client/modules/*", Category::ClientExt),
        ("client/capabilities/*", Category::ClientExt),
        // ─────────────────────────────────────────────────────────────────────
        ("apps/*", Category::Apps),
        ("tools/*", Category::Tools),
    ]
    .into_iter()
    .map(|(p, c)| (p.to_owned(), c))
    .collect()
}

/// The §6 foundation sub-DAG grant table.
///
/// Records which intra-Foundation dep edges are granted.  Foundation crates
/// (arch, lib/*, uapi/*, kabi/*, system/*) may only depend on other Foundation
/// crates when an explicit grant exists here; absent entry →
/// `UngrantedFoundationEdge` violation.
///
/// ```rust
/// use reovim_depgraph::default_foundation_grants;
///
/// let grants = default_foundation_grants();
/// // uapi/protocol is granted the uapi/abi dep.
/// assert!(grants.get("reovim-uapi-protocol")
///     .is_some_and(|v| v.iter().any(|g| g == "reovim-uapi-abi")));
/// ```
#[allow(clippy::too_many_lines)] // flat grant table: length tracks edge count, not control-flow
#[must_use]
pub fn default_foundation_grants() -> std::collections::BTreeMap<String, Vec<String>> {
    let mut m = std::collections::BTreeMap::new();
    // Public up-face facade. It re-exports the domain leaf crates for ergonomic
    // `reovim_uapi::{net,sched,...}` imports. There is intentionally no
    // product-facing POSIX leaf.
    m.insert(
        "reovim-uapi".to_owned(),
        vec![
            "reovim-uapi-abi".to_owned(),
            "reovim-uapi-dump".to_owned(),
            "reovim-uapi-fs".to_owned(),
            "reovim-uapi-log".to_owned(),
            "reovim-uapi-mm".to_owned(),
            "reovim-uapi-net".to_owned(),
            "reovim-uapi-panic".to_owned(),
            "reovim-uapi-process".to_owned(),
            "reovim-uapi-protocol".to_owned(),
            "reovim-uapi-sched".to_owned(),
            "reovim-uapi-session".to_owned(),
            "reovim-uapi-service".to_owned(),
            "reovim-uapi-source".to_owned(),
            "reovim-uapi-syscall".to_owned(),
            "reovim-uapi-system".to_owned(),
            "reovim-uapi-terminal".to_owned(),
        ],
    );
    // uapi/protocol depends on uapi/abi for ErrorCode, FrameHeader, RawInput,
    // and the carrier headers (Phase 1 of #786).
    m.insert("reovim-uapi-protocol".to_owned(), vec!["reovim-uapi-abi".to_owned()]);
    // uapi/fs owns product-facing fd wrappers; they lower through the raw
    // syscall transport without making syscall a second semantic API.
    m.insert("reovim-uapi-fs".to_owned(), vec!["reovim-uapi-syscall".to_owned()]);
    // uapi/dump owns diagnostic dump-domain controls; they lower through the
    // raw syscall transport without making syscall a second semantic API.
    m.insert("reovim-uapi-dump".to_owned(), vec!["reovim-uapi-syscall".to_owned()]);
    // uapi/process owns process-domain wrappers; they lower through the raw
    // syscall transport without making syscall a second semantic API.
    m.insert("reovim-uapi-process".to_owned(), vec!["reovim-uapi-syscall".to_owned()]);
    // uapi/sched owns scheduler-domain wrappers; they lower through the raw
    // syscall transport without making syscall a second semantic API.
    m.insert("reovim-uapi-sched".to_owned(), vec!["reovim-uapi-syscall".to_owned()]);
    // uapi/session owns shell-session wrappers; they lower through the raw
    // syscall transport without making syscall a second semantic API.
    m.insert("reovim-uapi-session".to_owned(), vec!["reovim-uapi-syscall".to_owned()]);
    // uapi/service owns service-domain lifecycle controls; they lower through
    // the raw syscall transport without making syscall a second semantic API.
    m.insert("reovim-uapi-service".to_owned(), vec!["reovim-uapi-syscall".to_owned()]);
    // uapi/source owns source-domain install controls; they lower through the
    // raw syscall transport without making syscall a second semantic API.
    m.insert("reovim-uapi-source".to_owned(), vec!["reovim-uapi-syscall".to_owned()]);
    // uapi/system owns typed system-control wrappers; they lower through the
    // raw syscall transport without making syscall a second semantic API.
    m.insert("reovim-uapi-system".to_owned(), vec!["reovim-uapi-syscall".to_owned()]);
    // uapi/terminal owns typed terminal-domain controls; they lower through
    // the raw syscall transport without exposing a generic ioctl surface.
    m.insert("reovim-uapi-terminal".to_owned(), vec!["reovim-uapi-syscall".to_owned()]);
    // uapi/log names the panic configuration result type for registering a log
    // sink as the panic flush target; both are up-face vocabulary crates.
    m.insert("reovim-uapi-log".to_owned(), vec!["reovim-uapi-panic".to_owned()]);
    // The declare-macro crates emit `::reovim_uapi_abi::*` paths textually,
    // so their [dependencies] tables stay empty — but their doc-tests
    // compile real expansions, making the dev-dependency edge to uapi/abi
    // architecturally real and granted here (Phase 4 of #786).
    m.insert("reovim-uapi-module-macros".to_owned(), vec!["reovim-uapi-abi".to_owned()]);
    m.insert("reovim-uapi-driver-macros".to_owned(), vec!["reovim-uapi-abi".to_owned()]);

    // ── Structural reorg grants (§6, arch → {kabi, lib/ds}) ─────────────────
    //
    // The grants table is exact-string lookup (NOT glob) — a glob key is
    // permanently inert and forbidden. The crate names below are the live
    // package names for the down-face platform contract and the portable DS
    // algorithm crate.
    m.insert(
        "reovim-arch".to_owned(),
        vec![
            "reovim-kabi-platform".to_owned(), // arch → kabi/platform (implements the vtable)
            "reovim-lib-ds".to_owned(),        // arch → lib/ds (uses DS algorithms via the handle)
            // arch → kabi/panic: the panic handler reads the five write-once
            // fault-floor atoms (relocated from arch) through kabi/panic's
            // get_*/set_* shims; the registry lives down-face there (#778,
            // Platform-Contract §3.4).
            "reovim-kabi-panic".to_owned(),
            // arch → uapi/panic: arch re-exports and names the Disposition /
            // PanicRecord value types whose canonical home is uapi/panic.
            "reovim-uapi-panic".to_owned(),
            // arch → arch-sys-{target}: the raw per-target mechanism carved out
            // of arch (SP01). arch reaches each through the `arch::sys` facade;
            // the deps are target-cfg-gated, so at most one enters a given
            // build's graph. Downward floor edges — no upward edge back.
            "reovim-arch-sys-linux-x86-64".to_owned(),
            "reovim-arch-sys-linux-aarch64".to_owned(),
            "reovim-arch-sys-none-aarch64".to_owned(),
            "reovim-arch-sys-none-x86-64".to_owned(),
            // arch → lib/testrt: arch re-exports the core-only test runtime
            // through the `arch::testrt` facade and wires its injected
            // output/TID edges onto arch's `sys` floor (SP07). lib/testrt is a
            // Foundation leaf naming nothing back — a permitted Foundation edge.
            "reovim-testrt".to_owned(),
        ],
    );
    // ── SP01/SP07 arch-sys grants ───────────────────────────────────────────
    // The none targets name the neutral boot-data types in kabi/platform
    // (MemoryKind / MemoryRange) for memory maps assembled from target-native
    // boot facts below the bridge. The device-neutral BootInfo assembly lives in
    // reovim-system-kernel; raw register/mailbox/Multiboot facts stay here
    // behind target-owned accessors.
    // Both crates depend on lib/testrt (SP07): their `selftest`-gated
    // `*_tests.rs` modules register through its arch_test! / check / check_eq,
    // reaching the runner via the leaf rather than an upward arch-sys → arch
    // edge. lib/testrt is a Foundation leaf — a permitted Foundation→Foundation
    // grant.
    m.insert(
        "reovim-arch-sys-none-aarch64".to_owned(),
        vec![
            "reovim-kabi-platform".to_owned(),
            "reovim-testrt".to_owned(),
        ],
    );
    m.insert(
        "reovim-arch-sys-none-x86-64".to_owned(),
        vec![
            "reovim-kabi-platform".to_owned(),
            "reovim-testrt".to_owned(),
        ],
    );
    // The two Linux crates are otherwise core-only (raw syscall layer); their
    // sole Foundation edge is the selftest runtime dep above.
    m.insert("reovim-arch-sys-linux-x86-64".to_owned(), vec!["reovim-testrt".to_owned()]);
    m.insert("reovim-arch-sys-linux-aarch64".to_owned(), vec!["reovim-testrt".to_owned()]);
    // ── Arch-floor language-floor grants ─────────────────────────────────────
    // Each per-target language-floor crate carries exactly three Foundation
    // edges: its matching `arch-sys-{target}` (the `exit_group`/`write`/clock
    // syscall floor + the `DTB_PTR`/`MULTIBOOT_INFO_PTR` boot-pointer statics the
    // `_start` asm names by `sym`), `kabi/panic` (the registry the relocated
    // `#[panic_handler]` reads), and `uapi/panic` (the `Disposition`/`PanicRecord`
    // value types the handler + `rust_eh_personality` name). The floor names NO
    // `arch` edge — that would close the floor→product airlock (invariant #8).
    m.insert(
        "reovim-arch-floor-linux-x86-64".to_owned(),
        vec![
            "reovim-arch-sys-linux-x86-64".to_owned(),
            "reovim-kabi-panic".to_owned(),
            "reovim-uapi-panic".to_owned(),
        ],
    );
    m.insert(
        "reovim-arch-floor-linux-aarch64".to_owned(),
        vec![
            "reovim-arch-sys-linux-aarch64".to_owned(),
            "reovim-kabi-panic".to_owned(),
            "reovim-uapi-panic".to_owned(),
        ],
    );
    m.insert(
        "reovim-arch-floor-none-aarch64".to_owned(),
        vec![
            "reovim-arch-sys-none-aarch64".to_owned(),
            "reovim-kabi-panic".to_owned(),
            "reovim-uapi-panic".to_owned(),
        ],
    );
    m.insert(
        "reovim-arch-floor-none-x86-64".to_owned(),
        vec![
            "reovim-arch-sys-none-x86-64".to_owned(),
            "reovim-kabi-panic".to_owned(),
            "reovim-uapi-panic".to_owned(),
        ],
    );
    // ── Platform-provider grants ─────────────────────────────────────────────
    // The hosted Linux provider implements the down-face kabi/platform
    // contract. Slot vocabulary is named through kabi/platform, never through a
    // direct provider -> uapi/posix edge. Raw mechanism comes from the matching
    // target arch-sys crate, not the reovim-arch facade.
    m.insert(
        "reovim-platform-linux-native".to_owned(),
        vec![
            "reovim-kabi-platform".to_owned(), // provider → kabi/platform (builds + installs the vtable)
            // provider -> arch-sys-{linux target}: raw syscall/termios/clone/
            // clock/mmap mechanism. The deps are target-cfg-gated, so at most
            // one enters a given build graph; the provider canonicalizes those
            // NATIVE results into the stable kabi/platform table.
            "reovim-arch-sys-linux-x86-64".to_owned(),
            "reovim-arch-sys-linux-aarch64".to_owned(),
        ],
    );
    // The bare-metal selftest handle scaffold builds + installs the same
    // `PlatformVtable` (kabi/platform), names slot vocabulary through kabi, and
    // reaches the matching freestanding arch-sys floor for clock, arena-backed
    // mmap, futex, and write. It carries no reovim-arch facade edge.
    m.insert(
        "reovim-platform-stub-none".to_owned(),
        vec![
            "reovim-kabi-platform".to_owned(), // scaffold → kabi/platform (builds + installs the vtable)
            "reovim-arch-sys-none-aarch64".to_owned(),
            "reovim-arch-sys-none-x86-64".to_owned(),
        ],
    );
    // ── System-kernel grant ──────────────────────────────────────────────────
    // The World system-kernel crate is the arch-free bridge: it shapes
    // caller-supplied raw facts into uapi/system boot data, owns common
    // console/FDT/splash/terminal/panic/log/fs/net bridge policy, maps up-face
    // terminal, panic/log, log-sink, borrowed-fd, and UDS requests from uapi/*
    // to down-face platform/panic slots, and registers selftests through
    // lib/testrt. It is not a provider and carries no
    // arch/platform/uapi-posix edge.
    m.insert(
        "reovim-system-kernel".to_owned(),
        vec![
            "reovim-kabi-panic".to_owned(),
            "reovim-kabi-platform".to_owned(),
            "reovim-lib-ds".to_owned(),
            "reovim-uapi-fs".to_owned(),
            "reovim-uapi-dump".to_owned(),
            "reovim-uapi-mm".to_owned(),
            "reovim-uapi-log".to_owned(),
            "reovim-uapi-net".to_owned(),
            "reovim-uapi-panic".to_owned(),
            "reovim-uapi-process".to_owned(),
            "reovim-uapi-sched".to_owned(),
            "reovim-uapi-session".to_owned(),
            "reovim-uapi-service".to_owned(),
            "reovim-uapi-source".to_owned(),
            "reovim-uapi-syscall".to_owned(),
            "reovim-uapi-system".to_owned(),
            "reovim-uapi-terminal".to_owned(),
            "reovim-testrt".to_owned(),
        ],
    );
    m.insert(
        "reovim-lib-ds".to_owned(),
        vec![
            // lib/ds → uapi/mm: heap-owning containers receive their allocator
            // backend as an injected up-face control table instead of naming
            // the down-face platform handle directly.
            "reovim-uapi-mm".to_owned(),
            // lib/ds → uapi/sched: futex-backed sync primitives receive their
            // park/unpark backend as an injected up-face control table instead
            // of naming the down-face platform handle directly.
            "reovim-uapi-sched".to_owned(),
        ],
    );
    // kabi/platform is a down-face leaf. Its POSIX-shaped Fd/OpenFlags/Mode/
    // Errno newtypes and open-flag constants live inside the contract so lower
    // layers do not import any product-facing uapi for provider slot scalars.
    // kabi/panic → uapi/panic: the fault-floor seam stores and exposes hook
    // fn-pointer aliases (RingTailProviderFn/StateRecordHookFn/PreExitHookFn)
    // and value types (Disposition/PanicRecord) from uapi/panic. This is the
    // only permitted intra-Foundation edge for kabi/panic (Platform-Contract §3.4).
    m.insert(
        "reovim-kabi-panic".to_owned(),
        vec!["reovim-uapi-panic".to_owned()], // kabi/panic → uapi/panic (Disposition/PanicRecord + hook aliases)
    );
    // ─────────────────────────────────────────────────────────────────────────
    m
}

/// Returns true when `pattern` matches the crate directory `path`
/// (workspace-relative, `/`-separated).
///
/// ```rust
/// use reovim_depgraph::pattern_matches;
///
/// // Exact path match.
/// assert!(pattern_matches("arch", "arch"));
/// // Wildcard: `lib/*` matches any direct child of `lib`.
/// assert!(pattern_matches("lib/*", "lib/depgraph"));
/// // Wildcard does not match the parent itself.
/// assert!(!pattern_matches("lib/*", "lib"));
/// // Deep path under a wildcard.
/// assert!(pattern_matches("editor/modules/*", "editor/modules/vim"));
/// // No match for a sibling.
/// assert!(!pattern_matches("apps/*", "client/modules/foo"));
/// ```
#[must_use]
pub fn pattern_matches(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('/').collect();
    let comps: Vec<&str> = path.split('/').collect();
    let (fixed, wildcard) = match pat.split_last() {
        Some((&"*", head)) => (head, true),
        _ => (pat.as_slice(), false),
    };
    if comps.len() < fixed.len() {
        return false;
    }
    if !fixed.iter().zip(&comps).all(|(a, b)| a == b) {
        return false;
    }
    if wildcard {
        comps.len() > fixed.len()
    } else {
        true
    }
}

/// One workspace crate as the probe sees it: manifest-derived only
/// (DAG4 — no install-layout input exists in this model).
///
/// ```rust
/// use reovim_depgraph::Crate;
///
/// let c = Crate { name: "reovim-arch".to_owned(), path: "arch".to_owned(), deps: vec![] };
/// assert_eq!(c.name, "reovim-arch");
/// assert_eq!(c.path, "arch");
/// assert!(c.deps.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct Crate {
    /// Package name from `[package].name`.
    pub name: String,
    /// Workspace-relative directory, `/`-separated.
    pub path: String,
    /// Named dependencies: name + parsed shape for each dep table entry.
    pub deps: Vec<DepEntry>,
}

/// One entry from a dependency table, carrying enough shape for DAG2 + DAG5.
///
/// ```rust
/// use reovim_depgraph::{DepEntry, DepTable};
///
/// let e = DepEntry {
///     name: "reovim-editor-core".to_owned(),
///     table: DepTable::Dependencies,
///     is_path: true,
///     is_workspace_true: false,
///     optional: false,
/// };
/// assert!(e.is_path);
/// assert!(!e.is_workspace_true);
/// assert!(!e.optional);
/// assert_eq!(format!("{}", e.table), "dependencies");
/// ```
#[derive(Debug, Clone)]
pub struct DepEntry {
    /// Resolved package name (after `package = "..."` renaming).
    pub name: String,
    /// Which dependency table this entry came from.
    pub table: DepTable,
    /// Whether the dep is an in-repo `path = "..."`.
    pub is_path: bool,
    /// Whether the dep has `workspace = true`.
    pub is_workspace_true: bool,
    /// Whether the dep is declared `optional = true`.
    ///
    /// An optional dep is compiled into the product build ONLY when a Cargo
    /// feature activates it (`dep:name`). The firewall probe treats an optional
    /// dep as a non-product edge: a genuinely-needed product dep cannot hide
    /// behind `optional = true`, because a product build that omits the
    /// activating feature would fail to compile — the compiler enforces what
    /// the firewall no longer needs to police.
    pub optional: bool,
}

/// Which Cargo dependency table a dep entry came from.
///
/// ```rust
/// use reovim_depgraph::DepTable;
///
/// assert_eq!(format!("{}", DepTable::Dependencies), "dependencies");
/// assert_eq!(format!("{}", DepTable::DevDependencies), "dev-dependencies");
/// assert_eq!(format!("{}", DepTable::BuildDependencies), "build-dependencies");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepTable {
    /// `[dependencies]`
    Dependencies,
    /// `[dev-dependencies]`
    DevDependencies,
    /// `[build-dependencies]`
    BuildDependencies,
}

impl fmt::Display for DepTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Dependencies => "dependencies",
            Self::DevDependencies => "dev-dependencies",
            Self::BuildDependencies => "build-dependencies",
        })
    }
}

/// A violation found by the probe. Every variant is fail-closed:
/// the probe reports it unless an §8 allowlist entry covers it.
///
/// ```rust
/// use reovim_depgraph::Violation;
///
/// // Violation::Display shows the DAG rule and the offending path.
/// let v = Violation::UnknownPath { path: "mystery/crate".to_owned() };
/// let msg = format!("{v}");
/// assert!(msg.contains("DAG1"));
/// assert!(msg.contains("mystery/crate"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    /// DAG1: crate path matched no §1 category pattern.
    UnknownPath { path: String },
    /// DAG1: crate path matched more than one §1 category pattern.
    AmbiguousPath {
        path: String,
        candidates: Vec<String>,
    },
    /// DAG2: category-level edge not granted by §2.
    ForbiddenEdge {
        from: String,
        from_category: Category,
        to: String,
        to_category: Category,
    },
    /// §6: foundation-internal edge without a sub-DAG grant.
    UngrantedFoundationEdge { from: String, to: String },
    /// DAG3: `apps/*`/`tools/*` edge missing from the §7 catalog.
    UncatalogedCompositionEdge { from: String, to: String },
    /// DAG5: third-party or non-path dependency detected in a workspace manifest.
    NonSovereignDep {
        /// Workspace-relative path of the crate's manifest dir.
        crate_path: String,
        /// Dependency table where the violation was found.
        table: DepTable,
        /// Dependency name.
        dep: String,
    },
    /// DAG6: a product crate's root source file is missing `#![no_std]`.
    MissingNoStd {
        /// Crate name from `[package].name`.
        crate_name: String,
        /// Workspace-relative path of the crate.
        crate_path: String,
    },
    /// DAG6: a product crate source file uses `std` (outside `#[cfg(test)]` blocks).
    StdUsage {
        /// Path of the offending source file.
        file: PathBuf,
    },
    /// DAG6: a product crate source file uses `alloc` (outside `#[cfg(test)]` blocks).
    AllocUsage {
        /// Path of the offending source file.
        file: PathBuf,
    },
    /// DAG6: `[profile.dev]` or `[profile.release]` in the workspace manifest
    /// does not set `panic = "abort"`, or the key is absent.
    PanicProfileNotAbort {
        /// Which profile section was wrong or missing the key.
        profile: String,
    },
    /// L11: a `uapi/protocol` source file imports a crate outside the purity
    /// allowlist (`core`, `reovim_uapi_abi`, intra-crate paths).
    ForbiddenExternalImport {
        /// Path of the source file containing the forbidden import.
        file: PathBuf,
        /// The forbidden import token (e.g. `arch`, `std`, `alloc`).
        import: String,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPath { path } => {
                write!(f, "DAG1: crate at `{path}` matches no source category")
            }
            Self::AmbiguousPath { path, candidates } => write!(
                f,
                "DAG1: crate at `{path}` matches more than one source category pattern: {}",
                candidates.join(", ")
            ),
            Self::ForbiddenEdge {
                from,
                from_category,
                to,
                to_category,
            } => write!(
                f,
                "DAG2: `{from}` ({from_category}) may not depend on `{to}` ({to_category})"
            ),
            Self::UngrantedFoundationEdge { from, to } => {
                write!(f, "DAG2: foundation crate `{from}` has no §6 grant for `{to}`")
            }
            Self::UncatalogedCompositionEdge { from, to } => write!(
                f,
                "DAG3: composition edge `{from}` -> `{to}` is not in the composition catalog"
            ),
            Self::NonSovereignDep {
                crate_path,
                table,
                dep,
            } => write!(
                f,
                "DAG5: `{crate_path}` [{table}]: `{dep}` is not an in-repo path dependency"
            ),
            Self::MissingNoStd {
                crate_name,
                crate_path,
            } => write!(
                f,
                "DAG6: `{crate_name}` (`{crate_path}`): crate root is missing `#![no_std]`"
            ),
            Self::StdUsage { file } => {
                write!(f, "DAG6: `{}`: `std` usage in product source", file.display())
            }
            Self::AllocUsage { file } => {
                write!(f, "DAG6: `{}`: `alloc` usage in product source", file.display())
            }
            Self::PanicProfileNotAbort { profile } => write!(
                f,
                "DAG6: `[profile.{profile}]` does not set `panic = \"abort\"` (missing or wrong value)"
            ),
            Self::ForbiddenExternalImport { file, import } => write!(
                f,
                "L11: `{}`: forbidden external import `{import}` (only `core` and `reovim_uapi_abi` allowed)",
                file.display()
            ),
        }
    }
}

/// Errors raised while loading the workspace or probe inputs.
///
/// ```rust
/// use reovim_depgraph::ProbeError;
/// use std::path::PathBuf;
///
/// let e = ProbeError::Parse { path: PathBuf::from("Cargo.toml"), message: "missing name".to_owned() };
/// let msg = format!("{e}");
/// assert!(msg.contains("Cargo.toml"));
/// assert!(msg.contains("missing name"));
/// ```
#[derive(Debug)]
pub enum ProbeError {
    /// Filesystem read failure.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A `Cargo.toml`, catalog, or allowlist failed to parse.
    Parse { path: PathBuf, message: String },
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "io error at `{}`: {source}", path.display())
            }
            Self::Parse { path, message } => {
                write!(f, "parse error at `{}`: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for ProbeError {}

/// §7 composition catalog (`composition-edges.toml`).
///
/// ```rust
/// use reovim_depgraph::{Catalog, CatalogEdge};
///
/// let catalog = Catalog {
///     edge: vec![CatalogEdge {
///         from: "reovim".to_owned(),
///         to: "reovim-editor-core".to_owned(),
///         gate: None,
///         reason: "top-level compositor".to_owned(),
///     }],
/// };
/// assert_eq!(catalog.edge.len(), 1);
/// assert_eq!(catalog.edge[0].from, "reovim");
/// ```
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    /// Named composition edges.
    pub edge: Vec<CatalogEdge>,
}

impl Catalog {
    /// Validates that every entry has a non-blank `reason` and, if present,
    /// a non-blank `gate`.
    ///
    /// Blank fields turn the catalog into an unaudited pass-list, which is
    /// indistinguishable from a forbidden edge.
    ///
    /// # Errors
    ///
    /// Returns `ProbeError::Parse` when any entry has a blank required field.
    pub fn validate(&self, path: &Path) -> Result<(), ProbeError> {
        for edge in &self.edge {
            if edge.reason.trim().is_empty() {
                return Err(schema_error(
                    path,
                    &format!("edge `{}` -> `{}`: empty reason", edge.from, edge.to),
                ));
            }
            if edge.gate.as_ref().is_some_and(|g| g.trim().is_empty()) {
                return Err(schema_error(
                    path,
                    &format!("edge `{}` -> `{}`: empty gate", edge.from, edge.to),
                ));
            }
        }
        Ok(())
    }
}

/// One named composition edge.
///
/// ```rust
/// use reovim_depgraph::CatalogEdge;
///
/// let edge = CatalogEdge {
///     from: "reovim".to_owned(),
///     to: "reovim-server".to_owned(),
///     gate: Some("embedded-server".to_owned()),
///     reason: "top-level compositor wires server library".to_owned(),
/// };
/// assert_eq!(edge.from, "reovim");
/// assert!(edge.gate.is_some());
/// ```
#[derive(Debug, Clone)]
pub struct CatalogEdge {
    /// Depending package name.
    pub from: String,
    /// Depended-on package name.
    pub to: String,
    /// Optional feature gate.
    pub gate: Option<String>,
    /// Mandatory justification.
    pub reason: String,
}

/// §8 transitional allowlist (`transitional-allowlist.toml`).
///
/// ```rust
/// use reovim_depgraph::{Allowlist, AllowlistEntry};
///
/// let allowlist = Allowlist {
///     entry: vec![AllowlistEntry {
///         from: "reovim-server".to_owned(),
///         to: "reovim-tui".to_owned(),
///         reason: "transitional coupling".to_owned(),
///         issue: "#775".to_owned(),
///         expires: "when #775 lands".to_owned(),
///     }],
/// };
/// assert_eq!(allowlist.entry.len(), 1);
/// assert_eq!(allowlist.entry[0].issue, "#775");
/// ```
#[derive(Debug, Clone, Default)]
pub struct Allowlist {
    /// Time-bounded known violations.
    pub entry: Vec<AllowlistEntry>,
}

impl Allowlist {
    /// Validates that every entry has non-blank `reason`, `issue`, and
    /// `expires` fields.
    ///
    /// Blank fields would admit an untracked permanent hole rather than a
    /// transition.
    ///
    /// # Errors
    ///
    /// Returns `ProbeError::Parse` when any entry has a blank required field.
    pub fn validate(&self, path: &Path) -> Result<(), ProbeError> {
        for entry in &self.entry {
            for (field, value) in [
                ("reason", &entry.reason),
                ("issue", &entry.issue),
                ("expires", &entry.expires),
            ] {
                if value.trim().is_empty() {
                    return Err(schema_error(
                        path,
                        &format!("entry `{}` -> `{}`: empty {field}", entry.from, entry.to),
                    ));
                }
            }
        }
        Ok(())
    }
}

fn schema_error(path: &Path, message: &str) -> ProbeError {
    ProbeError::Parse {
        path: path.to_path_buf(),
        message: message.to_owned(),
    }
}

/// One dated allowlist entry. `issue` and `expires` are mandatory:
/// an entry without an expiry is a forbidden edge, not a transition.
///
/// ```rust
/// use reovim_depgraph::AllowlistEntry;
///
/// let e = AllowlistEntry {
///     from: "a".to_owned(), to: "b".to_owned(),
///     reason: "temporary".to_owned(),
///     issue: "#100".to_owned(),
///     expires: "when #100 lands".to_owned(),
/// };
/// assert!(!e.expires.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct AllowlistEntry {
    /// Depending crate (package name or source path).
    pub from: String,
    /// Depended-on package name.
    pub to: String,
    /// Justification.
    pub reason: String,
    /// Tracking issue reference.
    pub issue: String,
    /// Expiry criterion.
    pub expires: String,
}

/// Probe configuration: the spec tables as data. `default_for` loads the
/// real tables; fixtures may construct custom ones.
///
/// ```rust
/// use reovim_depgraph::{ProbeConfig, Catalog, Allowlist, default_category_table};
/// use std::collections::BTreeMap;
///
/// // Build a minimal in-memory config with empty catalog and allowlist.
/// let config = ProbeConfig {
///     category_table: default_category_table(),
///     foundation_grants: BTreeMap::new(),
///     catalog: Catalog::default(),
///     allowlist: Allowlist::default(),
/// };
/// assert!(!config.category_table.is_empty());
/// assert!(config.catalog.edge.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct ProbeConfig {
    /// §1 category table.
    pub category_table: Vec<(String, Category)>,
    /// §6 foundation sub-DAG grants: crate name → allowed workspace dep
    /// names. Foundation-internal edges absent from this map are violations.
    pub foundation_grants: BTreeMap<String, Vec<String>>,
    /// §7 composition catalog.
    pub catalog: Catalog,
    /// §8 transitional allowlist.
    pub allowlist: Allowlist,
}

impl ProbeConfig {
    /// Loads the normative configuration for a workspace root:
    /// the §1/§6 tables compiled in, and the §7/§8 TOML files from
    /// `tools/depgraph-probes/` (absent files mean empty tables).
    ///
    /// # Errors
    ///
    /// Returns `ProbeError::Io` / `ProbeError::Parse` when a catalog or
    /// allowlist file exists but cannot be read, parsed, or schema-validated.
    pub fn default_for(root: &Path) -> Result<Self, ProbeError> {
        let catalog_path = root.join("tools/depgraph-probes/composition-edges.toml");
        let allowlist_path = root.join("tools/depgraph-probes/transitional-allowlist.toml");
        let catalog = if catalog_path.exists() {
            let catalog = load_catalog(&catalog_path)?;
            catalog.validate(&catalog_path)?;
            catalog
        } else {
            Catalog::default()
        };
        let allowlist = if allowlist_path.exists() {
            let allowlist = load_allowlist(&allowlist_path)?;
            allowlist.validate(&allowlist_path)?;
            allowlist
        } else {
            Allowlist::default()
        };
        Ok(Self {
            category_table: default_category_table(),
            foundation_grants: default_foundation_grants(),
            catalog,
            allowlist,
        })
    }
}

fn load_catalog(path: &Path) -> Result<Catalog, ProbeError> {
    let doc = toml::parse_file(path)?;
    let mut edges = Vec::new();
    for record in doc.array("edge") {
        let from = get_required_str(record, "from", path)?;
        let to = get_required_str(record, "to", path)?;
        let reason = get_required_str(record, "reason", path)?;
        let gate = record.get("gate").cloned();
        edges.push(CatalogEdge {
            from: from.to_owned(),
            to: to.to_owned(),
            gate,
            reason: reason.to_owned(),
        });
    }
    Ok(Catalog { edge: edges })
}

fn load_allowlist(path: &Path) -> Result<Allowlist, ProbeError> {
    let doc = toml::parse_file(path)?;
    let mut entries = Vec::new();
    for record in doc.array("entry") {
        let from = get_required_str(record, "from", path)?;
        let to = get_required_str(record, "to", path)?;
        let reason = get_required_str(record, "reason", path)?;
        let issue = get_required_str(record, "issue", path)?;
        let expires = get_required_str(record, "expires", path)?;
        entries.push(AllowlistEntry {
            from: from.to_owned(),
            to: to.to_owned(),
            reason: reason.to_owned(),
            issue: issue.to_owned(),
            expires: expires.to_owned(),
        });
    }
    Ok(Allowlist { entry: entries })
}

fn get_required_str<'a>(
    record: &'a std::collections::BTreeMap<String, String>,
    key: &str,
    path: &Path,
) -> Result<&'a str, ProbeError> {
    record
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| ProbeError::Parse {
            path: path.to_path_buf(),
            message: format!("missing required field `{key}` in array entry"),
        })
}

/// A violation suppressed by a dated §8 entry, carrying the entry's tracking
/// data so the suppression stays auditable in the report.
///
/// ```rust
/// use reovim_depgraph::{Allowlisted, Violation};
///
/// let a = Allowlisted {
///     violation: Violation::UnknownPath { path: "x/y".to_owned() },
///     issue: "#42".to_owned(),
///     expires: "when x/y is classified".to_owned(),
/// };
/// assert_eq!(a.issue, "#42");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allowlisted {
    /// The suppressed violation.
    pub violation: Violation,
    /// Tracking issue from the allowlist entry.
    pub issue: String,
    /// Expiry criterion from the allowlist entry.
    pub expires: String,
}

/// Probe outcome: per-category counts plus every violation found.
/// `allowlisted` records violations suppressed by §8 entries.
///
/// ```rust
/// use reovim_depgraph::Report;
///
/// let report = Report::default();
/// assert!(report.is_clean());
/// assert!(report.summary().is_empty());
/// ```
#[derive(Debug, Default)]
pub struct Report {
    /// Crate count per classified category.
    pub category_counts: BTreeMap<Category, usize>,
    /// Violations not covered by the allowlist.
    pub violations: Vec<Violation>,
    /// Violations suppressed by a dated §8 entry.
    pub allowlisted: Vec<Allowlisted>,
}

impl Report {
    /// True when the workspace conforms (no live violations).
    ///
    /// ```rust
    /// use reovim_depgraph::Report;
    ///
    /// let clean = Report::default();
    /// assert!(clean.is_clean());
    /// ```
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    /// Human-readable summary, one violation per line.
    ///
    /// ```rust
    /// use reovim_depgraph::Report;
    ///
    /// // A clean report produces an empty summary string.
    /// let report = Report::default();
    /// assert!(report.summary().is_empty());
    /// ```
    #[must_use]
    pub fn summary(&self) -> String {
        use fmt::Write as _;
        let mut out = String::new();
        for (category, count) in &self.category_counts {
            // Writing to a String is infallible.
            let _ = writeln!(out, "{category}: {count} crate(s)");
        }
        for a in &self.allowlisted {
            let _ =
                writeln!(out, "allowlisted ({}, expires {}): {}", a.issue, a.expires, a.violation);
        }
        for v in &self.violations {
            let _ = writeln!(out, "VIOLATION: {v}");
        }
        out
    }
}

/// Directories never descended into during enumeration. `archive` is the
/// spec's Archive category (excluded from the depgraph); `tests` holds test
/// fixtures (e.g. `arch/tests/fixtures/*`, the `#![no_std] #![no_main]` bins
/// exec'd by integration tests) which are test infrastructure, not workspace
/// product/tooling crates — they are not subject to §1 category classification
/// (DAG1) and the source sweep already skips `tests/` (the documented
/// bootstrap-state-1 handling); the rest are build/VCS/scratch artifacts.
const SKIP_DIRS: &[&str] = &["archive", "target", ".git", "tmp", ".github", "tests"];

/// Enumerates every workspace crate under `root` by scanning for
/// `Cargo.toml` files with a `[package]` section.
///
/// Scanning the tree (rather than reading `[workspace.members]`) is
/// deliberate: a crate dropped at an unlisted path must still be
/// classified, so DAG1 catches it before anyone wires it into the
/// workspace.
///
/// # Errors
///
/// Returns `ProbeError` on unreadable directories or unparseable manifests.
///
/// ```rust
/// use reovim_depgraph::enumerate_crates;
/// use std::path::Path;
///
/// // enumerate_crates on a non-existent directory returns Err (Io).
/// let result = enumerate_crates(Path::new("/nonexistent/path/xyz"));
/// assert!(result.is_err());
/// ```
pub fn enumerate_crates(root: &Path) -> Result<Vec<Crate>, ProbeError> {
    let mut crates = Vec::new();
    walk(root, "", &mut crates)?;
    crates.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(crates)
}

/// Maps an I/O error to `ProbeError::Io` at `path`. Shared by every
/// filesystem touch point so the conversion is a single covered region.
fn io_error(path: &Path) -> impl Fn(std::io::Error) -> ProbeError + '_ {
    move |source| ProbeError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Recursive enumeration step. `rel` is the `/`-separated
/// workspace-relative path of `dir` ("" at the root), threaded down
/// so crate paths never depend on prefix arithmetic.
fn walk(dir: &Path, rel: &str, out: &mut Vec<Crate>) -> Result<(), ProbeError> {
    // Collecting up front folds per-entry iteration errors into the
    // same fallible step as the directory open.
    let entries: Vec<fs::DirEntry> = fs::read_dir(dir)
        .and_then(Iterator::collect)
        .map_err(io_error(dir))?;
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if SKIP_DIRS.contains(&name.as_ref()) {
                continue;
            }
            let child_rel = if rel.is_empty() {
                name.into_owned()
            } else {
                format!("{rel}/{name}")
            };
            walk(&path, &child_rel, out)?;
        } else if name == "Cargo.toml"
            && !rel.is_empty()
            && let Some(krate) = parse_manifest(rel, &path)?
        {
            out.push(krate);
        }
    }
    Ok(())
}

fn parse_manifest(rel: &str, manifest: &Path) -> Result<Option<Crate>, ProbeError> {
    let doc = toml::parse_file(manifest)?;
    let Some(pkg_section) = doc.sections.get("package") else {
        // A nested virtual workspace manifest has no [package] section.
        return Ok(None);
    };
    let name = pkg_section
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ProbeError::Parse {
            path: manifest.to_path_buf(),
            message: "missing [package].name".to_owned(),
        })?
        .to_owned();

    let mut deps = Vec::new();
    for (label, dep_table) in [
        ("dependencies", DepTable::Dependencies),
        ("dev-dependencies", DepTable::DevDependencies),
        ("build-dependencies", DepTable::BuildDependencies),
    ] {
        // Inline-table / bare-string form: `[dependencies]` with one row per dep.
        if let Some(section) = doc.sections.get(label) {
            for (key, spec) in section {
                let entry = dep_entry_from_toml_value(key, spec, dep_table, manifest)?;
                deps.push(entry);
            }
        }
        // Section-table form: `[dependencies.dep-name]` blocks, each a section
        // whose header is `<label>.<dep-name>`. The parser stores these as a
        // sibling section keyed by the dotted header, so a dep declared this
        // way is invisible to the inline pass above and must be swept here —
        // otherwise `optional`/`path` carried by a section-table dep would not
        // reach the firewall.
        let prefix = format!("{label}.");
        for (header, fields) in &doc.sections {
            let Some(dep_key) = header.strip_prefix(&prefix) else {
                continue;
            };
            let entry = dep_entry_from_section_table(dep_key, fields, dep_table);
            deps.push(entry);
        }
    }
    Ok(Some(Crate {
        name,
        path: rel.to_owned(),
        deps,
    }))
}

/// Builds a [`DepEntry`] from a section-table dep block
/// (`[dependencies.name]` with `path`/`optional`/`package` keys as rows).
///
/// Mirrors the inline-table arm of [`dep_entry_from_toml_value`]: `package`
/// renames the dep, `path` marks it sovereign, `workspace`/`optional` carry
/// their booleans. The keys are plain [`toml::TomlValue`] rows the parser
/// already validated, so this arm cannot fail.
fn dep_entry_from_section_table(
    key: &str,
    fields: &std::collections::BTreeMap<String, toml::TomlValue>,
    table: DepTable,
) -> DepEntry {
    let resolved_name = fields
        .get("package")
        .and_then(toml::TomlValue::as_str)
        .unwrap_or(key)
        .to_owned();
    let is_path = fields.contains_key("path");
    let is_workspace_true = fields
        .get("workspace")
        .and_then(toml::TomlValue::as_bool)
        .unwrap_or(false);
    let optional = fields
        .get("optional")
        .and_then(toml::TomlValue::as_bool)
        .unwrap_or(false);
    DepEntry {
        name: resolved_name,
        table,
        is_path,
        is_workspace_true,
        optional,
    }
}

fn dep_entry_from_toml_value(
    key: &str,
    spec: &toml::TomlValue,
    table: DepTable,
    manifest: &Path,
) -> Result<DepEntry, ProbeError> {
    match spec {
        toml::TomlValue::String(_) => {
            // Bare version string: `dep = "1.0"` — registry dep, not path.
            Ok(DepEntry {
                name: key.to_owned(),
                table,
                is_path: false,
                is_workspace_true: false,
                optional: false,
            })
        }
        toml::TomlValue::InlineTable(t) => {
            // `package` key renames the dependency.
            let resolved_name = t
                .get("package")
                .and_then(toml::TomlValue::as_str)
                .unwrap_or(key)
                .to_owned();
            let is_path = t.contains_key("path");
            let is_workspace_true = t
                .get("workspace")
                .and_then(toml::TomlValue::as_bool)
                .unwrap_or(false);
            let optional = t
                .get("optional")
                .and_then(toml::TomlValue::as_bool)
                .unwrap_or(false);
            Ok(DepEntry {
                name: resolved_name,
                table,
                is_path,
                is_workspace_true,
                optional,
            })
        }
        toml::TomlValue::Bool(_) | toml::TomlValue::Integer(_) => Err(ProbeError::Parse {
            path: manifest.to_path_buf(),
            message: format!("unexpected scalar value for dependency `{key}`"),
        }),
        toml::TomlValue::Array(_) => Err(ProbeError::Parse {
            path: manifest.to_path_buf(),
            message: format!("unexpected array value for dependency `{key}`"),
        }),
    }
}

/// Classifies one crate path against a category table. Returns the
/// matched category, or the DAG1 violation when zero or multiple
/// patterns match.
///
/// # Errors
///
/// Returns `Violation::UnknownPath` when no pattern matches and
/// `Violation::AmbiguousPath` when more than one matches (DAG1).
///
/// ```rust
/// use reovim_depgraph::{Category, Violation, classify, default_category_table};
///
/// let table = default_category_table();
/// assert_eq!(classify("arch", &table), Ok(Category::Foundation));
/// assert_eq!(classify("apps/server", &table), Ok(Category::Apps));
/// // Unknown path → DAG1 violation.
/// assert!(matches!(classify("unknown/crate", &table), Err(Violation::UnknownPath { .. })));
/// ```
pub fn classify(path: &str, table: &[(String, Category)]) -> Result<Category, Violation> {
    let matches: Vec<&(String, Category)> = table
        .iter()
        .filter(|(pattern, _)| pattern_matches(pattern, path))
        .collect();
    match matches.as_slice() {
        [] => Err(Violation::UnknownPath {
            path: path.to_owned(),
        }),
        [(_, category)] => Ok(*category),
        many => {
            // Ambiguity is a CATEGORY conflict, not pattern overlap: an exact
            // entry and its own wildcard (e.g. `editor/lib/core` and
            // `editor/lib/core/*`, both editor-core) agree, and agreement
            // is a classification, not a violation.
            let first = many[0].1;
            if many.iter().all(|(_, c)| *c == first) {
                return Ok(first);
            }
            let mut candidates: Vec<String> =
                many.iter().map(|(p, c)| format!("`{p}` ({c})")).collect();
            candidates.sort();
            Err(Violation::AmbiguousPath {
                path: path.to_owned(),
                candidates,
            })
        }
    }
}

/// §2 category matrix: which categories `from` may depend on.
///
/// Foundation-internal edges are not listed here — they are governed
/// exclusively by the §6 grant table. Cells the spec conditions on
/// per-chapter lists that do not exist yet (peer subsys edges, ext peer
/// manifests, the client-subsys DAG) are closed; realising chapters open
/// them by spec edit plus a table change here.
const fn allowed_categories(from: Category) -> &'static [Category] {
    match from {
        Category::Foundation | Category::Apps | Category::Tools => &[],
        Category::ServerContracts | Category::ClientContracts => &[Category::Foundation],
        Category::EditorCore | Category::ServerExt => {
            &[Category::ServerContracts, Category::Foundation]
        }
        Category::ServerRuntime => &[
            Category::EditorCore,
            Category::ServerContracts,
            Category::Foundation,
        ],
        Category::ClientExt => &[Category::ClientContracts, Category::Foundation],
    }
}

/// Runs the full §5 probe over a workspace.
///
/// # Errors
///
/// Returns `ProbeError` when the workspace cannot be enumerated;
/// rule violations are reported in the `Report`, not as errors.
///
/// ```rust
/// use reovim_depgraph::{ProbeConfig, Catalog, Allowlist, default_category_table, run_probe};
/// use std::collections::BTreeMap;
/// use std::path::Path;
///
/// let config = ProbeConfig {
///     category_table: default_category_table(),
///     foundation_grants: BTreeMap::new(),
///     catalog: Catalog::default(),
///     allowlist: Allowlist::default(),
/// };
/// // run_probe on a non-existent root returns Err (Io).
/// let result = run_probe(Path::new("/nonexistent/xyz"), &config);
/// assert!(result.is_err());
/// ```
pub fn run_probe(root: &Path, config: &ProbeConfig) -> Result<Report, ProbeError> {
    let crates = enumerate_crates(root)?;
    let mut report = Report::default();
    let mut classified: BTreeMap<&str, Category> = BTreeMap::new();

    // DAG1: classify every crate.
    for krate in &crates {
        match classify(&krate.path, &config.category_table) {
            Ok(category) => {
                *report.category_counts.entry(category).or_insert(0) += 1;
                classified.insert(&krate.name, category);
            }
            Err(violation) => report.violations.push(violation),
        }
    }

    let mut raw_violations = Vec::new();

    // DAG2/DAG3: category-edge and composition checks.
    for krate in &crates {
        let Some(&from_category) = classified.get(krate.name.as_str()) else {
            // Unclassified crates already carry their DAG1 violation.
            continue;
        };
        for dep in &krate.deps {
            let Some(&to_category) = classified.get(dep.name.as_str()) else {
                // External dependency: outside the workspace graph — DAG5 handles it below.
                continue;
            };
            if let Some(v) = check_edge(krate, from_category, &dep.name, to_category, config) {
                raw_violations.push(v);
            }
        }
    }

    // DAG5: sovereignty walk — every dep must be an in-repo path dep.
    for krate in &crates {
        for dep in &krate.deps {
            if !dep.is_path && !dep.is_workspace_true {
                // Plain string version or unrecognised inline-table shape
                // without `path` or `workspace = true` → non-sovereign.
                raw_violations.push(Violation::NonSovereignDep {
                    crate_path: krate.path.clone(),
                    table: dep.table,
                    dep: dep.name.clone(),
                });
            } else if dep.is_workspace_true {
                // `{ workspace = true }` — the workspace dep table is intentionally
                // empty (L9 marker); any workspace = true dep is a violation.
                raw_violations.push(Violation::NonSovereignDep {
                    crate_path: krate.path.clone(),
                    table: dep.table,
                    dep: dep.name.clone(),
                });
            }
        }
    }

    // Apply allowlist to DAG2/DAG3 violations (DAG5 has no allowlist, §9).
    for violation in raw_violations {
        match &violation {
            Violation::NonSovereignDep { .. } => {
                // DAG5 violations are never allowlisted.
                report.violations.push(violation);
            }
            _ => {
                if let Some(entry) = allowlist_match(&violation, &config.allowlist) {
                    report.allowlisted.push(Allowlisted {
                        violation,
                        issue: entry.issue.clone(),
                        expires: entry.expires.clone(),
                    });
                } else {
                    report.violations.push(violation);
                }
            }
        }
    }

    Ok(report)
}

#[must_use]
pub(crate) fn check_edge(
    krate: &Crate,
    from_category: Category,
    dep: &str,
    to_category: Category,
    config: &ProbeConfig,
) -> Option<Violation> {
    match from_category {
        Category::Foundation => {
            let granted = config
                .foundation_grants
                .get(&krate.name)
                .is_some_and(|grants| grants.iter().any(|g| g == dep));
            if granted {
                None
            } else {
                Some(Violation::UngrantedFoundationEdge {
                    from: krate.name.clone(),
                    to: dep.to_owned(),
                })
            }
        }
        Category::Apps | Category::Tools => {
            let cataloged = config
                .catalog
                .edge
                .iter()
                .any(|e| e.from == krate.name && e.to == dep);
            if cataloged {
                None
            } else {
                Some(Violation::UncatalogedCompositionEdge {
                    from: krate.name.clone(),
                    to: dep.to_owned(),
                })
            }
        }
        _ => {
            if allowed_categories(from_category).contains(&to_category) {
                None
            } else {
                Some(Violation::ForbiddenEdge {
                    from: krate.name.clone(),
                    from_category,
                    to: dep.to_owned(),
                    to_category,
                })
            }
        }
    }
}

#[must_use]
pub(crate) fn allowlist_match<'a>(
    violation: &Violation,
    allowlist: &'a Allowlist,
) -> Option<&'a AllowlistEntry> {
    let (from, to) = match violation {
        Violation::ForbiddenEdge { from, to, .. }
        | Violation::UngrantedFoundationEdge { from, to }
        | Violation::UncatalogedCompositionEdge { from, to } => (from, to),
        Violation::UnknownPath { .. }
        | Violation::AmbiguousPath { .. }
        | Violation::NonSovereignDep { .. }
        | Violation::MissingNoStd { .. }
        | Violation::StdUsage { .. }
        | Violation::AllocUsage { .. }
        | Violation::PanicProfileNotAbort { .. }
        | Violation::ForbiddenExternalImport { .. } => return None,
    };
    allowlist
        .entry
        .iter()
        .find(|e| &e.from == from && &e.to == to)
}

// ── DAG6: zero-std walk + profile gate ───────────────────────────────────────

/// Checks `[profile.dev]` and `[profile.release]` in the workspace root
/// `Cargo.toml`, returning one `PanicProfileNotAbort` violation per profile
/// that is absent or does not set `panic = "abort"`.
///
/// The workspace manifest is at `root/Cargo.toml`.
///
/// This function uses a direct line scan rather than the TOML reader to
/// avoid failures on workspace manifests that contain constructs not in the
/// TOML reader's supported subset (e.g. `priority = -1` in lint tables,
/// single-quoted strings in `check-cfg` arrays).  The scan is intentionally
/// minimal: it finds `[profile.dev]` / `[profile.release]` section headers
/// and then looks for a `panic = "abort"` key-value pair before the next
/// section header.  This approach is robust and branch-complete.
///
/// # Errors
///
/// Returns `ProbeError::Io` when the root manifest cannot be read.
///
/// ```rust
/// use reovim_depgraph::check_panic_profiles;
/// use std::path::Path;
///
/// // A non-existent root returns Err (Io — no Cargo.toml there).
/// let result = check_panic_profiles(Path::new("/nonexistent/xyz"));
/// assert!(result.is_err());
/// ```
pub fn check_panic_profiles(root: &Path) -> Result<Vec<Violation>, ProbeError> {
    let manifest = root.join("Cargo.toml");
    let text = fs::read_to_string(&manifest).map_err(|source| ProbeError::Io {
        path: manifest.clone(),
        source,
    })?;
    let mut violations = Vec::new();
    for profile in ["dev", "release"] {
        if !profile_has_panic_abort(&text, profile) {
            violations.push(Violation::PanicProfileNotAbort {
                profile: profile.to_owned(),
            });
        }
    }
    Ok(violations)
}

/// Returns `true` when `text` (a `Cargo.toml` file) contains a
/// `[profile.<name>]` section that includes `panic = "abort"`.
///
/// The scan:
/// 1. Locates the section header `[profile.<name>]` (exact match after
///    stripping line comments and whitespace).
/// 2. Reads lines until the next `[` (new section) or EOF.
/// 3. Checks each line (comment-stripped) for `panic = "abort"`.
///
/// Case-sensitive, tolerates surrounding whitespace.
#[must_use]
fn profile_has_panic_abort(text: &str, profile_name: &str) -> bool {
    let section_header = format!("[profile.{profile_name}]");
    let mut in_section = false;
    for raw_line in text.lines() {
        let line = toml::strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_section = line == section_header;
            continue;
        }
        if in_section {
            // Check: `panic = "abort"` (tolerates whitespace around `=`).
            let stripped = line.replace(' ', "");
            if stripped.contains("panic=\"abort\"") {
                return true;
            }
        }
    }
    false
}

/// Runs the DAG6 zero-std walk over every product crate under `root`.
///
/// Product crates are all crates enumerated by `enumerate_crates` except
/// those whose workspace-relative path is in `DAG6_BOOTSTRAP_EXCLUSIONS`.
///
/// For each product crate the walk:
/// 1. Locates `src/lib.rs` or `src/main.rs` (crate root) and checks for
///    a `#![no_std]` inner attribute.  Missing → `MissingNoStd`.
/// 2. Sweeps every `.rs` file under `src/` (excluding `tests/` subdirs)
///    for `extern crate std`, `use std::`, `extern crate alloc`, and
///    `use alloc::` — each hit outside a `#[cfg(test)]` block → `StdUsage`
///    or `AllocUsage`.
///
/// # Errors
///
/// Returns `ProbeError::Io` / `ProbeError::Parse` when enumeration or a
/// manifest cannot be read.
///
/// ```rust
/// use reovim_depgraph::run_dag6_probe;
/// use std::path::Path;
///
/// // A non-existent root returns Err (Io).
/// let result = run_dag6_probe(Path::new("/nonexistent/xyz"));
/// assert!(result.is_err());
/// ```
pub fn run_dag6_probe(root: &Path) -> Result<Vec<Violation>, ProbeError> {
    let crates = enumerate_crates(root)?;
    let mut violations = Vec::new();
    for krate in &crates {
        if DAG6_BOOTSTRAP_EXCLUSIONS.contains(&krate.path.as_str()) {
            continue;
        }
        let crate_src = root.join(&krate.path).join("src");
        // 1. Crate-root `#![no_std]` check.
        check_crate_root_no_std(krate, &crate_src, &mut violations);
        // 2. Source sweep for std/alloc usage.
        if crate_src.is_dir() {
            sweep_src_dir(&crate_src, &mut violations)?;
        }
    }
    Ok(violations)
}

/// Checks whether the crate root (`src/lib.rs` or `src/main.rs`) declares
/// `#![no_std]`, appending a `MissingNoStd` violation when absent.
///
/// The check uses a textual scan: it walks lines until it finds
/// `#![no_std]`, skipping line comments (`// ...`) and tolerating doc
/// comments and other attributes that appear above it.  A line that
/// contains `#![no_std]` (not inside a line comment) is a match.
///
/// Limitation: block comments (`/* ... */`) are not stripped.  A
/// `#![no_std]` hidden inside a block comment would produce a false
/// negative (no violation when there should be one).  This is
/// documented as an acceptable limitation; the real workspace has no
/// block-commented inner attributes.
fn check_crate_root_no_std(krate: &Crate, crate_src: &Path, violations: &mut Vec<Violation>) {
    let root_file = crate_src
        .join("lib.rs")
        .exists()
        .then(|| crate_src.join("lib.rs"))
        .or_else(|| {
            let m = crate_src.join("main.rs");
            m.exists().then_some(m)
        });
    let Some(root_path) = root_file else {
        // No src/lib.rs or src/main.rs: treat as missing no_std.
        violations.push(Violation::MissingNoStd {
            crate_name: krate.name.clone(),
            crate_path: krate.path.clone(),
        });
        return;
    };
    let Ok(text) = fs::read_to_string(&root_path) else {
        // Unreadable file: treat as missing no_std (fail closed).
        violations.push(Violation::MissingNoStd {
            crate_name: krate.name.clone(),
            crate_path: krate.path.clone(),
        });
        return;
    };
    if !has_no_std_attr(&text) {
        violations.push(Violation::MissingNoStd {
            crate_name: krate.name.clone(),
            crate_path: krate.path.clone(),
        });
    }
}

/// Returns `true` when `text` contains a top-level `#![no_std]` inner
/// attribute (not inside a line comment on the same line).
///
/// The scan stops at the first line that is NOT a blank line, a line
/// comment, a doc comment (`//!`, `///`), or an attribute line starting
/// with `#`.  This mirrors the typical file layout where `#![no_std]`
/// appears at the top before any item.  A `#![no_std]` anywhere in the
/// file that is not behind a line comment is accepted.
///
/// Limitation: block-comment-hidden `#![no_std]` is not detected (not
/// stripped).  Documented in `check_crate_root_no_std`.
///
/// ```rust
/// use reovim_depgraph::has_no_std_attr;
///
/// assert!(has_no_std_attr("#![no_std]\npub fn foo() {}"));
/// // Commented-out attribute is not a match.
/// assert!(!has_no_std_attr("// #![no_std]\npub fn foo() {}"));
/// // Absent attribute.
/// assert!(!has_no_std_attr("pub fn foo() {}"));
/// ```
#[must_use]
pub fn has_no_std_attr(text: &str) -> bool {
    for line in text.lines() {
        let stripped = strip_line_comment(line).trim();
        if stripped.contains("#![no_std]") {
            return true;
        }
    }
    false
}

/// Recursively sweeps every `.rs` file under `dir` (excluding `tests/`
/// subdirectories) for `std` and `alloc` usage outside `#[cfg(test)]`
/// blocks.
///
/// Appends `StdUsage { file }` or `AllocUsage { file }` violations.
///
/// # Errors
///
/// Returns `ProbeError::Io` when a directory cannot be read.
fn sweep_src_dir(dir: &Path, violations: &mut Vec<Violation>) -> Result<(), ProbeError> {
    let entries: Vec<fs::DirEntry> = fs::read_dir(dir)
        .and_then(Iterator::collect)
        .map_err(io_error(dir))?;
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if path.is_dir() {
            // Skip `tests/` directories: test targets may use std.
            if name_str == "tests" {
                continue;
            }
            sweep_src_dir(&path, violations)?;
        } else if path.extension().is_some_and(|e| e == "rs")
            && let Ok(text) = fs::read_to_string(&path)
        {
            // Unreadable files are silently skipped (not a violation source).
            check_source_for_std_alloc(&path, &text, violations);
        }
    }
    Ok(())
}

/// Scans `text` (the contents of `file`) for `std` or `alloc` usage
/// outside `#[cfg(test)]` blocks, appending violations.
///
/// # Matching patterns
///
/// - `extern crate std` → `StdUsage`
/// - `use std::` → `StdUsage`
/// - `extern crate alloc` → `AllocUsage`
/// - `use alloc::` → `AllocUsage`
///
/// # cfg(test) skip
///
/// When the scanner encounters `#[cfg(test)]` on a line, it checks
/// whether the *next non-blank, non-comment* line opens a module block
/// (`{` is present) or is an item line.
///
/// - If a `{` is found on that line (or the `#[cfg(test)]` line itself),
///   the scanner skips forward using bounded brace-matching until the
///   matching `}` at depth 0.  This covers the `#[cfg(test)] mod tests { ... }`
///   pattern.
/// - If the next non-blank line does NOT contain `{`, the attribute is
///   assumed to guard a single item (e.g. `#[cfg(test)] fn ...`), and
///   only that one line is skipped.
///
/// **Documented limitation**: a `#[cfg(test)]` attribute on a non-module
/// item that spans multiple lines (e.g. a multi-line function signature)
/// is only skipped to the end of the first line of the item.  In practice,
/// the patterns we match (`use std::`, `extern crate std`) are single-line
/// statements; this limitation does not produce false positives in the
/// real workspace.
///
/// # Comment stripping
///
/// Line comments (`// ...`) are stripped from each line before matching,
/// so `// use std::fmt;` does not trigger a violation.
///
/// **Documented limitation**: block comments (`/* ... */`) are not
/// stripped.  A `use std::` inside a block comment would be a false
/// positive.  The real workspace does not use block-commented imports.
fn check_source_for_std_alloc(file: &Path, text: &str, violations: &mut Vec<Violation>) {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        let raw = lines[i];
        let stripped = strip_line_comment(raw).trim();

        // Detect `#[cfg(test)]` attribute.
        if stripped == "#[cfg(test)]" {
            // Look ahead for the block body or a single-item skip.
            let advance = cfg_test_skip(&lines, i);
            i += advance;
            continue;
        }

        // Check for std/alloc usage patterns on the non-comment content.
        if line_has_std_usage(stripped) {
            violations.push(Violation::StdUsage {
                file: file.to_path_buf(),
            });
        } else if line_has_alloc_usage(stripped) {
            violations.push(Violation::AllocUsage {
                file: file.to_path_buf(),
            });
        }
        i += 1;
    }
}

/// Returns the number of lines to advance from the `#[cfg(test)]` line
/// (inclusive) to skip the cfg-guarded block or item.
///
/// Strategy:
/// - If the `#[cfg(test)]` line itself contains `{`, start brace-match
///   from that line.
/// - Otherwise scan forward for the first non-blank, non-comment line.
///   If it contains `{`, start brace-match from that line.
///   If it does not, skip just that one item line (single-item form).
fn cfg_test_skip(lines: &[&str], cfg_idx: usize) -> usize {
    let cfg_line = lines[cfg_idx];
    let stripped_cfg = strip_line_comment(cfg_line).trim();

    // Case: `#[cfg(test)] mod tests { ... }` on one line (unusual but legal).
    if stripped_cfg.contains('{') {
        let end = brace_match_from_line(lines, cfg_idx);
        return end - cfg_idx + 1;
    }

    // Look ahead for the item that the attribute decorates.
    let mut j = cfg_idx + 1;
    while j < lines.len() {
        let item_stripped = strip_line_comment(lines[j]).trim();
        if item_stripped.is_empty() {
            j += 1;
            continue;
        }
        // Found the item line.
        if item_stripped.contains('{') {
            // Block item (module, impl, fn, …) — brace-match from here.
            let end = brace_match_from_line(lines, j);
            return end - cfg_idx + 1;
        }
        // Single-item (use, const, static, …) — skip just that one line.
        return j - cfg_idx + 1;
    }
    // Nothing after the attribute; advance one line (the attribute line itself).
    1
}

/// Returns the index of the line that closes the brace-block opened on or
/// after `start_idx`.  Uses bounded counting of `{` / `}` characters,
/// tolerating quoted strings by ignoring `{`/`}` inside `"..."`.
///
/// If the closing `}` is never found (malformed source), returns the last
/// line index (`lines.len() - 1`), capping the skip conservatively.
fn brace_match_from_line(lines: &[&str], start_idx: usize) -> usize {
    let mut depth = 0usize;
    let mut found_open = false;
    let mut i = start_idx;
    while i < lines.len() {
        let line = strip_line_comment(lines[i]);
        let mut in_str = false;
        for ch in line.chars() {
            match ch {
                '"' => in_str = !in_str,
                '{' if !in_str => {
                    depth += 1;
                    found_open = true;
                }
                '}' if !in_str => {
                    depth = depth.saturating_sub(1);
                    if found_open && depth == 0 {
                        return i;
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    // Guard: never found closing brace; return last line.
    lines.len().saturating_sub(1)
}

/// Returns `true` when the (comment-stripped, trimmed) line contains a
/// `std` usage pattern: `extern crate std` or `use std::`.
///
/// ```rust
/// use reovim_depgraph::line_has_std_usage;
///
/// assert!(line_has_std_usage("use std::collections::HashMap;"));
/// assert!(line_has_std_usage("extern crate std;"));
/// assert!(!line_has_std_usage("use alloc::vec::Vec;"));
/// // The input is already comment-stripped by the caller; a line that was
/// // `// use std::fmt;` arrives here as an empty string.
/// assert!(!line_has_std_usage(""));
/// ```
#[must_use]
pub fn line_has_std_usage(stripped: &str) -> bool {
    stripped.contains("extern crate std") || stripped.contains("use std::")
}

/// Returns `true` when the (comment-stripped, trimmed) line contains an
/// `alloc` usage pattern: `extern crate alloc` or `use alloc::`.
///
/// ```rust
/// use reovim_depgraph::line_has_alloc_usage;
///
/// assert!(line_has_alloc_usage("use alloc::vec::Vec;"));
/// assert!(line_has_alloc_usage("extern crate alloc;"));
/// assert!(!line_has_alloc_usage("use std::collections::HashMap;"));
/// ```
#[must_use]
pub fn line_has_alloc_usage(stripped: &str) -> bool {
    stripped.contains("extern crate alloc") || stripped.contains("use alloc::")
}

/// Strips the `// ...` line comment from a raw source line, respecting
/// `"..."` quoted strings (a `//` inside a string is not a comment).
///
/// This is the same algorithm as `toml::strip_comment` but operates on
/// Rust source (`//` delimiter instead of `#`).
///
/// ```rust
/// use reovim_depgraph::strip_line_comment;
///
/// assert_eq!(strip_line_comment("let x = 1; // comment"), "let x = 1; ");
/// // A `//` inside a quoted string is not a comment delimiter.
/// assert_eq!(strip_line_comment(r#"let s = "a//b";"#), r#"let s = "a//b";"#);
/// // Line with no comment is returned as-is.
/// assert_eq!(strip_line_comment("fn foo() {}"), "fn foo() {}");
/// ```
#[must_use]
pub fn strip_line_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => in_string = !in_string,
            b'\\' if in_string => i += 1,
            b'/' if !in_string && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                return &line[..i];
            }
            _ => {}
        }
        i += 1;
    }
    line
}

// ── L11 purity probe ─────────────────────────────────────────────────────────

/// Crate-root tokens that are always permitted in any `use`/`extern crate`
/// statement inside `uapi/protocol` source files (L11).
///
/// - `core` — the only standard library available under `#![no_std]`.
/// - `reovim_uapi_abi` — the one allowed external sibling dep.
/// - `crate` / `self` / `super` — intra-crate paths.
const L11_PERMITTED_IMPORT_ROOTS: &[&str] = &["core", "reovim_uapi_abi", "crate", "self", "super"];

/// Scans every `.rs` source file under `src_dir` (the `src/` directory of a
/// `uapi`-tier crate) for `use` or `extern crate` statements that reference
/// a crate root outside the L11 allowlist.
///
/// Returns a list of [`Violation::ForbiddenExternalImport`] entries.
///
/// `src_dir` is the physical `src/` path.  `tests/` sub-directories are
/// skipped (test code may link `std` under bootstrap state 1).
///
/// ```rust
/// use reovim_depgraph::run_l11_purity_probe;
/// use std::path::Path;
///
/// // An empty directory produces no violations.
/// let violations = run_l11_purity_probe(Path::new("/nonexistent")).unwrap_or_default();
/// assert!(violations.is_empty());
/// ```
///
/// # Errors
///
/// Returns [`ProbeError::Io`] when a directory entry cannot be listed.
pub fn run_l11_purity_probe(src_dir: &Path) -> Result<Vec<Violation>, ProbeError> {
    let mut violations = Vec::new();
    if src_dir.is_dir() {
        sweep_l11_src_dir(src_dir, &mut violations)?;
    }
    Ok(violations)
}

/// Recursively sweeps `.rs` files under `dir` (skipping `tests/` directories)
/// for imports forbidden by L11.
fn sweep_l11_src_dir(dir: &Path, violations: &mut Vec<Violation>) -> Result<(), ProbeError> {
    let entries: Vec<fs::DirEntry> = fs::read_dir(dir)
        .and_then(Iterator::collect)
        .map_err(io_error(dir))?;
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if path.is_dir() {
            if name_str == "tests" {
                continue; // skip test target directories
            }
            sweep_l11_src_dir(&path, violations)?;
        } else if path.extension().is_some_and(|e| e == "rs")
            && let Ok(text) = fs::read_to_string(&path)
        {
            check_source_for_l11_violations(&path, &text, violations);
        }
    }
    Ok(())
}

/// Scans `text` for `use X::` or `extern crate X` statements where `X` is not
/// in [`L11_PERMITTED_IMPORT_ROOTS`].
///
/// Line comments are stripped before matching.  `#[cfg(test)]`-guarded blocks
/// are skipped (test code may import `std`).
fn check_source_for_l11_violations(file: &Path, text: &str, violations: &mut Vec<Violation>) {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        let raw = lines[i];
        let stripped = strip_line_comment(raw).trim();

        // Skip cfg(test)-guarded blocks (same logic as DAG6).
        if stripped == "#[cfg(test)]" {
            let advance = cfg_test_skip(&lines, i);
            i += advance;
            continue;
        }

        // Match `use X::...` — extract the leading crate-root token `X`.
        if let Some(rest) = stripped.strip_prefix("use ") {
            // rest is e.g. "std::fmt::Write;" or "core::mem;" or "{…}"
            // Take the token up to the first `:` or `{` or `;` or space.
            let root = rest.split([':', '{', ';', ' ']).next().unwrap_or("");
            if !root.is_empty() && !L11_PERMITTED_IMPORT_ROOTS.contains(&root) {
                violations.push(Violation::ForbiddenExternalImport {
                    file: file.to_path_buf(),
                    import: root.to_owned(),
                });
            }
        } else if let Some(rest) = stripped.strip_prefix("extern crate ") {
            // rest is e.g. "std;" or "alloc;" or "arch as …;"
            let root = rest.split([';', ' ']).next().unwrap_or("");
            if !root.is_empty() && !L11_PERMITTED_IMPORT_ROOTS.contains(&root) {
                violations.push(Violation::ForbiddenExternalImport {
                    file: file.to_path_buf(),
                    import: root.to_owned(),
                });
            }
        }

        i += 1;
    }
}

// ── SP01 structural probes ────────────────────────────────────────────────────
//
// Four new probes encoding the Math/World airlock invariants that the coarse
// category matrix cannot express.  Each returns a Vec of human-readable
// violation strings (not `Violation` enum values — these probes operate at a
// different abstraction level from DAG1..DAG5).
//
// Design (Option A from SP01): the probes are standalone functions, not wired
// into `run_probe()`.  The category matrix stays coarse; the dedicated probes
// carry the airlock.  One mechanism per invariant, named honestly.

// ── helpers shared across SP01 probes ────────────────────────────────────────

/// Returns `true` when `path` (workspace-relative, `/`-separated) starts with
/// the given prefix component(s).  For example, `path_starts_with("editor/lib/core/foo", "editor")`
/// returns `true`.
///
/// This is a component-prefix check, not a byte-prefix check, so
/// `path_starts_with("editorx/foo", "editor")` returns `false`.
#[must_use]
fn path_starts_with(path: &str, prefix: &str) -> bool {
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

/// Returns `true` when `path` is a swap-set leaf — i.e. it lives inside one
/// of the named swap-set directories under any top-level kernel root.
///
/// Swap-set directories: `drivers`, `modules`, `providers`, `platforms`,
/// `capabilities`.  A crate at `editor/drivers/gpu` or `client/platforms/tui`
/// or `system/drivers/usb` is a swap-set leaf.
///
/// Note: the check is path-component-prefix only — the swap-set directory must
/// be the SECOND component of a workspace-relative path (e.g. `<root>/<set>/<leaf>`).
/// A crate at `editor/lib/core` is NOT a swap-set leaf by this rule (its second
/// component is `lib`, a kernel-internal path, not a swap-set dir); only
/// `editor/`, `client/`, or `system/` swap-sets count.
#[must_use]
fn is_swapset_leaf(path: &str) -> bool {
    // Accepted roots for swap-set leaves.
    const SWAPSET_ROOTS: &[&str] = &["editor", "client", "system"];
    // Named swap-set directories.
    const SWAPSET_DIRS: &[&str] = &[
        "drivers",
        "modules",
        "providers",
        "platforms",
        "capabilities",
    ];

    let comps: Vec<&str> = path.split('/').collect();
    // A swap-set leaf has at least three components: <root>/<set>/<leaf>.
    if comps.len() < 3 {
        return false;
    }
    SWAPSET_ROOTS.contains(&comps[0]) && SWAPSET_DIRS.contains(&comps[1])
}

/// Returns the "swap-set group key" for a swap-set leaf path: the first two
/// path components joined, e.g. `editor/drivers`.  Two crates in the same
/// group are siblings; two crates in different groups are not.
///
/// Returns `None` when `path` is not a swap-set leaf.
#[must_use]
fn swapset_group(path: &str) -> Option<String> {
    if !is_swapset_leaf(path) {
        return None;
    }
    let mut comps = path.splitn(3, '/');
    let root = comps.next()?;
    let dir = comps.next()?;
    Some(format!("{root}/{dir}"))
}

/// Returns `true` when `name` identifies a **floor-family** crate — a Tier-1
/// crate below the import airlock no product may name by import.
///
/// The forbidden direct-import family is: `reovim-arch` itself, the per-target
/// raw-mechanism crates carved out of it (`reovim-arch-sys-*`), the platform
/// providers (`reovim-platform-*`), the per-target language-floor lang-item
/// crates (`reovim-arch-floor-*`), and the World system-kernel bridge
/// (`reovim-system-kernel`). The firewall probe uses this to detect a DIRECT
/// product edge to any of them.
#[must_use]
fn is_floor_crate_name(name: &str) -> bool {
    // `reovim-arch` plus the per-target raw-mechanism crates carved out of it,
    // the platform providers above it, the per-target language floors, and the
    // system-kernel bridge. A product names its
    // uapi/kabi-facing contracts, never these lower implementation/bridge
    // crates. `reovim-arch-floor-*` is matched before the
    // `reovim-arch-sys-` prefix check would matter — the two prefixes are
    // disjoint, so order is moot.
    name == "reovim-arch"
        || name == "reovim-system-kernel"
        || name.starts_with("reovim-arch-sys-")
        || name.starts_with("reovim-arch-floor-")
        || name.starts_with("reovim-platform-")
}

// ── Probe 1: Direct-edge firewall ─────────────────────────────────────────────

/// Result type for SP01 structural probes: a list of human-readable violation
/// descriptions.  An empty vec means the probe passed.
///
/// ```rust
/// use reovim_depgraph::StructuralViolations;
///
/// let v: StructuralViolations = Vec::new();
/// assert!(v.is_empty());
/// ```
pub type StructuralViolations = Vec<String>;

/// **Firewall probe** — Architectural invariant 1 (master-plan §Architectural
/// Invariants, sub-plan SP01 Phase 2).
///
/// Asserts that no crate classified under `editor/**` or `client/**` has a
/// DIRECT Cargo dependency edge naming an `arch`, `system/kernel`, or
/// `*/drivers` crate.
///
/// The invariant is stated at the strength it actually holds: *direct* edge
/// only.  Transitive World-fulness via `lib/ds → arch` is allowed — that path
/// runs through the `kabi` contract handle, not through a direct kernel → arch
/// edge.
///
/// # Errors
///
/// Returns `ProbeError` when workspace enumeration fails.
///
/// ```rust
/// use reovim_depgraph::run_firewall_probe;
/// use std::path::Path;
///
/// // A non-existent root returns Err (Io).
/// let result = run_firewall_probe(Path::new("/nonexistent/xyz"));
/// assert!(result.is_err());
/// ```
pub fn run_firewall_probe(root: &Path) -> Result<StructuralViolations, ProbeError> {
    let crates = enumerate_crates(root)?;

    // Build a path index: crate name → workspace-relative path.
    let name_to_path: BTreeMap<&str, &str> = crates
        .iter()
        .map(|c| (c.name.as_str(), c.path.as_str()))
        .collect();

    let mut violations = Vec::new();

    for krate in &crates {
        // The firewall applies to Math kernels: editor/** and client/**.
        if !path_starts_with(&krate.path, "editor") && !path_starts_with(&krate.path, "client") {
            continue;
        }

        for dep in &krate.deps {
            // Only inspect direct path deps — sovereign in-repo edges only.
            // (A non-path dep would already be a DAG5 violation.)
            if !dep.is_path {
                continue;
            }
            // Optional deps are not product edges: they compile in only when a
            // feature activates `dep:name`, and a product build that omits that
            // feature would fail to compile if the dep were genuinely needed.
            // A selftest-gated optional arch dep is the
            // test-infra edge, not the product floor — skip it, but log an
            // audit line per skipped arch dep so the suppression is visible
            // rather than silent (analogous to the structural probe's
            // allowlist reporting).
            if dep.optional {
                if is_floor_crate_name(&dep.name) {
                    eprintln!(
                        "firewall: skipped optional floor dep `{}` in `{}` ({}) — \
                         optional (selftest-gated) edge, not a product edge",
                        dep.name, krate.name, krate.path
                    );
                }
                continue;
            }
            let dep_path = name_to_path.get(dep.name.as_str()).copied().unwrap_or("");

            // Violation cases:
            // (a) direct dep on any floor-family crate (arch / arch-sys-* /
            //     platform-*).
            let is_arch_dep = is_floor_crate_name(&dep.name);
            // (b) direct dep on system/kernel (World sovereign kernel).
            //     `path_starts_with` already matches the exact path component-wise.
            let is_sys_kernel = path_starts_with(dep_path, "system/kernel");
            // (c) direct dep on any */drivers/* crate (swap-set impl leaf).
            //     Applies to drivers under editor/, client/, system/, ext/.
            let is_drivers_dep = dep_path.split('/').any(|component| component == "drivers");

            if is_arch_dep || is_sys_kernel || is_drivers_dep {
                violations.push(format!(
                    "firewall: `{}` ({}) has a direct dep on `{}` ({}) — \
                     Math kernel must not name arch/system-kernel/drivers directly; \
                     reach World services through uapi/system bridge surfaces",
                    krate.name, krate.path, dep.name, dep_path
                ));
            }
        }
    }

    Ok(violations)
}

// ── Probe 1b: no product `arch::net`/`arch::thread` ───────────────────────────

/// **No-product-arch-net probe.**
///
/// Asserts that the named crates' PRODUCT source (every `.rs` file under `src/`
/// that is NOT an L12 `*_tests.rs` sibling, with `#[cfg(test)]` and
/// `#[cfg(feature = "selftest")]` blocks skipped) names no `arch::net`,
/// `arch::thread`, `reovim_arch::net`, or `reovim_arch::thread`. The
/// server-rt + tui net/thread transport flows through the `kabi` handle (via the
/// `lib/ds` wrappers), so a product reference to those arch paths would be a
/// firewall bypass the manifest-level firewall probe cannot see (it inspects
/// Cargo edges, not source).
///
/// `crate_names` is the set the caller wants checked (server-rt's product code
/// is fully off `arch::net`/`arch::thread`; tui keeps arch for panic/sys/term but its
/// PRODUCT net use is gone). The L12 `*_tests.rs` files and selftest-gated
/// blocks are skipped because the test-infra legitimately binds arch sockets to
/// stand up a fake server.
///
/// # Errors
///
/// Returns `ProbeError` when workspace enumeration or filesystem I/O fails.
///
/// ```rust
/// use reovim_depgraph::run_no_product_arch_net_probe;
/// use std::path::Path;
///
/// // A non-existent root returns Err (Io).
/// let result = run_no_product_arch_net_probe(Path::new("/nonexistent/xyz"), &[]);
/// assert!(result.is_err());
/// ```
pub fn run_no_product_arch_net_probe(
    root: &Path,
    crate_names: &[&str],
) -> Result<StructuralViolations, ProbeError> {
    let crates = enumerate_crates(root)?;
    let mut violations = Vec::new();

    for krate in &crates {
        if !crate_names.contains(&krate.name.as_str()) {
            continue;
        }
        let src_dir = root.join(&krate.path).join("src");
        if src_dir.is_dir() {
            sweep_product_src_for_arch_net(&src_dir, &krate.name, &mut violations)?;
        }
    }

    Ok(violations)
}

/// Recursively sweeps product `.rs` files under `dir` for `arch::net`/
/// `arch::thread` references, skipping L12 `*_tests.rs` siblings and `tests`
/// directories.
fn sweep_product_src_for_arch_net(
    dir: &Path,
    crate_name: &str,
    violations: &mut Vec<String>,
) -> Result<(), ProbeError> {
    let entries: Vec<fs::DirEntry> = fs::read_dir(dir)
        .and_then(Iterator::collect)
        .map_err(io_error(dir))?;
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if path.is_dir() {
            if name_str == "tests" {
                continue; // skip test target directories
            }
            sweep_product_src_for_arch_net(&path, crate_name, violations)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            // L12 sibling test files compile only under `selftest`; the fake-
            // server test-infra legitimately uses arch sockets, so skip them.
            if name_str.ends_with("_tests.rs") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue; // unreadable files: silently skip
            };
            check_product_source_for_arch_net(&path, &text, crate_name, violations);
        }
    }
    Ok(())
}

/// Scans `text` (a product source file) for `arch::net`/`arch::thread` path
/// references outside `#[cfg(test)]` and `#[cfg(feature = "selftest")]` blocks.
fn check_product_source_for_arch_net(
    file: &Path,
    text: &str,
    crate_name: &str,
    violations: &mut Vec<String>,
) {
    /// The product-forbidden arch transport paths.
    const FORBIDDEN: &[&str] = &[
        "arch::net",
        "arch::thread",
        "reovim_arch::net",
        "reovim_arch::thread",
    ];

    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        let stripped = strip_line_comment(lines[i]).trim();

        // Skip a `#[cfg(test)]` or `#[cfg(feature = "selftest")]` block: the
        // selftest-gated test-infra may name arch::net/thread legitimately.
        if stripped == "#[cfg(test)]" || is_selftest_cfg(stripped) {
            i += cfg_test_skip(&lines, i);
            continue;
        }

        for pat in FORBIDDEN {
            if stripped.contains(pat) {
                violations.push(format!(
                    "no-product-arch-net: `{}` `{}` line {}: product code names `{}` — \
                     net/thread must route through injected uapi/system services, not arch",
                    crate_name,
                    file.display(),
                    i + 1,
                    pat,
                ));
            }
        }

        i += 1;
    }
}

/// Returns `true` when `stripped` is a `#[cfg(feature = "selftest")]` attribute
/// (tolerating single/double quotes and inner spacing).
fn is_selftest_cfg(stripped: &str) -> bool {
    stripped.starts_with("#[cfg(") && stripped.contains("feature") && stripped.contains("selftest")
}

// ── Probe 1c: no product `arch::time`/`arch::sys` time+sys-call surface ────────

/// **No-product-arch-time-sys probe.**
///
/// Source-level companion to [`run_no_product_arch_net_probe`]: asserts that
/// the named crates' PRODUCT source (every `.rs` file under `src/` that is NOT
/// an L12 `*_tests.rs` sibling, with `#[cfg(test)]` and
/// `#[cfg(feature = "selftest")]` blocks skipped) names no `arch::time` and
/// none of the `arch::sys` TIME/FILE/THREAD-IDENTITY syscall surface — the
/// kernel's clock + log-sink + service-registration and the tui's stdio route
/// those through the `kabi` handle (via the `lib/ds` `time`/`fs`/`thread`
/// wrappers), so a product reference to those arch paths would be a firewall
/// bypass the manifest-level probe cannot see.
///
/// ## What is and is not forbidden (and why)
///
/// Forbidden: `arch::time` (the whole monotonic + wall-clock module) and the
/// `arch::sys` CALL names moved behind the handle —
/// `openat`, `gettid`, `read`, `write`, `close` (plus the `reovim_arch::`
/// path forms). These are matched as the qualified call names
/// (`arch::sys::openat`, …), NOT as the bare `arch::sys::` prefix.
///
/// Deliberately ALLOWED: `arch::sys::ioctl` and the `arch::sys::term::*`
/// termios surface are not matched by this probe because it scans product code
/// for the specific moved call names, not the whole `arch::sys::` prefix. The
/// old provider-side arch facade consumer has been retired; the remaining
/// allowance only avoids over-matching unrelated text while the product
/// firewall focuses on the moved calls.
///
/// # Errors
///
/// Returns `ProbeError` when workspace enumeration or filesystem I/O fails.
///
/// ```rust
/// use reovim_depgraph::run_no_product_arch_time_sys_probe;
/// use std::path::Path;
///
/// // A non-existent root returns Err (Io).
/// let result = run_no_product_arch_time_sys_probe(Path::new("/nonexistent/xyz"), &[]);
/// assert!(result.is_err());
/// ```
pub fn run_no_product_arch_time_sys_probe(
    root: &Path,
    crate_names: &[&str],
) -> Result<StructuralViolations, ProbeError> {
    let crates = enumerate_crates(root)?;
    let mut violations = Vec::new();

    for krate in &crates {
        if !crate_names.contains(&krate.name.as_str()) {
            continue;
        }
        let src_dir = root.join(&krate.path).join("src");
        if src_dir.is_dir() {
            sweep_product_src_for_arch_time_sys(&src_dir, &krate.name, &mut violations)?;
        }
    }

    Ok(violations)
}

/// Recursively sweeps product `.rs` files under `dir` for the forbidden
/// `arch::time`/`arch::sys` time+sys-call references, skipping L12 `*_tests.rs`
/// siblings and `tests` directories.
fn sweep_product_src_for_arch_time_sys(
    dir: &Path,
    crate_name: &str,
    violations: &mut Vec<String>,
) -> Result<(), ProbeError> {
    let entries: Vec<fs::DirEntry> = fs::read_dir(dir)
        .and_then(Iterator::collect)
        .map_err(io_error(dir))?;
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if path.is_dir() {
            if name_str == "tests" {
                continue; // skip test target directories
            }
            sweep_product_src_for_arch_time_sys(&path, crate_name, violations)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            // L12 sibling test files compile only under `selftest`; the
            // selftest test-infra legitimately uses arch time/sys calls.
            if name_str.ends_with("_tests.rs") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue; // unreadable files: silently skip
            };
            check_product_source_for_arch_time_sys(&path, &text, crate_name, violations);
        }
    }
    Ok(())
}

/// Scans `text` (a product source file) for `arch::time`/`arch::sys` time+sys
/// call references outside `#[cfg(test)]` and `#[cfg(feature = "selftest")]`
/// blocks, ALLOWING the termios `arch::sys::ioctl`/`arch::sys::term` surface
/// (the transitional provider backend edge, retired later; no swept product names it).
fn check_product_source_for_arch_time_sys(
    file: &Path,
    text: &str,
    crate_name: &str,
    violations: &mut Vec<String>,
) {
    /// The product-forbidden arch time + sys-call paths. The whole `arch::time`
    /// module, and the specific `arch::sys` CALL names moved behind the
    /// handle — NOT the bare `arch::sys::` prefix, which would over-match
    /// unrelated or historical termios text.
    const FORBIDDEN: &[&str] = &[
        "arch::time",
        "reovim_arch::time",
        "arch::sys::openat",
        "reovim_arch::sys::openat",
        "arch::sys::gettid",
        "reovim_arch::sys::gettid",
        "arch::sys::read",
        "reovim_arch::sys::read",
        "arch::sys::write",
        "reovim_arch::sys::write",
        "arch::sys::close",
        "reovim_arch::sys::close",
    ];

    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        let stripped = strip_line_comment(lines[i]).trim();

        // Skip a `#[cfg(test)]` or `#[cfg(feature = "selftest")]` block: the
        // selftest-gated test-infra may name arch time/sys calls legitimately.
        if stripped == "#[cfg(test)]" || is_selftest_cfg(stripped) {
            i += cfg_test_skip(&lines, i);
            continue;
        }

        for pat in FORBIDDEN {
            if stripped.contains(pat) {
                violations.push(format!(
                    "no-product-arch-time-sys: `{}` `{}` line {}: product code names `{}` — \
                     time/file/thread-identity must route through injected uapi/system services, \
                     not arch",
                    crate_name,
                    file.display(),
                    i + 1,
                    pat,
                ));
            }
        }

        i += 1;
    }
}

// ── Probe 2: lib/ds ⊄ arch ────────────────────────────────────────────────────

/// **`lib/ds` purity probe** — Architectural invariant 2.
///
/// Asserts two things about `lib/ds` crate(s):
///
/// 1. No `lib/ds` crate lists an `arch` crate in any Cargo dependency table
///    (Cargo-manifest check).
/// 2. No `lib/ds` source file contains `use arch::` or `arch::` path references
///    (source-grep, mirroring the L11 approach in `uapi_purity.rs`).
///
/// `lib/ds` reaches allocator/park primitives through injected uapi control
/// tables — never by naming `arch`, a provider, or a `kabi` handle directly.
///
/// # Errors
///
/// Returns `ProbeError` when workspace enumeration or filesystem I/O fails.
///
/// ```rust
/// use reovim_depgraph::run_lib_ds_purity_probe;
/// use std::path::Path;
///
/// // A non-existent root returns Err (Io).
/// let result = run_lib_ds_purity_probe(Path::new("/nonexistent/xyz"));
/// assert!(result.is_err());
/// ```
pub fn run_lib_ds_purity_probe(root: &Path) -> Result<StructuralViolations, ProbeError> {
    let crates = enumerate_crates(root)?;
    let mut violations = Vec::new();

    for krate in &crates {
        // Only inspect crates under lib/ds (or lib/ds/* if it nests).
        if !path_starts_with(&krate.path, "lib/ds") {
            continue;
        }

        // Manifest check: no dep in any table may name a floor-family crate
        // (arch / arch-sys-* / platform-*) — lib/ds reaches the floor only
        // through injected controls.
        for dep in &krate.deps {
            if is_floor_crate_name(&dep.name) {
                violations.push(format!(
                    "lib-ds-purity: `{}` [{}] names floor crate `{}` — \
                     lib/ds must reach floor primitives only through injected controls, never directly",
                    krate.name, dep.table, dep.name
                ));
            }
        }

        // Source-grep: no `use arch::` or standalone `arch::` path usage.
        let src_dir = root.join(&krate.path).join("src");
        if src_dir.is_dir() {
            sweep_lib_ds_src(&src_dir, &krate.name, &mut violations)?;
        }
    }

    Ok(violations)
}

/// Recursively sweeps `.rs` files under `dir` for `arch::` path references,
/// appending violations for each hit outside `#[cfg(test)]` blocks.
fn sweep_lib_ds_src(
    dir: &Path,
    crate_name: &str,
    violations: &mut Vec<String>,
) -> Result<(), ProbeError> {
    let entries: Vec<fs::DirEntry> = fs::read_dir(dir)
        .and_then(Iterator::collect)
        .map_err(io_error(dir))?;
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if path.is_dir() {
            if name_str == "tests" {
                continue; // skip test target directories
            }
            sweep_lib_ds_src(&path, crate_name, violations)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let Ok(text) = fs::read_to_string(&path) else {
                continue; // unreadable files: silently skip
            };
            check_lib_ds_source_for_arch(&path, &text, crate_name, violations);
        }
    }
    Ok(())
}

/// Scans `text` (a source file in `lib/ds`) for `arch::` path references
/// outside `#[cfg(test)]` blocks.
///
/// Matches:
/// - `use arch::…` — direct import from arch
/// - `arch::` anywhere else on the line (e.g. `arch::syscall::write(…)`)
fn check_lib_ds_source_for_arch(
    file: &Path,
    text: &str,
    crate_name: &str,
    violations: &mut Vec<String>,
) {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        let raw = lines[i];
        let stripped = strip_line_comment(raw).trim();

        if stripped == "#[cfg(test)]" {
            let advance = cfg_test_skip(&lines, i);
            i += advance;
            continue;
        }

        // Check for any `arch::` token (catches `use arch::`, `arch::alloc(…)`, etc.)
        if stripped.contains("arch::") {
            violations.push(format!(
                "lib-ds-purity: `{}` `{}` line {}: contains `arch::` reference — \
                 lib/ds must not name arch primitives directly",
                crate_name,
                file.display(),
                i + 1,
            ));
        }

        i += 1;
    }
}

// ── Probe 3: Swap-set isolation ───────────────────────────────────────────────

/// **Swap-set isolation probe** — Architectural invariant 4 (no impl→impl).
///
/// Asserts that no Cargo edge exists between two swap-set leaves where the two
/// leaves belong to DIFFERENT swap-set groups.  A swap-set leaf is a crate
/// whose workspace path matches `<root>/<set>/<leaf>` where `<root>` is one of
/// `editor`, `client`, `system` and `<set>` is one of `drivers`, `modules`,
/// `providers`, `platforms`, `capabilities`.
///
/// Two leaves in the SAME group (e.g. both under `editor/drivers/`) are not
/// cross-group and are not flagged.  Two leaves in DIFFERENT groups (e.g.
/// `editor/drivers/gpu` and `client/platforms/tui`) are flagged if a direct
/// Cargo edge exists.
///
/// This encodes the nouveau-vs-nvidia isolation: no impl depends on a sibling
/// impl.  Composition is the responsibility of `apps/*` composition roots.
///
/// # Errors
///
/// Returns `ProbeError` when workspace enumeration fails.
///
/// ```rust
/// use reovim_depgraph::run_swapset_isolation_probe;
/// use std::path::Path;
///
/// // A non-existent root returns Err (Io).
/// let result = run_swapset_isolation_probe(Path::new("/nonexistent/xyz"));
/// assert!(result.is_err());
/// ```
pub fn run_swapset_isolation_probe(root: &Path) -> Result<StructuralViolations, ProbeError> {
    let crates = enumerate_crates(root)?;

    // Build a path index: crate name → workspace-relative path.
    let name_to_path: BTreeMap<&str, &str> = crates
        .iter()
        .map(|c| (c.name.as_str(), c.path.as_str()))
        .collect();

    let mut violations = Vec::new();

    for krate in &crates {
        let Some(from_group) = swapset_group(&krate.path) else {
            continue; // not a swap-set leaf
        };

        for dep in &krate.deps {
            if !dep.is_path {
                continue; // non-path deps are a DAG5 concern, not ours
            }
            let dep_path = name_to_path.get(dep.name.as_str()).copied().unwrap_or("");
            let Some(to_group) = swapset_group(dep_path) else {
                continue; // dep is not a swap-set leaf
            };

            if from_group != to_group {
                // Two different swap-set leaves cross-depending: violation.
                violations.push(format!(
                    "swapset-isolation: `{}` ({}, group `{}`) has a direct dep on \
                     `{}` ({}, group `{}`) — swap-set leaves must not depend on \
                     each other across groups; compose only in apps/*",
                    krate.name, krate.path, from_group, dep.name, dep_path, to_group,
                ));
            }
        }
    }

    Ok(violations)
}

// ── Probe 4: Mode/provider selector manifest-grep ────────────────────────────

/// The feature name that marks provider-presence selection.
///
/// The provider-presence Cargo feature is optional today: current composition
/// roots bind providers through direct dependencies, so count 0 is legal. If a
/// feature selector is introduced, it must appear in AT MOST ONE `apps/*`
/// manifest and in NO non-`apps/*` manifest.
///
/// This constant gives the probe a stable search token for that selector.
///
/// ```rust
/// use reovim_depgraph::PROVIDER_PRESENCE_FEATURE;
///
/// assert!(!PROVIDER_PRESENCE_FEATURE.is_empty());
/// ```
pub const PROVIDER_PRESENCE_FEATURE: &str = "platform-provider";

/// **Mode/provider selector probe** — Architectural invariant 5.
///
/// Asserts via manifest-grep that the provider-presence Cargo feature
/// (`PROVIDER_PRESENCE_FEATURE`) appears in:
/// - AT MOST ONE `apps/*` manifest  (count 0 legal while direct deps bind providers)
/// - NO non-`apps/*` manifest
///
/// The DAG edge-walker cannot see feature *definitions* inside `[features]`
/// tables (it only sees dep edges), so this probe reads manifests directly.
///
/// # Errors
///
/// Returns `ProbeError` when workspace enumeration or manifest I/O fails.
///
/// ```rust
/// use reovim_depgraph::run_mode_selector_probe;
/// use std::path::Path;
///
/// // A non-existent root returns Err (Io).
/// let result = run_mode_selector_probe(Path::new("/nonexistent/xyz"));
/// assert!(result.is_err());
/// ```
pub fn run_mode_selector_probe(root: &Path) -> Result<StructuralViolations, ProbeError> {
    let crates = enumerate_crates(root)?;
    let mut violations = Vec::new();
    let mut apps_count = 0usize;

    for krate in &crates {
        let manifest_path = root.join(&krate.path).join("Cargo.toml");
        let text = match fs::read_to_string(&manifest_path) {
            Ok(t) => t,
            Err(source) => {
                return Err(ProbeError::Io {
                    path: manifest_path,
                    source,
                });
            }
        };

        // Grep for the provider-presence feature name in the [features] section.
        // We scan all lines of the manifest for the feature token.
        // A line that contains `PROVIDER_PRESENCE_FEATURE` and is not a comment
        // is treated as a declaration of the feature.
        let has_feature = manifest_contains_feature(&text, PROVIDER_PRESENCE_FEATURE);

        if !has_feature {
            continue;
        }

        let is_apps = path_starts_with(&krate.path, "apps");

        if is_apps {
            apps_count += 1;
            if apps_count > 1 {
                violations.push(format!(
                    "mode-selector: `{}` ({}) declares feature `{}` — \
                     the provider-presence feature must appear in AT MOST ONE `apps/*` manifest \
                     (found it in multiple apps crates)",
                    krate.name, krate.path, PROVIDER_PRESENCE_FEATURE
                ));
            }
        } else {
            violations.push(format!(
                "mode-selector: `{}` ({}) declares feature `{}` — \
                 the provider-presence feature must ONLY appear in `apps/*` manifests, \
                 never outside",
                krate.name, krate.path, PROVIDER_PRESENCE_FEATURE
            ));
        }
    }

    Ok(violations)
}

/// Returns `true` when `manifest_text` (a `Cargo.toml` file) appears to
/// declare `feature_name` in a `[features]` section.
///
/// The scan is intentionally conservative: it looks for the feature name as a
/// key on its own line inside a `[features]` block, stripping TOML line
/// comments via `toml::strip_comment`.  False negatives are possible for
/// unusual TOML layouts (multi-line feature definitions); false positives are
/// unlikely given the feature name is project-specific.
#[must_use]
fn manifest_contains_feature(manifest_text: &str, feature_name: &str) -> bool {
    let mut in_features = false;
    for raw_line in manifest_text.lines() {
        let line = toml::strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            // Track whether we are inside a [features] section.
            // Matches `[features]` exactly; `[features.*]` (e.g. future inline
            // sub-tables) would be an unusual TOML shape and is not supported.
            in_features = line == "[features]";
            continue;
        }
        if !in_features {
            continue;
        }
        // Inside [features]: check whether this line declares our feature name.
        // Canonical form: `feature-name = []` or `feature-name = ["dep"]`.
        // We test that the trimmed line starts with the feature name followed
        // immediately (after optional whitespace) by `=`.
        if let Some(stripped) = line.strip_prefix(feature_name)
            && stripped.trim_start().starts_with('=')
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests;
