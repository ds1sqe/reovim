# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Goals

- **Fastest-reaction editor**: Prioritize minimal latency and instant response to user input
- **Scalability**: Architecture designed to scale with large files and complex operations

## Architecture

Reovim is a Rust-based neovim-like text editor following a **Linux kernel-inspired architecture** with clear separation between kernel mechanisms, drivers, and loadable modules.

### Linux/Unix Philosophy

| Principle | Application in Reovim |
|-----------|----------------------|
| **Mechanism vs Policy** | Kernel provides WHAT (traits, syscalls), modules decide HOW (keybindings, layout) |
| **Do one thing well** | Each component has single responsibility |
| **Composability** | Small, focused modules that combine |
| **Separation of concerns** | Clear boundaries: kernel → drivers → modules |
| **API purity** | Kernel has zero external syntax dependencies (tree-sitter in drivers) |
| **Simplicity** | No over-engineering, minimal complexity |

### Layer Model

```
┌─────────────────────────────────────────────────────────┐
│  CLIENTS (clients/)                         APPLICATION │
│  tui/, cli/                                             │
│  → User-facing applications (gRPC v2 protocol)          │
├─────────────────────────────────────────────────────────┤
│  MODULES (server/modules/)                  POLICY      │
│  vim/, keymap/, motions/, textobjects/, editor/         │
│  → Decide HOW things behave                             │
├─────────────────────────────────────────────────────────┤
│  DRIVERS (server/lib/drivers/)              MECHANISM   │
│  input/, syntax/, lsp/, vfs/, session/, buffer/         │
│  → Provide services, define trait contracts             │
├─────────────────────────────────────────────────────────┤
│  KERNEL (server/lib/kernel/)                MECHANISM   │
│  mm/, ipc/, core/, block/, sched/, api/                 │
│  → Core primitives, WHAT can be done                    │
└─────────────────────────────────────────────────────────┘
```

### Design Rules

- **Kernel Purity**: Zero external syntax dependencies in kernel. All tree-sitter lives in drivers.
- **API Boundary**: Modules use ONLY `reovim_kernel::api::*`. Kernel internals are `pub(crate)` (compile-time enforced).
- **Trait Contracts**: Drivers define traits (e.g., `LayoutPolicy`), modules implement them (e.g., `TilingLayout`).

### Session Model

Reovim uses a **tmux-like session model**:

- **Session** (`SessionId(Arc<str>)`) - Named editing context, shared buffer content and kernel
- **Client** (`ClientId(usize)`) - Connection to server, independent mode, cursor, active buffer, selection, registers, viewport

Multiple clients can attach to one session, sharing buffer content but with
independent modes, cursors, active buffer views, and viewports. See `docs/architecture/session-model.md`.

Key types (all use `usize` storage, `as_usize()` accessor):
- `SessionId(Arc<str>)` - Named session in runner
- `ClientId(usize)` - Client connection in driver
- `BufferId(usize)` - Buffer identifier in kernel
- `WindowId(usize)` - Window identifier in kernel

## Code Standards

### Zero-Warning Policy

This project enforces a **zero-warning policy** via `Cargo.toml` workspace lints:

```toml
[workspace.lints.rust]
warnings = "deny"    # All warnings become errors

[workspace.lints.clippy]
all = "deny"
pedantic = "deny"
nursery = "deny"
```

- **All warnings are compile errors** - code won't build with warnings
- `#[allow(...)]` - escape-hatch only with justification, try not to use

### Coverage Policy

**100% code coverage** enforced in CI via Codecov. PRs that regress coverage are blocked.

