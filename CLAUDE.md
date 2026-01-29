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

- **Session** (`SessionId(Arc<str>)`) - Named editing context, shared state
- **Client** (`ClientId(usize)`) - Connection to server, own viewport

Multiple clients can attach to one session, sharing buffers and mode but with
independent cursors and viewports. See `docs/architecture/session-model.md`.

Key types (all use `usize` storage, `as_usize()` accessor):
- `SessionId(Arc<str>)` - Named session in runner
- `ClientId(usize)` - Client connection in driver
- `BufferId(usize)` - Buffer identifier in kernel
- `WindowId(usize)` - Window identifier in kernel

### Workspace Structure (Phase 8)

The project follows a server/client architecture with clear separation:

```
reovim/
├── apps/bin/              # Main binary entry point (reovim-app)
├── server/                # Server-side components
│   ├── lib/kernel/        # Core mechanisms (mm/, ipc/, core/, block/, sched/, api/)
│   ├── lib/server/        # Server runtime (gRPC handlers, session management)
│   └── lib/drivers/       # Server drivers (13 crates)
│       ├── command/       # Command trait and registry
│       ├── command-types/ # Command type definitions
│       ├── input/         # Key events and input parsing
│       ├── syntax/        # Tree-sitter integration
│       ├── lsp/           # Language server protocol client
│       ├── vfs/           # Virtual filesystem operations
│       ├── session/       # Session management traits
│       ├── undo/          # Undo/redo system
│       ├── buffer/        # Buffer operations
│       ├── search/        # Search and replace
│       ├── clipboard/     # Clipboard operations
│       ├── ffi/           # Foreign function interface
│       └── ffi-python/    # Python FFI bindings
├── server/modules/        # Policy modules (17 loadable modules)
│   ├── vim/               # Core Vim-like behavior
│   ├── editor/            # Editor core operations
│   ├── motions/           # Movement commands
│   ├── textobjects/       # Text object definitions
│   ├── commands/          # Command implementations
│   ├── keymap/            # Keymap definitions
│   ├── mode-manager/      # Mode system
│   └── ...                # (options, buffer-ops, clipboard, search, undo, etc.)
├── clients/               # Client applications
│   ├── cli/               # CLI client (gRPC v2)
│   └── tui/               # TUI client (gRPC v2)
│       └── lib/drivers/   # TUI-specific drivers (tui, display)
├── shared/                # Shared libraries
│   ├── protocol/          # gRPC v2 protocol definitions (.proto files)
│   ├── arch/              # Platform abstraction (unix/, windows/)
│   ├── net/               # Network transport layer
│   ├── log/               # Logging infrastructure
│   ├── trace/             # Tracing/diagnostics
│   ├── module-macros/     # `declare_module!` proc-macro for FFI
│   └── testing/           # Integration test utilities
├── tools/                 # Development tools
│   ├── bench/             # Benchmarking suite
│   └── perf-report/       # Performance report generator
├── perf/                  # Versioned performance reports (PERF-{version}.md)
└── archive/               # Legacy code (reference only)
    ├── pre_kernel/        # v0.8.x code (lib/core, lib/sys, plugins)
    └── post_kernel/       # Intermediate v0.9.0 (pre-Phase 8B runner)
```

**Key crate names:**
- `reovim-app` → `apps/bin/` (main binary)
- `reovim-kernel` → `server/lib/kernel/`
- `reovim-server` → `server/lib/server/`
- `reovim-driver-*` → `server/lib/drivers/*/`
- `reovim-module-*` → `server/modules/*/`
- `reovim-client-tui` → `clients/tui/`
- `reovim-client-cli` → `clients/cli/`
- `reovim-protocol` → `shared/protocol/`
- `reovim-arch` → `shared/arch/`

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
- Policy belongs in `server/modules/`, mechanism belongs in `server/lib/kernel/` or `server/lib/drivers/`

### Design Philosophy: Slow but Right

**Always choose the correct solution over the fast hack.** This project prioritizes long-term maintainability and architectural integrity over quick fixes.

**Principles:**
- **Never choose fast but hacky** - Quick workarounds create technical debt that compounds
- **It's OK to be slow** - Taking time to design the right solution pays dividends
- **Proper abstractions over verification hacks** - If you need tests to catch what the compiler should catch, redesign the API
- **Extend APIs properly** - If the current API doesn't support your needs, extend it correctly rather than working around it
- **No "pragmatic" shortcuts** - What seems pragmatic today becomes legacy tomorrow

