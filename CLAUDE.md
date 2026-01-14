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
│  MODULES (modules/)                         POLICY      │
│  Keymap, Motions, Operators, Layout, Options            │
│  → Decide HOW things behave                             │
├─────────────────────────────────────────────────────────┤
│  DRIVERS (lib/drivers/)                     MECHANISM   │
│  syntax/, input/, display/, lsp/, net/, vfs/            │
│  → Provide services, define trait contracts             │
├─────────────────────────────────────────────────────────┤
│  KERNEL (lib/kernel/)                       MECHANISM   │
│  mm/, ipc/, core/, block/, sched/, api/                 │
│  → Core primitives, WHAT can be done                    │
└─────────────────────────────────────────────────────────┘
```

### Design Rules

- **Kernel Purity**: Zero external syntax dependencies in kernel. All tree-sitter lives in drivers.
- **API Boundary**: Modules use ONLY `reovim_kernel::api::*`. Kernel internals are `pub(crate)` (compile-time enforced).
- **Trait Contracts**: Drivers define traits (e.g., `LayoutPolicy`), modules implement them (e.g., `TilingLayout`).

### Workspace Structure

**Kernel Layer:**
- `lib/kernel/` (reovim-kernel) - Core mechanisms: mm/, ipc/, core/, block/, sched/, api/

**Driver Layer:**
- `lib/drivers/command/` - Command trait and registry
- `lib/drivers/display/` - Terminal rendering, style, decorations
- `lib/drivers/input/` - Key events and input parsing
- `lib/drivers/syntax/` - Tree-sitter integration
- `lib/drivers/lsp/` - Language server protocol client
- `lib/drivers/net/` - Network transport (TCP, Unix socket)
- `lib/drivers/vfs/` - Virtual filesystem operations
- `lib/drivers/log/` - Logging infrastructure

**Platform Abstraction:**
- `lib/arch/` (reovim-arch) - Platform abstraction: unix/, windows/
- `lib/module-macros/` - `declare_module!` proc-macro for FFI entry points

**Runner:**
- `runner/` (reovim) - Event loop, registries, application state

**Policy Modules:**
- `modules/` - Policy modules (editor, keymap, operators, layout, options, etc.)

**Tools:**
- `tools/perf-report/` - Performance report generator CLI
- `tools/reo-cli/` - CLI client for server mode
- `perf/` - Versioned performance reports (PERF-{version}.md)

**Archive (legacy reference only):**
- `archive/` - Pre-v0.9.0 legacy code (lib/core, lib/sys, lib/lsp, plugins, old runner)

### Key Dependencies

- `tokio` - Async runtime
- `crossterm` (via reovim-arch) - Terminal I/O
- `futures`/`futures-timer` - Async utilities
- `tracing` - Logging and diagnostics

### Minimum Rust Version

1.92 (Rust 2024 edition)

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

### Kernel Purity Policy

**NEVER add plugin/module-specific code to kernel.** Kernel must remain policy-agnostic.

**If the current API doesn't support your needs:**
1. **Propose an API extension** - Design a general-purpose API that solves the problem
2. **Document the proposal** in `tmp/<feature>-api-proposal.md`
3. **Get approval** before implementing

**Correct approach:**
- Modules/plugins define their own events and types
- Use generic kernel APIs, extend only when multiple modules benefit
- Policy belongs in `modules/`, mechanism belongs in `lib/kernel/` or `lib/drivers/`

### Process Safety Policy

**NEVER kill other reovim instances.** Multiple reovim servers can run concurrently for debugging.

**Forbidden commands:**
- `pkill reovim` or `pkill -f reovim`
- `killall reovim`
- Any command that terminates reovim processes you didn't start

**Safe workflow for testing:**
1. **Before starting a server**: Run `reo-cli list` to see existing servers
2. **Start your server**: Note the PID/port from stderr output or `reo-cli list`
3. **Track your servers**: Keep a list of PIDs you started in this session
4. **After testing**: Only kill the specific server(s) YOU started: `reo-cli --tcp 127.0.0.1:<PORT> kill`
5. **Never assume**: Don't kill servers just because they exist - they might be from another session

## Build Commands

```bash
# Build all crates
cargo build

# Build release
cargo build --release

# Run the main binary (default-members set to runner/)
cargo run

# Run tests
cargo test

# Run tests for a specific crate
cargo test -p reovim-kernel

# Check code without building
cargo check

# Format code
cargo fmt

# Run clippy
cargo clippy

# Generate performance report
cargo run -p perf-report -- bench -v X.Y.Z

# Run in server mode (TCP on 127.0.0.1:12521, or next available port)
cargo run -- --server
# Server prints "Listening on 127.0.0.1:<PORT>" to stderr

# Run server with stdio transport
cargo run -- --stdio

# Run server on Unix socket
cargo run -- --listen-socket /tmp/reovim.sock

# Run server on custom TCP port
cargo run -- --listen-tcp 9000

