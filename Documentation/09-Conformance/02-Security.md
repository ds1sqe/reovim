# 9.2 — Security

**Scope.** Threat model, transport auth, library-root path identity,
sandbox boundary statement.

**Heritage.** v4 README §15.0, §15.1.

**Locked rules.** `SEC1..SEC5`.

---

## 1. Threat model

v4 explicitly models these threats:

| Threat | Mitigation owner |
|---|---|
| Malicious project config altering host-class behaviour | CFG4 + CFG8 |
| Compromised package source distributing native cdylibs | PM10, SEC3, SEC4 |
| Local attacker manipulating library roots, symlinks, hardlinks, parent dir perms | SEC3, SEC4 |
| Remote unauthenticated client reaching debug/session surfaces | SEC2 |
| Debug-drive abuse mutating editor state | DS13 |
| Secret leakage through config dump, DS12, ErrorBuf, crash reports, debug recordings | CFG7 |
| Malicious native cdylib AFTER load | NOT MITIGATED — see SEC1 |

> **SEC5 — Threat model is enumerated and named.** This table is
> the authoritative list. New mitigations carry a row pointer here.
> *Class*: spec.

## 2. Sandbox boundary

> **SEC1 — Runtime-loaded cdylibs are trusted native code; no
> memory sandbox in v4.** v4 hardens loading, lifecycle, ABI, and
> observability. Once a cdylib is `Active`, the kernel cannot
> contain its memory access or syscalls.
>
> Implications:
> - A signed cdylib in the lockfile is treated as trusted code
>   running in-process.
> - Untrusted user-supplied native code is not a v4 use case.
> - Future sandbox work (WASM, RPC isolation) is out of v4 target.
>
> *Class*: spec.

## 3. Transport auth

> **SEC2 — `auth = "none"` is local-only.** Valid for:
> - Unix sockets with owner-only permissions (mode 0700 directory
>   AND mode 0600 socket file).
> - Loopback transports explicitly marked local at config time.
>
> Remote/listening TCP requires `auth = "mtls"` or `auth = "token"`
> unless an explicit unsafe override is forced AND logged at WARN
> per launch.
>
> *Class*: runtime.

## 4. Library-root path identity

> **SEC3 — Library-root path identity verified before `dlopen`.**
> Per 7.1 §8, all paths from library root to artefact are checked
> for owner, perms, symlink resolution, and the same-inode
> guarantee between checksum and `dlopen`.
> *Class*: runtime.

> **SEC4 — Symlink and hardlink policy.** §7.1 §8.
> *Class*: runtime.

### 4.1 Owner-only definition

For SEC2's "owner-only permissions":

| Object | Required mode | Notes |
|---|---|---|
| Unix socket directory | 0700 | only the running uid may traverse |
| Unix socket file | 0600 | only the running uid may connect |
| TCP listener | NOT covered by SEC2 | requires mTLS / token |

Implementation MUST stat both the file and its parent directory
before binding/connecting.

## 5. Audit events

Required DS12 audit events (cross-reference 9.4):

- `auth.reject`
- `pkg.verify.fail`
- `cdylib.load.fail` (when path identity fails)
- `debug.drive.start|ok|fail` (DS13)
- `force-override` warning lines (CFG5)

## 6. Out of v4 target

- Memory-sandboxed cdylibs (WASM, sub-process isolation).
- Per-cdylib syscall filtering.
- Project package overlay trust prompts beyond `pkg sync`.

## Open items

1. Token / mTLS protocol details for `auth = "token"` /
   `auth = "mtls"` — protocol envelope, rotation, revocation.
2. Cross-platform "owner-only" semantics for Windows transports.
3. Whether `force = "unsafe-allow"` should require a TTY confirm
   in interactive launches. Default: no — log only.

## Conformance

| Rule | Fixture |
|---|---|
| SEC1 | Untrusted cdylib loaded → no sandbox; behaviour proves trust assumption. |
| SEC2 | TCP listener with `auth = "none"` and no unsafe override → reject at boot. |
| SEC3 | World-writable parent dir → reject before `dlopen`; DS12 emits. |
| SEC4 | Symlink to /etc/passwd inside library root → reject at sync. |
| SEC5 | Threat-model row added without a mitigation → CF1 fails. |