**Examples:**
- If `&'static str` prevents type safety → change the API to not require `&'static str`
- If kernel purity blocks a feature → design a proper driver/runner-side abstraction
- If a workaround needs unit tests to catch errors → redesign for compile-time safety
- If "it works" but the architecture is wrong → refactor first, then implement

**Red Flags (do NOT proceed):**
- "This is a workaround but it works"
- "We can verify this with tests instead"
- "The proper solution is too complex"
- "This violates X principle but is faster"

**Green Flags (proceed):**
- "This extends the existing pattern correctly"
- "The compiler enforces this constraint"
- "This follows the established architecture"
- "Future modules will benefit from this design"

### Process Safety Policy

**NEVER kill other reovim instances.** Multiple reovim servers can run concurrently for debugging.

**Forbidden commands:**
- `pkill reovim` or `pkill -f reovim`
- `killall reovim`
- Any command that terminates reovim processes you didn't start

**Safe workflow for testing:**
1. **Before starting a server**: Run `reovim cli list` to see existing servers
2. **Start your server**: Note the PID/port from stderr output or `reovim cli list`
3. **Track your servers**: Keep a list of PIDs you started in this session
4. **After testing**: Only kill the specific server(s) YOU started: `reovim cli --tcp 127.0.0.1:<PORT> kill`
5. **Never assume**: Don't kill servers just because they exist - they might be from another session

## Build Commands

```bash
# Build all crates
cargo build

# Build release
cargo build --release

# Run the main binary (default-members set to apps/bin/)
cargo run

# Run tests
cargo test

# Run tests for a specific crate
cargo test -p reovim-kernel

# Run integration tests (currently ignored, pending module loading)
cargo test --test operators --ignored
cargo test --test cursor_movement --ignored

# Run all ignored integration tests
cargo test --ignored

# Check code without building
cargo check

# Format code
cargo fmt

# Run clippy
cargo clippy

# Generate performance report
cargo run -p perf-report -- bench -v X.Y.Z

# Default: Start server + TUI in one command (tmux-like)
cargo run
# Server spawns in background, TUI attaches automatically
# TUI exit doesn't kill server (graceful detach)

# Start server in detached/daemon mode (no TUI)
cargo run -- -d
cargo run -- --detach

# Attach TUI to existing server
cargo run -- attach
cargo run -- attach --tcp 127.0.0.1:12521

# Explicit server mode (TCP on 127.0.0.1:12521, or next available port)
cargo run -- server
# Server prints "Listening on 127.0.0.1:<PORT>" to stderr

# Run server with stdio transport
cargo run -- server --stdio

# Run server on Unix socket
cargo run -- server --socket /tmp/reovim.sock

# Run server on custom TCP port
cargo run -- server --tcp 9000

# Run CLI client
cargo run -- cli list                     # List running servers
cargo run -- cli keys 'iHello<Esc>'       # Inject keys
cargo run -- cli --tcp 127.0.0.1:12522 keys 'j'  # Connect to specific server
cargo run -- cli mode                     # Get current mode
cargo run -- cli cursor                   # Get cursor position
cargo run -- cli --format json mode       # JSON output format
cargo run -- cli -i                       # Interactive REPL mode

# TUI frame capture (#447) - requires connected TUI
cargo run -- cli capture                  # Capture with raw_ansi (ANSI colors)
cargo run -- cli capture --format plain_text  # Plain text without colors

# Explicit TUI client (connects to running server)
cargo run -- tui                          # Auto-discover server
cargo run -- tui --tcp 127.0.0.1:12521    # Connect to specific server
cargo run -- tui --headless               # Headless mode (no TTY, for CI/scripting)
```

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

**Deferral Protocol (MANDATORY):**

When deferring work (skipping a test, leaving a TODO, postponing a feature), you MUST:

1. **Create a draft proposal** - Write `tmp/deferral-draft-{topic}.md` with:
   ```markdown
   # Deferral Proposal: {topic}

   ## What
   [Description of deferred work]

   ## Why
   [Reason for deferring]

   ## Draft Issue
   - Title: `fix: {description}`
   - Body: [Acceptance criteria, context]

   ## Code Reference
   [File and line where `#[ignore]` or `// TODO` will be added]
   ```
2. **Continue working** - Don't block on approval
3. **Mention at finish** - At end of work session, remind user: "Review deferral draft at `tmp/deferral-draft-*.md`"
4. **After user approval** - Create issue with `gh issue create`, update code with issue reference

**Never create tracking issues without user approval.** The draft file gives user control over what gets tracked and how.

Example workflow:
```
[During work] Claude encounters failing test
[During work] Claude creates tmp/deferral-draft-dG-motion.md
[During work] Claude continues with other tasks
[At finish]  Claude: "Review deferral draft at tmp/deferral-draft-dG-motion.md"
[User]       Reviews and approves
[Claude]     Creates issue #429, updates #[ignore = "... (#429)"]
```

**Self-Contained Issues Policy:**
- **Every GitHub issue MUST be completely self-contained**
- A developer reading ONLY the issue must have ALL information needed to implement it
- **NEVER reference local files** - No "see `tmp/plan.md`", "details in `~/.claude/plans/...`", or "see file X"
- **NEVER reference plan files** - All relevant plan content must be copied INTO the issue
- **NEVER use relative file paths** for repo files - No "see `docs/heritage/legacy-memorial.md`"
- **DO use git blob permalinks** for repo file references:
  - Get blob URL: `gh browse <file> --commit HEAD` or GitHub UI
  - Format: `[Description](https://github.com/owner/repo/blob/<commit-sha>/path/to/file.md)`
  - Permalinks point to a specific version - paths break when files move or change
- Include in the issue itself:
  - Full technical context and architecture decisions
  - Acceptance criteria and test requirements
  - Code snippets, type definitions, file paths as needed
  - Any diagrams or examples (use markdown code blocks)
- **Epics follow the same rule** - Epic descriptions must be self-sufficient, not depend on external docs
- Local plan files (`tmp/`, `~/.claude/plans/`) are for Claude sessions only, not for issue references

**Issue Title Format:**
```
type: short description
```
- **Types**: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `proposal`
- Examples:
  - `feat: add insert mode character input`
  - `fix: cursor not updating after delete`
  - `chore: rename agents to NASA theme`
  - `docs: update architecture overview`
  - `proposal: new buffer management API`

**Issue Body Format:**
```markdown
## Summary
[1-3 sentences describing what and why]

## Changes
[Bullet list of planned changes]

## Why
[Motivation/context for this work]

## Related
- Part of #EPIC_NUMBER (if applicable)
- Depends on #ISSUE (if applicable)
```

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

For any changes, Claude should create:
1. **`tmp/commit.sh`** - Executable script with git commands
2. Optionally **`tmp/<feature-name>-commit.md`** for complex commits with:
   - Proposed commit message
   - List of files changed (including `CHANGELOG.md`)
   - Verification checklist

### Pull Requests

When creating PRs with `gh pr create`, do NOT add promotional footers like "Generated with Claude Code" or similar. Keep the PR body clean and professional with only relevant technical content.

### Launch Sequence & Final Approach

**Launch Sequence (Start of Work):**

1. **Gather Context**:
   - Check Epic and related issues via `gh issue view {Number}` and linked issues
   - Read `tmp/{ISSUE}/*` for context from previous sessions
   - Check recent git logs: `git log --oneline -20`
   - Launch `voyager` agent for codebase understanding if needed

2. **Check Environment**:
   - Check `git worktree list` for active branches
   - Check `git status` for uncommitted work

3. **Plan File Setup** (for implementation tasks):
   - Launch `oracle` agent for complex planning (uses Opus)
   - Create/update plan file at `~/.claude/plans/reovim/{ISSUE_NUMBER}-{SUBJECT}.md`
   - Plan must be **self-contained** (works after context compact/clear)
   - Include all information needed for implementation:
     - Architecture diagrams, file locations, type definitions
     - Reference to related issues and dependencies
     - Acceptance criteria and test targets

4. **Countdown - Go Poll Round 1**:
   - Launch review agents (mission-control, telemetry, flight-director) in `countdown` mode with `model: {model}`
   - Each agent reviews plan for enhancements and missing details
   - Incorporate all feedback into plan

5. **Countdown - Go Poll Round 2**:
   - Launch agents again in `countdown` mode
   - Apply **A+ skip rule**: skip agents that gave A+ in round 1
   - Focus on: missing parts, edge cases, architectural concerns
   - **Keep iterating until all agents report GO (A or A+)**
   - **IMPORTANT** Only proceed to implementation when plan is fully validated

**REMINDER - Countdown Procedure:**
```
Before ANY implementation:
1. oracle (Opus)     → Create/refine plan
2. mission-control   → Review plan compliance
3. telemetry         → Review test strategy
4. flight-director   → Review architecture
All must GO before coding starts!
```

**Plan File Instructions:**
- Add to every plan: **"Never stop until ALL phases are FULLY FINISHED."**
- Include a **Final Procedure** section at the end referencing Final Approach below

**Final Approach (End of Work):**

Use `/final-approach [issue]` or run manually:

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

3. **Go Poll** (agents in `reentry` mode):
   | Agent | Focus | Call |
   |-------|-------|------|
   | `mission-control` | Plan compliance, documentation | *"Mission Control: GO"* |
   | `telemetry` | Test coverage, test quality | *"Telemetry: GO"* |
   | `flight-director` | Unix philosophy, code quality | *"Flight Director: GO"* |

   Flight Director checks: Mechanism vs Policy, Do one thing well, Composability, Separation of concerns, API purity, Simplicity

   **Grading:** A+ (exceeds) / A (go for landing) / B (minor fixes) / C (significant) / F (no-go)
   - Each agent writes to `tmp/{ISSUE}/landing/round-{N}/{agent}.md`
   - **Must achieve A+ from ALL THREE agents**
   - If ANY agent reports NO-GO → fix issues → go back to Step 2
   - Loop until all agents report GO

4. **Create Landing Document** at `tmp/{ISSUE}/landing.md`:
   - **Section 1: Go Poll Summary**
     - Final grades table (all three agents) with links to detailed reports
     - Files changed (new/modified with LOC)
     - Highlights from each agent
   - **Section 2: Verification Checklist**
     - `./scripts/check.sh` passed
     - CHANGELOG.md updated
     - All agents reported GO
   - **Section 3: Commit**
     - Commit message
     - Files to stage
     - Git commands for user

5. **Update CHANGELOG.md** with correct phase number (match issue title)

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

## Claude Agents & Skills

Custom agents and skills in `.claude/` provide specialized workflows for this project.

### Available Agents

| Agent | Model | Purpose |
|-------|-------|---------|
| `oracle` | Opus | Planning, architecture, complex decisions |
| `voyager` | Sonnet | Codebase exploration, dependency tracing |
| `mission-control` | Haiku | Plan compliance, documentation review |
| `telemetry` | Haiku | Test coverage & quality review |
| `flight-director` | Sonnet | Unix philosophy & code quality review |

**Agent Modes (review agents):**
- **countdown**: T-minus checks before launch (validate plan)
- **orbit**: In-flight monitoring (Claude uses, asks user when blocked)
- **reentry**: Final Approach review (Go Poll for landing)
- **ground-ops**: General ground support (design help, strategy advice)

**A+ Skip Rule:** If an agent gives A+ in round N, skip that agent in round N+1.

**Deferral Policy:** Deferrals are OK if tracking issue exists (e.g., #248). No grade penalty for properly documented deferrals.

### Available Skills

| Skill | Command | Purpose |
|-------|---------|---------|
| `countdown` | `/countdown [issue]` | Pre-implementation validation with oracle (Opus) + Go Poll  |
| `final-approach` | `/final-approach [issue]` | Landing sequence with Go Poll (3 agents in reentry mode) |

### Agent Files

```
.claude/
├── agents/
│   ├── oracle.md                # Planning, architecture - Opus (purple)
│   ├── voyager.md               # Codebase exploration - Sonnet (red)
│   ├── mission-control.md       # Plan compliance review - Haiku (blue)
│   ├── telemetry.md             # Test coverage review - Haiku (green)
│   └── flight-director.md       # Code quality review - Haiku (yellow)
└── skills/
    ├── countdown/
    │   └── SKILL.md             # /countdown command
    └── final-approach/
        └── SKILL.md             # /final-approach command
```

### Output Structure

```
tmp/{ISSUE}/
├── countdown/           # Plan review (before coding)
│   └── round-{N}/
│       ├── mission-control.md
│       ├── telemetry.md
│       └── flight-director.md
└── landing/             # Landing review (before merge)
    └── round-{N}/
        ├── mission-control.md
        ├── telemetry.md
        └── flight-director.md
```

## Detailed Documentation

For in-depth information, see:

**Architecture (v0.9.0+ kernel-based):**
- [docs/architecture/overview.md](./docs/architecture/overview.md) - Layer diagram, Linux mapping, crate deps
- [docs/architecture/kernel/overview.md](./docs/architecture/kernel/overview.md) - Kernel subsystems: mm/, ipc/, core/, block/, sched/
- [docs/architecture/drivers/overview.md](./docs/architecture/drivers/overview.md) - Driver layer: syntax/, input/, display/, lsp/, net/
- [docs/architecture/modules/overview.md](./docs/architecture/modules/overview.md) - Module trait, declare_module!, loader, registry
- [docs/architecture/mechanism-policy/](./docs/architecture/mechanism-policy/) - Epic #353: mechanism/policy separation implementation
- [docs/contributing/philosophy/mechanism-vs-policy.md](./docs/contributing/philosophy/mechanism-vs-policy.md) - Core design principle
- [docs/architecture/modules/mode-inheritance.md](./docs/architecture/modules/mode-inheritance.md) - Mode system with inheritance

**Contributing:**
- [docs/contributing/getting-started.md](./docs/contributing/getting-started.md) - Development setup
- [docs/contributing/guides/testing.md](./docs/contributing/guides/testing.md) - Testing guide

**User Guide:**
- [docs/user-guide/configuration.md](./docs/user-guide/configuration.md) - Editor settings
- [docs/user-guide/commands.md](./docs/user-guide/commands.md) - Command system
- [docs/user-guide/server-mode.md](./docs/user-guide/server-mode.md) - RPC server mode

**Heritage (foundational documents):**
- [docs/heritage/project-kernel-phases.md](./docs/heritage/project-kernel-phases.md) - Epic #150 phase history
- [docs/heritage/legacy-memorial.md](./docs/heritage/legacy-memorial.md) - Patterns learned from v0.8.x
- [docs/heritage/clean-architecture-proposal.md](./docs/heritage/clean-architecture-proposal.md) - Original architecture proposal

**Archive (v0.8.x legacy code):**
- [archive/docs/](./archive/docs/) - Legacy documentation
- [archive/](./archive/) - Archived legacy code (lib/core, lib/sys, lib/lsp, plugins)

## Logs and Debugging

### Current State (v0.9.0)

The logging system is partially implemented. The driver infrastructure exists in `shared/log/` but CLI argument wiring is pending.

**What works now:**
- Server outputs logs to stderr via tracing
- RPC debug commands to query/control running server

**Planned (not yet implemented):**
- `--log=<path>` CLI argument
- `--lsp-log=<path>` CLI argument
- `REOVIM_LOG` environment variable integration
- Timestamped log files in `~/.local/share/reovim/`

### RPC Debug Commands

Query and control logging on a running server via CLI:

```bash
# Get current log level
reovim cli log-level

# Set log level dynamically (trace, debug, info, warn, error, off)
reovim cli log-level debug

# Get recent log entries (default: 50)
reovim cli log-tail
reovim cli log-tail --count 100

# Filter by level (shows entries >= specified level)
reovim cli log-tail --level warn

# Filter by target module
reovim cli log-tail --target runner::server

# Search for text in messages (case-insensitive)
reovim cli log-tail --grep "connection"

# Combine filters
reovim cli log-tail --level info --target runner --grep error

# Follow mode - stream logs in real-time (Ctrl+C to stop)
reovim cli log-tail --follow
reovim cli log-tail --follow --level warn
```

Output is color-coded when stdout is a TTY:
- ERROR (red), WARN (yellow), INFO (green), DEBUG (cyan), TRACE (gray)

### TUI Debug Mode

Enable debug mode for TUI diagnostics:

```bash
# Enable debug statusline and frame capture
reovim tui --debug

# Custom log directory
reovim tui --debug --debug-dir /tmp/reovim-debug

# Custom session name
reovim tui --debug --debug-name mysession
```

**Output files:**
- `~/.local/share/reovim/logs/tui/{name}_{start_time}.log` - Session log
- `~/.local/share/reovim/logs/tui/frame-buffer/{name}-{timestamp}.frame` - Frame captures (every 5 seconds)

**Statusline format:**
```
[YY-MM-DD HH:MM:SS TZ] [server: ADDR] [mode: MODE] [modules: N]
```

The statusline is rendered at the bottom of the screen with inverse colors.

**Session log events:**
- Session start/end
- Mode changes
- Terminal resize
- Frame captures

### Debugging Flaky Tests

**Think Twice Before Assuming Race Conditions**: When tests pass/fail intermittently:

1. **Don't immediately add delays** - This masks the real problem
2. **Add debug logging** first to trace execution flow
3. **Look for mode/state synchronization issues** - Often what appears to be a timing race is actually a missing state update

**Example**: `<C-w>h` tests failed randomly because `mode_for_command()` didn't recognize `enter_window_mode`, so the local mode wasn't updated before the next key was looked up. This looked like a race condition but was actually a **missing match arm**.

### Debugging Multi-Window and Layout Issues

**Common symptoms and causes:**

1. **TUI not syncing with server state**
   - **Symptom**: Server has correct state (verified via CLI), but TUI shows stale data
   - **Cause**: Push notifications not being sent for state changes
   - **Check**: Verify `emit_from_state_changes()` is called with correct `StateChanges`
   - **Key file**: `runner/src/server/rpc/handlers/input.rs` - accumulates and emits changes

2. **Focus indicator at wrong position**
   - **Symptom**: `▪` appears at x=0 instead of focused window's position
   - **Cause**: Window ID comparison finding wrong placement
   - **Debug**: Add logging in `render_multi_window()` to trace `focused_id` and found placement

3. **Window separators incomplete**
   - **Symptom**: Missing `│`, `─`, or `┼` characters
   - **Cause**: Border drawing not checking for existing characters
   - **Key file**: `runner/src/server/rpc/handlers/screen.rs` - `draw_window_separators()`

**Notification model (Push vs Pull):**

| Type | Method | Use Case |
|------|--------|----------|
| **Pull** | RPC call (`state/layout`, `content`) | CLI queries, debugging |
| **Push** | `layout_changed` notification | TUI real-time updates |

- TUI caches layout from push notifications
- CLI always queries fresh data from server
- If push is broken, TUI shows stale data but CLI works

**Manual testing workflow:**

```bash
# Start server
./target/release/reovim server --tcp 13000 &

# Test via CLI (Pull - always fresh)
./target/release/reovim cli --tcp 127.0.0.1:13000 --format json layout

# Test via TUI (Push - depends on notifications)
./target/release/reovim tui --tcp 127.0.0.1:13000 --debug
```

**Key files for layout/notification debugging:**

| File | Purpose |
|------|---------|
| `server/lib/server/src/grpc/input.rs` | Key processing, change accumulation |
| `server/lib/server/src/grpc/state.rs` | Screen content rendering, state queries |
| `server/lib/server/src/session/` | Session and notification logic |
| `clients/tui/src/` | TUI notification handling, layout caching |
| `server/modules/vim/` | Vim-like behavior, mode management |

### Integration Test Infrastructure

Phase 7+ integration tests use a fluent builder API in `shared/testing/`:

```rust
// Single-client test example
let result = IntegrationTest::new()
    .await
    .with_buffer("hello world")
    .send_keys("dw")
    .run()
    .await;
result.assert_buffer_eq("world");

// Step-by-step test with per-key assertions (Issue #428)
let trace = StepTest::new()
    .await
    .with_buffer("hello world")
    .step("d")
        .expect_mode_contains("DELETE")
    .step("w")
        .expect_buffer("world")
    .run()
    .await;
trace.assert_ok();

// Multi-client test example
MultiClientTest::with_clients(2)
    .await
    .run(|mut clients| async move {
        clients[0].send_keys("ihello<Esc>").await.unwrap();
        let content = clients[1].get_buffer().await.unwrap();
        assert!(content.contains("hello"));
    })
    .await;
```

**Key components:**
- `TestServerHarness` - Spawns server on OS-assigned port, auto-cleanup via Drop
- `IntegrationTest` - Fluent builder for single-client tests, temp file cleanup
- `StepTest` - Per-keystroke state tracking with inline assertions
- `MultiClientTest` - Multi-client concurrent testing
- `TestResult` - Assertions: `assert_buffer_eq!`, `assert_cursor!`, `assert_register!`, `assert_mode!`

**Per-test log capture (Issue #428):**
- Logs are saved to `{module}/tmp/test-logs/{test_name}_{timestamp}.log`
- Example: `modules/vim/tmp/test-logs/test_dw_delete_word_20260124_140503.log`
- Failed assertions include log path hint: `Server log: tmp/test-logs/...`
- Access via `result.log_path()` or `trace.log_path()`

**Module loading troubleshooting:**
- If tests behave unexpectedly, check logs for `WARN module not found`
- This indicates `.so` files are outdated - run `./scripts/build-module.sh <module> --install`
- Verify FFI symbols with the install script output
