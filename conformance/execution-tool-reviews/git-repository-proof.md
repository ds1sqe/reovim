# Git Repository Proof Review

Status: **admitted only for exact Source Role Map repository proof on the
reviewed Linux x86_64 environment**

Review ID: `git-repository-proof`

## Consumer and scope

The sole consumer is the `reovim-source-role-map/v2` exact-repository proof in
`tools/conformance/src/git.rs`. The admission covers only five closed
invocation families: Git version, revision/root/object-format resolution, HEAD
tree enumeration, index enumeration, and repository attributes-path
resolution. It grants no Product profile and no general Git command authority.

The exact host binding is `/usr/bin/git`, Arch package `git 2.55.0-1`, reporting
raw stdout `git version 2.55.0` followed by one LF. The admitted subcommands
`rev-parse`, `ls-tree`, and `ls-files` are built into this exact executable.

## Exact command authority

Every invocation uses the canonical executable and canonical repository root
as current directory, clears the inherited environment, and prepends these
ordered arguments before the family-specific arguments:

```text
-c core.fsmonitor=false
-c core.untrackedCache=false
-c core.hooksPath=/dev/null
-c core.fileMode=true
-c core.autocrlf=false
-c core.eol=lf
-c core.ignoreStat=false
-c core.ignoreCase=false
-c core.attributesFile=/dev/null
```

The replacement environment is exactly:

```text
GIT_CONFIG_NOSYSTEM=1
GIT_CONFIG_GLOBAL=/dev/null
GIT_ATTR_NOSYSTEM=1
GIT_NO_REPLACE_OBJECTS=1
GIT_OPTIONAL_LOCKS=0
LC_ALL=C
```

The family suffixes and required semantics are:

| Family | Exact suffix | Required result |
| --- | --- | --- |
| `git-version` | `--version` | One nonempty UTF-8 line; raw bytes and parsed text match the bound identity. |
| `git-revision` | `rev-parse --show-toplevel`; `rev-parse HEAD`; `rev-parse --show-object-format` | Canonical top level equals the checked root; HEAD equals the requested 40/64-lowercase-hex revision; format is `sha1` or `sha256`. |
| `git-tree` | `ls-tree -r -z --full-tree HEAD` | Complete final-NUL records contain only regular executable/non-executable blobs, normalized paths, and exact-length lowercase object IDs; duplicates fail. |
| `git-index` | `ls-files --stage -v -f -z` | Complete final-NUL stage-0 records have flag `H`, admitted modes, normalized paths, and the same mode/object/path map as HEAD. |
| `git-attributes` | `rev-parse --git-path info/attributes` | One path resolves under the repository metadata and the consumer proves that file absent. |

Every process must exit successfully within its committed stdout, stderr, and
deadline bounds, with no capture error, overflow, or stderr bytes. The consumer
also rejects `.gitattributes` anywhere in HEAD, root `.gitmodules`, worktree
root `.gitattributes`/`.gitmodules`, index flags that hide state, and source
bytes/modes that differ from the proved Git objects.

## Executable and direct ELF inputs

The registry binds the exact executable digest/size and these canonical direct
loader inputs discovered from the executable's `PT_INTERP` and `DT_NEEDED`
entries:

| Role/name | Canonical path |
| --- | --- |
| ELF interpreter | `/usr/lib/ld-linux-x86-64.so.2` |
| `libc.so.6` | `/usr/lib/libc.so.6` |
| `libpcre2-8.so.0` | `/usr/lib/libpcre2-8.so.0.15.0` |
| `libz-ng.so.2` | `/usr/lib/libz-ng.so.2.3.3` |

The exact digests and sizes live in the environment binding so identity drift
invalidates admission mechanically. `linux-vdso.so.1` has no file preimage and
is therefore an explicit host/kernel assumption, not a fabricated runtime
input.

## Upstream, license, and dependency posture

Upstream is <https://git-scm.com/>. The installed Arch package declares
`GPL-2.0-only`. This is an independently executed Repository Tooling binary,
not a Cargo dependency, linked Product component, shipped Reovim asset, or
extension contract.

## Assumptions

- The reviewed Linux x86_64 kernel, ELF loader, filesystem, process, and Git
  object/index semantics are trustworthy for this evidence attempt.
- The exact executable keeps all three admitted subcommands built in and does
  not dispatch them through `git-core`, a shell, pager, hook, credential
  helper, transport, editor, or another subprocess.
- Cleared environment plus the explicit `-c` and environment settings contain
  the configuration dimensions used by the admitted read-only commands.
- Repository-local configuration has no unmodeled effect after those
  overrides; the consumer's output-shape, object, index, and attribute checks
  fail closed on disagreement.
- The checker verifies executable and declared runtime-input identities before
  and after use and owns bounded process capture, termination, and reap.

## Invalidators

- Any executable path, bytes, size, package build, raw version output, version
  argv, direct ELF dependency, interpreter, canonical path, digest, or size
  change.
- Any change to command suffixes, ordered `-c` prefix, cleared-environment law,
  replacement environment, current-directory rule, process bounds, or output
  parsers.
- Any admitted command ceasing to be built in, or beginning to invoke a helper,
  hook, pager, transport, editor, credential process, or other executable.
- Any expansion to another Git family, command, repository semantic,
  environment, host architecture, Product consumer, or Product proof profile.
- Any weakening of the HEAD/index equality, object-format, mode, path,
  `.gitattributes`, `.gitmodules`, `info/attributes`, source-byte, or
  executable-identity checks.

## Nonclaims

This review does not enumerate every file or kernel object read by Git, libc,
the loader, NSS, locale handling, or the kernel. It does not admit network
transport, checkout mutation, commits, refs updates, hooks, filters, external
diffs, signing, credentials, submodules, worktrees, alternates, replacement
objects, arbitrary repository configuration, Git-LFS, or any command outside
the five families. It does not prove a malicious binary sandbox, Product
correctness, release qualification, support, or certification.