# Run reo-cli client
cargo run -p reo-cli -- list              # List running servers
cargo run -p reo-cli -- keys 'iHello<Esc>'  # Inject keys, show status (colored output by default)
cargo run -p reo-cli -- --tcp 127.0.0.1:12522 keys 'j'  # Connect to specific server
cargo run -p reo-cli -- keys --format plain_text 'gg'  # Plain text output
cargo run -p reo-cli -- keys --format cell_grid 'gg'  # JSON cell grid
cargo run -p reo-cli -- -i                # Interactive REPL mode
```

## Git Commits

**IMPORTANT**: Claude should NEVER handle git commits. The user will manage ALL git operations. Claude must not have any "opinion" about when or how to commit - only create proposals and let the user decide.

**Forbidden in commits/docs:**
- Emoji
- `Co-Authored-By: Claude ... <noreply@anthropic.com>`
- Any Claude/Anthropic advertisement or promotional content

**Changelog Convention**: Every feature, fix, or notable change MUST include an update to `CHANGELOG.md` under the `[Unreleased]` section before proposing a commit.

For any changes, Claude should create a proposal file at `tmp/<feature-name>-commit.md` with:
- Proposed commit message
- List of files changed (including `CHANGELOG.md`)
- Verification checklist
- Git commands for user to run

### Pull Requests

When creating PRs with `gh pr create`, do NOT add promotional footers like "Generated with Claude Code" or similar. Keep the PR body clean and professional with only relevant technical content.

### Ready-to-Take-Off & Request-for-Landing

**Ready-to-Take-Off (Start of Work):**
1. **Gather Context**:
   - Check Epic #150 and related issues via `gh issue view 150` and linked issues
   - Read `tmp/*` for context from previous sessions
   - Check recent git logs: `git log --oneline -20`
   - Launch `deep-explorer` agent for codebase understanding if needed

2. **Check Environment**:
   - Check `git worktree list` for active branches
   - Check `git status` for uncommitted work

3. **Plan File Setup** (for implementation tasks):
   - Create/update plan file at `~/.claude/plans/reovim-{ISSUE_NUMBER}-{SUBJECT}.md`
   - Plan must be **self-contained** (works after context compact/clear)
   - Include all information needed for implementation:
     - Architecture diagrams, file locations, type definitions
     - Reference to related issues and dependencies
     - Acceptance criteria and test targets

4. **Plan Polish - Round 1**:
   - Launch **multiple `deep-explorer` agents IN PARALLEL with Context** (triple-check pattern)
   - Each agent reviews plan for enhancements and missing details
   - Incorporate all feedback into plan

5. **Plan Polish - Round 2**:
   - Launch **multiple `deep-explorer` agents IN PARALLEL** again
   - Focus on: missing parts, edge cases, architectural concerns
   - **Keep iterating until issues are ZERO**
   - Only proceed to implementation when plan is fully validated

**Plan File Instructions:**
- Add to every plan: **"Never stop until ALL phases are FULLY FINISHED."**
- Include a **Final Procedure** section at the end referencing the Request-for-Landing workflow below

**Request-for-Landing (End of Work):**

1. **Sync with upstream (CRUCIAL)**:
   ```bash
   git stash && git fetch origin develop && git rebase origin/develop
   ```
   - Save work with `git stash` first to avoid rebase conflicts with unstaged changes
   - **Check upstream changes carefully**: Review what changed with `git log HEAD..origin/develop` and `git diff HEAD..origin/develop` before rebasing
   - **Use gh cli to check context**: Check recent PRs/issues with `gh pr list`, `gh issue list`, or `gh pr view <number>` to understand upstream changes
   - Carefully rebase - resolve any conflicts before landing
   - Contributors own their conflicts
   - Restore with `git stash pop` after successful rebase

2. **Run Full Check** → `./scripts/check.sh`
   - Format, build, clippy (zero warnings), all tests must pass

3. **Triple Deep-Explorer Review** (parallel agents):
   | Agent | Focus |
   |-------|-------|
   | 1 | Plan / Issue requirements compare & Documentation Quality |
   | 2 | Test Coverage & Test Quality|
   | 3 | Code quality & Linux/Unix philosophy |

   Agent 3 checks: Mechanism vs Policy, Do one thing well, Composability, Separation of concerns, API purity, Simplicity

   **Grading:** A+ (exceeds) / A (perfect) / B (minor fixes) / C (significant) / F (major)
   - Create review report from agent at `tmp/{ISSUE_NUMBER}-review-{ROUND}.md`
   - **Must achieve A or A+ from ALL THREE agents**
   - If ANY agent gives below A → fix all issues → go back to Step 2
   - Loop until ALL agents give A or A+

4. **Update CHANGELOG.md** with correct phase number (match issue title)

5. **Create Commit Proposal** at `tmp/{ISSUE_NUMBER}-commit.md`:
   - Commit message
   - Files to stage (include CHANGELOG.md)
   - Verification checklist
   - Git commands for user

6. **(Optional) Generate performance report** for version releases:
   ```bash
   cargo run -p perf-report -- bench -v X.Y.Z
   ```

### Cross-Session Communication

The `tmp/` directory is used for communication between Claude sessions:
- **Read `tmp/*` at start** to see proposals/notes from previous Claude sessions
- **Write to `tmp/*.md`** to leave proposals or context for user/future sessions

### Git Worktrees

Use `git worktree list` to see active worktrees. Each issue typically gets its own worktree (e.g., `reovim-{ISSUE_NUMBER}`).

## Detailed Documentation

For in-depth information, see:

**Architecture (v0.9.0+ kernel-based):**
- [docs/architecture/overview.md](./docs/architecture/overview.md) - Layer diagram, Linux mapping, crate deps
- [docs/architecture/kernel.md](./docs/architecture/kernel.md) - Kernel subsystems: mm/, ipc/, core/, block/, sched/
- [docs/architecture/drivers.md](./docs/architecture/drivers.md) - Driver layer: syntax/, input/, display/, lsp/, net/
- [docs/architecture/modules.md](./docs/architecture/modules.md) - Module trait, declare_module!, loader, registry
- [docs/architecture/mechanism-vs-policy.md](./docs/architecture/mechanism-vs-policy.md) - Core design principle
- [docs/architecture/module-mode-inheritance.md](./docs/architecture/module-mode-inheritance.md) - Mode system with inheritance

**Guides and Reference:**
- [docs/guides/development.md](./docs/guides/development.md) - Development guide
- [docs/guides/testing.md](./docs/guides/testing.md) - Testing guide
- [docs/guides/configuration.md](./docs/guides/configuration.md) - Editor settings
- [docs/reference/commands.md](./docs/reference/commands.md) - Command system
- [docs/reference/server-mode.md](./docs/reference/server-mode.md) - RPC server mode

**Archive (v0.8.x legacy reference):**
- [docs/archive/](./docs/archive/) - Legacy documentation
- [archive/](./archive/) - Archived legacy code (lib/core, lib/sys, lib/lsp, plugins)

## Logs and Debugging

Runtime logs are written to timestamped files in `~/.local/share/reovim/` by default (e.g., `reovim-2025-12-22-15-36-20.log`). Each instance creates a new log file with format `reovim-YYYY-MM-DD-HH-MM-SS.log`.

### Log Output Options

```bash
# Default: timestamped file in ~/.local/share/reovim/
reovim myfile.txt

# Custom log file path
reovim --log=/path/to/custom.log myfile.txt

# Log to stderr (useful for debugging)
reovim --log=- myfile.txt
reovim --log=stderr myfile.txt

# Disable logging entirely
reovim --log=none myfile.txt
reovim --log=off myfile.txt

# Server mode with stderr logging (for debugging)
cargo run -- --server --log=-

# Server mode with custom log file
cargo run -- --server --log=/tmp/reovim-server.log

# LSP JSON-RPC message logging (for debugging LSP issues)
reovim --lsp-log=default myfile.rs              # Timestamped lsp-*.log in data dir (trace level)
reovim --lsp-log=/tmp/lsp.log myfile.rs         # Custom path (trace level)
reovim --lsp-log=default:debug myfile.rs        # Default path with debug level
reovim --lsp-log=/tmp/lsp.log:info myfile.rs    # Custom path with info level
# LSP log levels: error, warn, info, debug, trace (default: trace)
cargo run -- --server --log=/tmp/main.log --lsp-log=/tmp/lsp.log  # Both logs

# View logs (timestamped files)
tail -f ~/.local/share/reovim/reovim-*.log

# View latest log
tail -f $(ls -t ~/.local/share/reovim/reovim-*.log | head -1)
```

### Log Level

Control log verbosity via `REOVIM_LOG` environment variable:

```bash
REOVIM_LOG=debug reovim myfile.txt
REOVIM_LOG=trace reovim --log=- myfile.txt  # Debug to stderr
```

### Debugging Flaky Tests

**Think Twice Before Assuming Race Conditions**: When tests pass/fail intermittently:

1. **Don't immediately add delays** - This masks the real problem
2. **Add debug logging** first to trace execution flow
3. **Look for mode/state synchronization issues** - Often what appears to be a timing race is actually a missing state update

**Example**: `<C-w>h` tests failed randomly because `mode_for_command()` didn't recognize `enter_window_mode`, so the local mode wasn't updated before the next key was looked up. This looked like a race condition but was actually a **missing match arm**.

Use `REOVIM_LOG=debug` to enable debug logging in spawned server processes.

**When debugging in background:** Always specify a log file output instead of stderr:
```bash
# GOOD: Use a specific log file
REOVIM_LOG=debug cargo run -- --server --log=/tmp/reovim-debug.log file.txt &

# BAD: Don't use stderr (--log=-) for background processes
REOVIM_LOG=debug cargo run -- --server --log=- file.txt &  # Output gets mixed up
```
Then monitor the log file:
```bash
tail -f /tmp/reovim-debug.log
```