- All non-server crates: 100% MC/DC coverage (line + condition)
- Server crate (`reovim-server`): 100% line coverage (MC/DC excluded due to LLVM bug #119558)
- Every new function must have test coverage for all code paths
- Use `#[cfg_attr(coverage_nightly, coverage(off))]` only for genuinely untestable code (tracing closures, `tokio::spawn` bodies, async channel errors, OnceLock globals)
- Coverage commands: `./scripts/coverage.sh [line|mcdc|server] [--lcov]`

### Kernel Purity Policy

**NEVER add plugin/module-specific code to kernel.** Kernel must remain policy-agnostic.

**If the current API doesn't support your needs:**
1. **Propose an API extension** - Design a general-purpose API that solves the problem
2. **Document the proposal** in `tmp/<feature>-api-proposal.md`
3. **Get approval** before implementing

**Correct approach:**
- Modules/plugins define their own events and types
- Use generic kernel APIs, extend only when multiple modules benefit
- Policy belongs in `server/modules/`, mechanism belongs in `server/lib/kernel/` or `server/lib/drivers/`

### Process Safety Policy

**NEVER kill other reovim instances.** Multiple servers can run concurrently.
Forbidden: `pkill reovim`, `killall reovim`, or any command that kills processes you didn't start.
Always check `ps aux | grep reovim` before starting, track your PIDs, kill only YOUR server.

## Build Commands

```bash
# Essential commands
cargo build              # Build all crates
cargo build --release    # Build release
cargo run                # Run (starts server + TUI)
cargo test               # Run tests
cargo check              # Check without building
cargo fmt                # Format code
cargo clippy             # Run linter

# Run tests for a specific crate
cargo test -p reovim-kernel

# Pre-commit check script
./scripts/check.sh                # Full parallel check (fmt + clippy || tests)
./scripts/check.sh --quick        # Format + clippy only (fast dev iteration)
./scripts/check.sh --sequential   # Sequential execution (debugging / low-memory)
./scripts/check.sh --clean-cache  # Remove clippy build cache (target/check-clippy)

# Generate performance report
cargo run -p perf-report -- bench -v X.Y.Z
```

For detailed server, CLI, and TUI commands, see [Server Mode](docs/user-guide/server-mode.md) and [CLI Reference](docs/user-guide/cli-reference.md).

## Git Commits

**IMPORTANT**: Claude should NEVER handle git commits. The user will manage ALL git operations. Claude must not have any "opinion" about when or how to commit - only create proposals and let the user decide.

**MANDATORY: Issue Tracking**
- **ALL code work MUST have a GitHub issue for tracking**
- **REFUSE to start ANY implementation without an issue number**
- **REFUSE any git operations (commit, push, PR) without an associated issue number**
- **PRs without an associated issue will be REJECTED**
- If user requests a feature/fix without an issue → ask to create one first
- If no issue exists, create one with `gh issue create` before proceeding
- Reference the issue in commit messages: `type(scope): description (#ISSUE_NUMBER)`

**Forbidden in commits/docs:**
- Emoji
- `Co-Authored-By: Claude ... <noreply@anthropic.com>`
- Any Claude/Anthropic advertisement or promotional content

**Commit Message Format:**
```
type(scope): short description (#ISSUE_NUMBER)

Detailed body explaining what and why.

- Bullet points for key changes
- Can be multi-line

Closes: #ISSUE_SOLVED, Fixs: #BUG_ISSUE, Refs #ISSUE_NUMBER, Parts of #EPIC_NUMBER
```

- **Header**: `type(scope): description (#NUM)` - issue number in parentheses
- **Body**: What changed and why, bullet points for clarity
- **Footer**: `Refs #NUM` for the issue, `Parts of #NUM` for parent epic
- **Types**: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`
- **IMPORTANT**: Lines starting with `#` are comments in git and will be stripped. Use plain text headers (e.g., `Section:`) instead of markdown `## Section`.

**Changelog Convention**: Every feature, fix, or notable change MUST include an update to `CHANGELOG.md` under the `[Unreleased]` section before proposing a commit.

### Pull Requests

When creating PRs with `gh pr create`, do NOT add promotional footers like "Generated with Claude Code" or similar. Keep the PR body clean and professional with only relevant technical content.

## Workflows

Use `/countdown` before implementation and `/final-approach` before landing.
Use `/stop` before ending any work session.
Use `/rebase [target]` for rebasing.
See `.claude/skills/` for full procedures, `.claude/rules/` for extended project rules.

## Detailed Documentation

- [docs/architecture/](./docs/architecture/) - Layer diagram, kernel, drivers, modules, mechanism-policy
- [docs/contributing/](./docs/contributing/) - Getting started, testing guide, philosophy
- [docs/user-guide/](./docs/user-guide/) - Configuration, commands, server mode, frame capture
- [docs/heritage/](./docs/heritage/) - Project history, legacy patterns, original proposal
- [archive/](./archive/) - v0.8.x legacy code (reference only)
