# 2.5 — Machine Boot

**Scope.** RTOS-itself boot: the system-kernel boot layer that sits between
`arch::_start` and root-daemon/payload launch, its subsystems, the boot-proof
model, the device lifecycle, connected-device handling at runtime, and the
scheduler park/wake seam. Over-OS mode skips this layer — the host OS *is*
the underlying system role.

**Heritage.** Design-stage. The bare-metal floor and the editor-core
`EditorInit`/`EditorCore` handoff exist (`#796`/`#797`); the middle machine-boot
layer is the 0.16 design that makes a single-seat Pi appliance real. The
editor-core handoff is a payload launch, not the proof that the OS booted.
Mode overview: `01-Architecture/06-OS-Modes.md`.

**Locked rules.** None new (design-stage). `machine.*` event families are
provisional and must follow LOG2 rendering.

---

## 1. The missing middle layer

The existing boot model is a good floor but is missing a middle layer:

```text
arch::_start
  -> system-kernel boot
     -> boot-profile selection
     -> IRQ/timer/memory/device/block/fs init
     -> root daemon
     -> /bin/init through exec/process/scheduler/syscall
     -> /bin/init requests /bin/sh through typed syscall
     -> /bin/sh requests interactive shell/session startup
     -> tty/CLI
     -> optional payload launch (editor/server/client)
```

RTOS-itself needs a real system kernel, not just `arch` plus the editor core.
`arch/` stays the smallest unsafe/hardware boundary (no policy);
system-kernel boot owns the services that are too large to hide inside
`arch::sys`, then starts a root daemon that can launch the editor/server/client
payload later. This keeps `editor/lib/core` from becoming an accidental
operating-system kernel and gives the kernel a testable boot success condition
that does not depend on editor-core `EditorInit::boot`.

| Layer | Owns |
|---|---|
| `arch/` platform floor | CPU entry, asm, MMIO primitives, timer registers, UART bytes, memory-map handoff |
| system kernel | scheduler, IRQ routing, driver model, block cache, filesystem, console device, tty/root daemon, power/shutdown |
| editor core | sessions, domains, streams, state, services, view/update scheduling |
| client/platform runtime | input/render adaptation (hosted terminal vs framebuffer + keyboard) |

## 2. Machine-kernel subsystems

Minimum real-kernel subsystems, ordered by what unblocks a credible
appliance:

1. **Scheduler.** Tickless event scheduler driven by the generic timer +
   IRQ wake. The first profile can stay cooperative/non-preemptive for
   editor tasks, but the IRQ path must be real. Priority bands and bounded
   preemption come only where input/display/block latency requires them.
   The park/wake seam is in §6.
2. **Interrupts.** GIC-400 setup, vector table, IRQ mask/ack/eoi, timer
   IRQ, UART IRQ, then PCIe/xHCI IRQ. This is the line between toy polling
   and a real appliance.
3. **Memory.** Physical memory map, page allocator, DMA-safe regions,
   MMU/cache policy. The current arena is enough for boot/selftest, not
   USB/block/display.
4. **Block.** EMMC2/SD driver (or a simpler early target), request queue,
   block cache, flush/barrier semantics, error model. Persistence is not
   credible until block writes have ordering and failure semantics.
5. **Filesystem.** A real persisted store, not only ramfs. FAT32 is
   attractive for SD interop; a log-structured store may fit PS1 atomicity
   better — a spec decision on atomic save for non-POSIX block stores is
   open (`02-Process/04-Persistence.md`).
6. **Device model.** A static bus/device/driver registry first (platform
   devices, PCIe root/VL805, USB device/interfaces, HID keyboard,
   framebuffer, block). Avoid Linux-scale hotplug until needed.
7. **Console / TUI substrate.** A framebuffer text console and input
   pipeline producing the same reovim raw-input records — not an ANSI/termios
   emulation (`01-Architecture/06-OS-Modes.md` §5).
8. **Root daemon + init + tty.** A first system-kernel supervisor task starts a
   userland `/bin/init` through exec/process/scheduler/syscall. Init publishes
   the first boot service as `shell -> /bin/sh` through the typed
   `init-service-start` syscall. Rootd requires that retained service table
   target before it starts the shell; the shipped `/bin/init` image no longer
   emits the older session-target shortcut row.
   Rootd then execs that normal `/bin/sh` program and exposes a small
   bash-like Reovim shell for recovery and diagnostics only after `/bin/sh`
   requests interactive startup through `session-shell-start`; an init that
   exits without a shell service target request, or a shell target that exits
   without the start request, leaves boot stopped instead of implicitly
   entering the shell. Rootd owns boot-profile targets, service/payload
   dispatch, and recovery/headless diagnostics. The current `/bin/init` owns
   the first service policy handoff, `/proc/services` exposes service state,
   reason, the init requester, and the running shell service process/task, and
   `/bin/sh` owns the first interactive-shell handoff plus retained
   `shell.session path=/bin/sh ... line=<command>` audit rows. `/bin/sh` must
   also request the implemented `argv-v1` / `single-pipe` line discipline
   through `session-line-discipline` before rootd can enter the prompt.
   `/proc/session` and `/bin/proc session` expose that active shell-session
   owner, requested target/start state, requested line discipline, pipe mode,
   cwd, prompt, input mode, launch state, and `line_loop_host=rootd` through the
   typed `snapshot-session` syscall. Rootd still hosts the physical line reader
   and parser implementation for that requested discipline, but direct
   shell-launched `/bin` programs are process and scheduler children of PID/task
   2, with retained `exec.parent ... ppid=2 ... parent_task=2` proof rows.
   `/bin/sh` is still not the full parser/line-loop implementation and init is
   only a bounded one-service boot handoff, not a full service manager.
   The first implementation must boot to a prompt with no editor-core
   dependency; editor/server/client launch is a registered payload program
   supplied by the composition root. Bash-like means familiar prompt, words,
   and `/bin` programs such as `ls` and `cd`; it
   does **not** mean a second executable implementation inside the shell,
   POSIX shell execution, or a public POSIX face. The system kernel owns only
   the `/bin` descriptor, exec, VFS, process, scheduler, and syscall ABI; the
   concrete `/bin` catalog and program bodies are installed by the
   `reovim-os` image. Every operator-visible executable must resolve through an
   image `/bin` descriptor before it can run; prompt parsing must not become a
   separate executable implementation path.
   `/bin` programs receive Reovim standard stream descriptors (`stdin=0`,
   `stdout=1`, `stderr=2`); output goes through a descriptor-shaped fd write
   path, and stderr/rejected writes retain `fd-write` rows even though today's
   framebuffer/serial console sink still renders stderr beside stdout. Stdin
   also goes through descriptor-shaped `fd-read`; current shell-launched
   programs receive an empty stdin buffer and therefore read EOF unless a pipe
   or scheduled exec payload seeds stdin. The first pipe sources are bounded
   run-to-completion pipes: root-shell `left | right` and payload-source
   `pipe-bin PRODUCER -- CONSUMER` both resolve each side as `/bin` programs,
   capture producer stdout through the syscall write path, and pass that
   capture as the consumer's pending exec stdin. `/bin/cat` reads stdin when no
   paths are passed, so `pwd | cat` and payload `pipe-bin pwd -- cat` prove the
   ordinary pipe consumer path while `/bin/input` remains the read-diagnostic
   program. Interactive
   line input is separate from fd 0 for now: `/bin/read` consumes one current
   root-console line through a typed `tty-read-line` syscall. Payload source
   images can also issue `read-tty-line`; that operation consumes the same
   root-daemon line callback through the payload syscall handle and records
   `tty-read-line` under `/payload/...`, not under `/bin/launch`. Even stateful
   navigation such as `cd` resolves as `/bin/cd` and changes session state only
   through typed kernel services; it is not a second executable path outside
   `/bin`.

   Current launch-capable images record payload source images as child
   process/task lifecycles such as `/payload/reovim` or
   `/payload/editor-smoke`, with explicit `ready -> running -> exited|failed`
   bookkeeping, FIFO scheduler dispatch records visible through
   `/proc/syscalls`, scheduler state visible through `/proc/scheduler`,
   parent wait records visible through `/proc/waits`, and retained
   `wait.start` / `payload.start` / `payload.exit` / `wait.end` audit rows.
   Scheduled payload execution also emits a child-context `payload-run`
   syscall row, separate from the parent `/bin/launch` or `/bin/reovim`
   `payload-launch` request. A payload source image can publish
   `service-ready NAME`; that typed syscall records the current `/payload/...`
   process as a started service in `/proc/services` and dump `services:` rows,
   with owner and service pid/task set to the payload process. If the service
   process exits normally, the service row becomes
   `state=exited reason=process-exited` through typed `service-exited`
   evidence. If a resident service process is later terminated through the typed
   process-kill path, the service row becomes
   `state=failed reason=process-killed` while retaining the failed service
   pid/task for evidence. Payload source images can also call
   `write-service-table`, which snapshots the same retained service table
   through the payload's typed `snapshot-services` syscall context instead of
   scraping `/proc/services`, and `write-process-table`, which snapshots
   retained process rows through the payload's typed `snapshot-processes`
   context instead of scraping `/proc/processes`. Payload source images can use
   `write-boot-profile` for selected profile, input, and payload catalog facts,
   and `write-device-table` for boot device inventory, instead of reading
   `/boot/profile` or `/boot/devices` pseudo-files for those facts.
   `write-exec-table`, `write-pending-exec-table`, and `write-source-table`
   similarly snapshot executable load, pending-exec, and source-store state
   through typed payload-context syscalls instead of `/proc` reads.
   `write-scheduler-state`, `write-task-table`, `write-wait-table`, and
   `write-syscall-table` snapshot scheduler, task, wait, and syscall trace
   state through the same typed payload context. The operator-facing linked
   `/bin/service-stop NAME` path is distinct: it terminates a started resident
   service process while retaining the backing process as failed and moves the
   service row to `state=stopped reason=operator-stop` through typed
   `service-stop` evidence.
   Linked `/bin/service-start NAME` and `/bin/service-restart NAME` now launch
   the retained `/payload/...` service target through the same payload
   exec/spawn/wait scheduler path as `/bin/launch`; restart first records the
   stop transition for any live resident instance, then records a replacement
   service process through typed `service-restart` evidence. The old `/bin/proc
   service-*` compatibility subcommands are retired. Non-payload services such
   as `shell -> /bin/sh` remain protected.
   Payload source images can exercise both sides of process ownership:
   `spawn-bin PROGRAM [ARG...]` leaves a live child to be adopted after payload
   exit, `spawn-wait-bin PROGRAM [ARG...]` waits on that child by retained PID
   through the same typed wait syscall exposed to linked `/bin/wait`,
   `spawn-kill-bin PROGRAM [ARG...]` terminates the retained child by PID
   through the same typed `process-kill` syscall exposed to linked `/bin/kill`,
   proving the child never dispatches, and
   `exec-payload PAYLOAD [ARG...]` launches another `/payload` child through
   the same payload load/spawn/wait path used by `/bin/launch`.
   `spawn-payload PAYLOAD [ARG...]` admits another `/payload` child without
   beginning a wait, leaving it pending for later scheduler dispatch or rootd
   adoption if the parent exits first.
   `spawn-wait-payload PAYLOAD [ARG...]` admits another `/payload` child,
   waits on it by retained PID through the same typed wait syscall exposed to
   linked `/bin/wait`, and reaps the child after its scheduled payload source
   image exits. `spawn-kill-payload PAYLOAD [ARG...]`
   admits a child payload, terminates it through the same retained-PID
   `process-kill` syscall before it can dispatch, and records no synthetic
   wait.
   Process and syscall rows include loader, source-kind, and `entry_fn`
   metadata. Process rows also retain bounded `argc`, `argv0`, and
   `argv0_truncated` metadata, and `/proc/self` plus `/bin/proc self` expose
   the current program record through a typed `process-self` syscall. `/bin`
   programs and launch-profile
   payloads now both use descriptor identity plus loader-visible source-store
   paths: descriptors name `/bin/*` or `/payload/*`, while image source tables
   own the default byte-backed source artifacts. A bounded runtime-installed
   source overlay is checked before those image tables, and `/proc/sources`
   plus `/bin/proc sources` report `origin=image|installed`.
   Payload launch preserves the bounded argv supplied by the launching `/bin`
   program: `/bin/launch PAYLOAD ARG...` spawns a `/payload/...` process whose
   process record has `argv0=PAYLOAD` plus the remaining args, and `/bin/reovim
   ARG...` forwards those alias arguments to `/payload/reovim`. The shipped
   `/payload/reovim` smoke body now starts `/payload/server-smoke` through
   `exec-payload`, then snapshots retained service state from the payload
   syscall context before returning ready.
   Payload source images may terminate with bounded numeric `exit-code N`
   results; code `0` is success, nonzero codes fail the payload process with
   the same retained exit code, and the parent `/bin/launch` or `/bin/reovim`
   process preserves that code instead of collapsing it to a generic
   `payload.failed`. Waited payload child `/bin` programs that return bounded
   `exit-code N` through `exec-bin`, `exec-bin-stdin-hex`, `pipe-bin`, or
   `spawn-wait-bin` now propagate that code through the payload process, parent
   launch process, retained wait rows, and `payload.exit ... exit=N` audit.
   Waited child payload `exit-code N` results through `exec-payload` propagate
   the same way.
   Linked `/bin/install-bin NAME ok|error` and
   `/bin/install-payload NAME ready|failed` cross typed source-control syscalls
   and write status-only source images into that overlay for `/bin/*` and
   `/payload/*` descriptors. Linked `/bin/install-bin-media NAME` and
   `/bin/install-payload-media NAME` first read a bounded
   `reovim-source-media-v1` artifact envelope from the kernel source-media block
   target, verify namespace/path/length/checksum against the requested
   descriptor, and then write the enclosed source bytes into the same overlay.
   The old `/bin/proc install-*` compatibility subcommands are retired; source
   installation is now a real `/bin/install-*` user-program path over
   `uapi::source`. This is an admission-path proof for current source loading,
   not a FAT/SD-card filesystem loader. Exec admission also uses the checked
   source-media envelope as an on-demand fallback when a descriptor resolves
   but its source artifact is missing. If offset zero contains a bounded
   `reovim-source-media-catalog-v1`, exec selects the matching catalog entry
   and reads the checked artifact at that entry's block offset; if no catalog
   is present, it falls back to the offset-zero checked artifact. The retained
   exec-load row records `reason=loaded-source-media` when that fallback succeeds.
   Live and dump evidence
   carries source-image loader metadata, and `/proc/execs` records executable
   class as `kind=bin` or `kind=payload`. Exec admission resolves the descriptor
   source path, records `status=error reason=source-not-found`
   for missing artifacts, and validates byte-backed source-image headers and
   bounded op syntax before scheduler/process admission; malformed `/bin` or
   payload artifacts stay in `/proc/execs` as
   `status=error reason=invalid-image` and do not become pending work. Current
   payload source bodies are byte-only artifacts with bounded source-image
   operations for direct stdout writes, direct stderr writes with retained
   `fd-write` rows, typed `process-self`, VFS pseudo-file reads through
   descriptor-shaped open/read/close syscalls, typed `service-ready NAME`
   publication through the retained service table, typed `service-hold` that
   keeps the payload process resident as `block=service` and returns
   `payload.resident` after a `wait-ready` detach, typed `service-exited`
   evidence when a run-to-completion service moves to `exited/process-exited`,
   typed `service-failed` evidence when a killed resident service moves to
   `failed/process-killed`, typed `service-stop` evidence when
   `/bin/service-stop NAME` explicitly stops a resident service and moves its
   row to `stopped/operator-stop`, and typed `service-start` /
   `service-restart` evidence when `/bin/service-start` or
   `/bin/service-restart` launches the retained payload target again,
   scheduled `/bin` child exec
   with wait, scheduled bounded-stdin `/bin` child exec with wait, scheduled child
   payload launch with wait, scheduled child payload spawn without wait,
   scheduled producer-to-consumer
   `/bin` child pipes, scheduled child `/bin` spawn without wait, scheduled child
   spawn with retained-PID wait, scheduled child kill by retained PID,
   scheduled child sleep until explicit ticks, scheduled child sleep plus
   payload-owned bounded tick wait, payload-owned
   `scheduler-tick`, cooperative `yield-now` through the scheduler syscall,
   payload-owned `read-tty-line`, typed `write-service-table` and
   `write-process-table` snapshots, typed `write-boot-profile` and
   `write-device-table` discovery, typed `write-exec-table`,
   `write-pending-exec-table`, and `write-source-table` executable/source
   snapshots, typed `write-scheduler-state`, `write-task-table`,
   `write-wait-table`, and `write-syscall-table` scheduler/process
   diagnostics, and explicit source-image status results.
   Bounded-stdin child exec decodes
   admitted source-image hex into the pending child stdin buffer, and payload
   `pipe-bin` captures one child stdout into another child's pending stdin;
   both reach `/bin/cat` through the same fd-read/stdout path as shell pipes.
   Stdout success remains intentionally unretained per fragment in the bounded
   syscall ring. A payload that exits after spawning live child work leaves that
   child adopted by rootd with retained `process-adopt` syscall and dmesg
   evidence; a payload that ticks a sleeping child and then yields lets the
   scheduler wake and dispatch that argv-bearing child before the payload
   resumes, with process rows retaining child bounded argv metadata.
   A payload that spawns a child payload without waiting can either leave that
   child pending for rootd adoption or yield so the scheduler dispatches the
   child payload before the parent resumes; because no wait was started, this
   path must not emit a `wait-end status=error` row.
   The x86
   launch profile proves `editor-smoke -> server-smoke` nested payload launch
   and an installed `/payload/server-smoke` source override. The x86
   `bundle-launch` profile ships no image payload descriptors, preinstalls
   provider-discovered `/payload/reovim` and `/payload/server-smoke`
   descriptors from the checked exec-bundle catalog, and runs `/bin/reovim` by
   loading `/payload/reovim` from the bundle with `origin=block-bundle`.
   That bundled `reovim` payload then launches and waits on the bundled
   `server-smoke` payload through `exec-payload`, so parent and server payload
   admission both retain `origin=block-bundle`. Real editor/server payload
   execution still waits for block-backed executable bodies and fuller
   scheduler semantics.

   Image `/bin` programs can also create or replace program execution through
   typed process-domain syscalls. Linked `/bin/exec [NAME=VALUE ...] PROGRAM
   [ARG...]` owns same-PID image replacement through `uapi::process::execve` /
   raw `EXECVE`: the caller builds the replacement argv/env records, the exec
   service updates the current process/task image metadata, the replacement
   `/bin` body runs with that retained PID/task, and `/proc/execs`,
   `/proc/syscalls`, retained `exec.path=...`, retained process env rows, and
   `exec-replace` audit rows prove the lifecycle.
   Linked `/bin/spawn [NAME=VALUE ...] PROGRAM [ARG...]` admits a child into
   the same pending exec and scheduler-ready state without waiting. Leading
   child env entries flow through the process-domain `ProcessSpawnRequest`
   record and are retained on the child process row. No_std and x86 transcript
   coverage prove the child remains ready across shell inputs and runs before a
   later just-entered command when FIFO order selects it. When the spawning
   `/bin` process exits before that child runs, rootd adopts the live child in
   both process and scheduler task records, so pending work never retains an
   exited parent. Linked `/bin/wait PID` waits on a retained Reovim process by
   PID, including a child that rootd has adopted after the spawning `/bin`
   process exited. It records typed `wait-begin` / `wait-end` rows and
   completes against ready or already-completed retained work. A successful
   wait keeps the wait record's terminal `child_state`, then marks the consumed
   child process/task rows as diagnostic `reaped` rows so the completion cannot
   be waited twice; it is not POSIX `waitpid`. Linked `/bin/block
   [NAME=VALUE ...] PROGRAM [ARG...]` creates the same child but moves it to
   `blocked` immediately; the child stays retained but absent from the ready
   queue until linked
   `/bin/wake PID` transitions it back to ready. The focused no_std and x86
   transcript proof checks the blocked child, the `process-block` /
   `process-wake` syscall rows, and FIFO execution after wake. Linked
   `/bin/kill PID` terminates retained ready/blocked children through a typed
   `process-kill` syscall, discards their pending executable image, records a
   failed process exit, updates any matching resident-service row to
   `failed/process-killed`, and proves the killed child does not later
   dispatch. `/bin/proc` process-control compatibility is retired, including
   the former `proc exec PROGRAM [ARG...]` helper. `/bin/service-stop NAME` is the service-level control path
   for resident services: it refuses protected or non-started services,
   terminates the retained ready/blocked service process, keeps that process as
   a failed process record, and records `service-stop` plus
   `stopped/operator-stop` service state instead of `service-failed`.
   `/bin/service-start NAME` launches a retained stopped/exited/failed payload
   service target and records `service-start`; `/bin/service-restart NAME`
   stops any live resident instance first, then launches the retained payload
   target and records `service-restart`. Both paths refuse protected
   non-payload targets such as the shell service.
   `/bin/proc pending` and `/proc/pending` expose retained
   pending executable invocations, so blocked or waiting executable images are
   visible before dispatch and disappear after kill cleanup. `/proc/execs` is the
   executable-admission table: it records `argv0`, load status/reason, resolved
   path, `source=`, loader, and `entry_fn` metadata for bounded source-image
   loads today and future media-backed loads later.

   Scheduler yield is cooperative and explicit. In a normal shell transcript
   with no other ready work, `/bin/sched yield` reports `yielded=false`,
   `status=no-peer`, and the selected pid/task while recording a handled
   `yield-now` syscall. When another pending image is ready, the syscall reports
   `status=yielded`, dispatches that scheduler-selected task to completion, and
   then resumes the yielding process; no_std coverage proves this with an older
   `/bin/pwd` process. `/bin/sched sleep TICKS` blocks the current process with
   `block=sleep`, lets ready work dispatch while the current process is asleep,
   advances explicit scheduler ticks until the wake deadline, records
   `process-sleep` / `process-wake`, and only returns after the scheduler has
   dispatched the current process back to `running`. Non-running processes are
   rejected before yield or sleep can requeue them.

   Payload source bodies use the same scheduler surface. `sleep-bin` admits a
   child `/bin` program in a sleeping state, `scheduler-tick` advances the
   scheduler clock and wakes due children, and `yield-now` dispatches the woken
   child before the payload resumes. `sleep-wait-bin` uses the same sleeping
   child admission but waits through the bounded timed-wait syscall, retaining
   payload-owned `wait-begin`, scheduler tick, `process-wake`,
   scheduler-dispatch, and `wait-end` evidence.

## 3. Boot proof model

Logging machine facts is not enough; the system-kernel boot layer must **prove**
required facts before later phases can rely on them. Boot produces a typed
system boot proof (not a user-facing config) before launching the selected
payload.

```text
arch::_start
  -> system-kernel boot
     -> collect raw environment facts
     -> prove required invariants for the selected boot profile
     -> build system boot proof
     -> publish device-inventory messages
     -> start root daemon
     -> exec /bin/init
     -> /bin/init requests shell -> /bin/sh as service target
     -> exec /bin/sh
     -> /bin/sh requests interactive shell/session start
     -> start tty shell
     -> hand system services + proof to the selected payload launch
```

Proofs are **profile-gated**:

| Profile | Required proofs |
|---|---|
| `selftest` | CPU mode, stack/BSS, timer, UART/semihost exit |
| `shell` | CPU mode, timer, tty output, at least one command input path or scripted input harness |
| `headless-diag` | CPU mode, timer, UART TX/RX or diagnostic output path |
| `recovery` | console output, ≥1 input path, block read path if storage repair is enabled |
| `appliance` | timer IRQ, GIC, framebuffer, USB HID keyboard, persistent block/fs, panic output path |

Failure policy:

- A missing **required** proof for the selected profile aborts before the root
  daemon starts that profile's payload. Diagnostic profiles may still enter a
  degraded root shell when their own tty/UART proofs pass.
- Optional devices may enter `Degraded` and still boot if the profile
  allows it.
- Every degraded/failed optional device emits a structured machine event
  with the reason and the profile rule that allowed boot to continue.

Development/diagnostic modes do **not** silently weaken appliance proofs —
they are explicit profiles. `appliance` refuses to boot the editor UI
without framebuffer + USB keyboard + persistent store.

Current #800 shell-only images are diagnostic, not appliance profiles. They
may boot with HDMI/framebuffer output plus PL011 UART input, and they must
state that explicitly in the boot report and `/boot/profile`. A bootline
script is a deterministic test harness, not a live input proof. The lower
USB/xHCI/HID path may be present as a provider, but `usb_keyboard=ready` is
honest only after a physical HID boot-keyboard report reaches the root shell
through the common input decoder. Before that proof, real-machine manual notes
must report `usb_keyboard=unavailable`.

Current dump proof is also explicit about its boundary. `/bin/dump status` and
`/bin/dump snapshot` expose a `reovim-dump-v1` in-memory snapshot with
boot/session identity, image package/version/target/profile identity,
process/service/exec-load/task/syscall/log counts, storage capacity status, and a
checksum. They also retain `last_sync_*` rows plus a `dump-sync:` artifact
section, so operators and host tools can distinguish storage capability from
the last actual flush attempt. The system-kernel `block` service now owns the
diagnostic block target abstraction used by dump sync. Diagnostic writes
require an explicit flush callback before read-back, so a target that cannot
name a durability boundary reports `flush-unavailable` instead of claiming
persistence. On a successful storage target, `dump sync` first proves a
write/flush/read-back pass, retains that result, then writes and verifies a
final artifact containing the retained checked-sync rows. The x86 QEMU image
installs `qemu-diagnostic-dump0`, so its transcript proves a checked write,
flush, and read-back verification through `/bin/dump sync`. The Pi 4 image
still installs no real SD/FAT target, so its dump sync path remains
fail-closed until a real block/fs or removable-media target with flush/barrier
semantics exists. The host `analyze-dump.sh` tool can validate a saved snapshot
text and reject checksum, wrong-image, missing `dump-sync:` section, or
dropped-log failures; `--require-persistent-proof` additionally rejects dumps
that do not report available storage plus a retained checked sync result. This
is not persistent Pi SD-card proof until `dump sync` writes a checked artifact
to card storage, flushes it, and reads it back.
Dump artifacts also carry `service_records` plus a `services:` table for
init-owned service handoff state, and `pending_exec_records` plus a `pending:`
table so post-poweroff analysis can distinguish started services from
loaded/admitted images still waiting for scheduler dispatch.
Scheduler snapshots and dump scheduler sections carry both task identity and
process identity (`current_task`/`current_pid`, `next_ready_task`/
`next_ready_pid`, and ready-queue `task=... pid=...` rows), so post-boot
evidence can correlate scheduling decisions with process records without
inferring that relationship from table order.

## 4. Device lifecycle

A small explicit state machine; avoid Linux-scale hotplug first.

```text
Declared -> Probing -> Present -> Bound -> Ready
                     \-> Absent
                     \-> Failed
          Bound -> Degraded
          Ready -> Lost     (runtime; see §5)
```

- `Declared`: static platform device or bus root from the board profile.
- `Probing`: driver is reading registers/descriptors.
- `Present`: hardware responded; identity known.
- `Bound`: a driver owns the device.
- `Ready`: a subsystem can depend on it.
- `Degraded`: usable with reduced capability (e.g. framebuffer without
  EDID).
- `Absent`/`Failed`: not usable; boot continues only if profile rules
  allow.

Initial device classes: `cpu`, `memory`, `timer`, `irq`, `console`,
`display`, `bus`, `input`, `block`, `fs`, `power`.

### 4.1 Device-info payload

A device message carries enough identity to debug the machine without
guessing — fixed fields plus optional class-specific extensions:

```text
device_id          stable boot-local id
stable_path        e.g. "platform/pcie0/usb0/1-1:hid0"
class              cpu|memory|timer|irq|display|input|block|fs|power|bus
driver             e.g. "gic400", "bcm2711-mailbox-fb", "vl805-xhci", "usb-hid-kbd"
state              declared|probing|present|bound|ready|degraded|absent|failed
vendor device revision     PCI/USB ids where applicable
mmio_base mmio_len irq dma_bits
capacity_bytes             block/memory
width height pitch format  display
error_code reason          failures (bounded static reason label)
profile_required           whether the current profile requires this device
```

Rendered lines (LOG2) read like:

```text
machine.device.ready stable_path=platform/timer0 class=timer driver=arm-generic-timer freq_hz=54000000 irq=30 profile_required=true
machine.device.ready stable_path=platform/fb0 class=display driver=bcm2711-mailbox-fb width=1280 height=720 pitch=5120 format=xrgb8888 profile_required=true
machine.proof.ok profile=appliance required=timer,irq,display,input,block,fs
```

### 4.2 Machine boot event families

Machine-kernel DS12 families, separate from editor `boot.stage.*`,
provisional names, structured (not free-form), rendered by LOG2. Start with
typed event structs per family; a generalized field map waits until the
third independent family needs it.

`machine.boot.profile`, `machine.env.probe.{start,ok,fail}`,
`machine.device.declared`, `machine.device.probe.{start,ok,fail}`,
`machine.driver.bind.{ok,fail}`, `machine.device.{ready,degraded,absent}`,
`machine.proof.{ok,fail}`.

## 5. Connected-device handling (runtime)

Boot proves the *minimum* environment for the profile; runtime connection
handling manages devices that appear, disappear, reset, or change state
after the machine is serving. Boot must not treat every missing optional
device as failure; runtime must not treat every disconnect as a panic. The
profile decides which losses are fatal.

| Device kind | Boot treatment | Runtime treatment |
|---|---|---|
| Required keyboard | present or boot enters recovery/headless | disconnect pauses input + degraded; reconnect can resume |
| Required display | present or boot enters recovery/headless | mode-loss fatal/degraded per profile |
| Persistent root store | must prove mount/commit before editor starts | removal forces read-only/degraded or safe shutdown |
| Extra USB kbd/mouse | optional | hotplug attach/detach |
| Removable storage | optional unless selected as state root | mount only after explicit trust/profile rule |
| Diagnostic serial | optional for appliance, required for diag | attach/detach without affecting editor state |

Runtime state transitions extend the boot machine:

```text
Ready -> Suspended -> Ready
Ready -> Lost -> Reprobing -> Ready
Ready -> Lost -> Removed
Ready -> Failed
Degraded -> Repaired -> Ready
```

**Generation fencing.** Every transition must revoke or fence stale handles
before new input/storage events flow. A reconnected device receives a fresh
generation; queued events from the old generation are discarded. (This is
the same generation rule the Stream device Domain relies on —
`04-Domain-Substrate/06-Device-Domains.md` §6.2.)

Connected-device event families (provisional): `machine.device.{connect,
disconnect}`, `machine.device.reprobe.{start,ok,fail}`,
`machine.device.generation.bump`, `machine.device.handle.revoked`,
`machine.device.policy.blocked`, `machine.storage.readonly`,
`machine.shutdown.device_loss`. Payloads carry `(device_id, generation,
stable_path, class, driver, state_before, state_after, profile_required,
reason, error_code)`, plus bus-local identity for bus devices
(`bus_path, vendor, device, class_code, subclass, protocol`, and a
`serial_hash` — serials are hashed/redacted in public logs).

### 5.1 Input-device rules

Key/mouse events carry `(device_id, generation)` internally until
normalized into reovim raw input; the pipeline drops stale-generation
events. Key state is cleared on disconnect or generation bump (no stuck
modifiers); repeat timers cancelled; reconnect starts a clean HID state
machine. Multiple keyboards feed one normalized stream while logs retain
device identity. Layout is not hard-coded in the HID driver — HID reports
become normalized physical key events first, then layout maps to text/key
commands.

### 5.2 Storage-device rules

Storage is more dangerous than input; loss must be explicit. If the device
backs the state root, loss immediately closes new writes and moves
persistence to `ReadOnlyLostBacking` or starts controlled shutdown, per
profile. Dirty buffers stay in memory — no fake successful save.
Reappearance does **not** auto-resume writes unless the block-identity
proof matches the prior generation and filesystem recovery succeeds.
Removable storage not selected as state root is inert until explicitly
mounted/trusted.

### 5.3 Trust boundary

The system kernel can detect and identify connected devices, but trust
policy is explicit, so "connected-device support" never becomes ambient
authority: the boot profile may allow/deny classes; an unknown HID keyboard
may be accepted for input in appliance mode; unknown storage is never
auto-mounted writable as state; user-identifying descriptors are
redacted/hashed in public logs. This is the machine-layer half of the
device-Domain trust gate (`04-Domain-Substrate/06-Device-Domains.md` §7).

## 6. Scheduler park/wake seam

The archive runtime's `tick()` never blocks (busy-poll); pacing was
external (a tokio interval in the hosted TUI). Under the no_std floor tokio
does not exist — a pacing vacuum in-repo code must fill, on the critical
path of the runtime realization independent of OS-mode. One seam serves all
targets.

- **Mechanism (`arch/`):** one primitive pair `park_until(Option<Instant>)`
  + `unpark`. Linux: futex wait-with-timeout. Bare metal: WFI +
  generic-timer compare (CNTP_CVAL) — the next deadline literally programs
  the compare register. Windows: `WaitOnAddress` timeout.
- **Policy (sched):** a HIGH/LOW split (Linux hrtimer/timer-wheel
  precedent):
  - **HIGH** deadline tier: min-heap of `Instant` deadlines, a
    `next_deadline()` API. Consumers: frame pacing (16 ms moves from the
    external interval into kernel sched), input coalescing, key repeat.
    Precision matters; these fire.
  - **LOW** timeout tier: coarse hashed wheel (~10–50 ms buckets) for
    cancelable timeouts (debounce, autosave, saturator, connection
    timeouts). O(1) insert/cancel; usually canceled; precision irrelevant.
  - **Tickless loop:** `park_until(min(high.next, low.next_bucket))`; events
    wake via `unpark` → input latency = wake cost, not tick cadence (the
    fastest-reaction goal); on Pi, idle = WFI instead of spin.

Staging (rule of three): land the seam + one deadline-ordered timer with
`next_deadline()` (replaces the full-scan timer map) first; split HIGH/LOW
behind the same API only when cancelable-timeout churn is measurable.
Design the API so the split is additive — no caller assumes a single
structure.

## 7. Threaded interrupt handling

IRQ is World-layer, system-kernel-owned, below the platform contract
(`06-ABI/05-Platform-Contract.md`). Interrupts exist only on bare metal;
on Over-OS the host kernel owns them and delivers fd-readable /
futex-wake / syscall-return. The IRQ effect surfaces upward only as
vocabulary already defined here and in the device model:

- **`unpark`** (§6) — "the loop should wake."
- **`poll()` / `Readiness`** (the `kabi/device` Stream plane,
  `04-Domain-Substrate/06-Device-Domains.md §6`) — "data ready."

On bare metal the IRQ top-half sets those signals; on Over-OS the host
fd/futex state sets them. **Same face above the airlock** — the
editor/client core never sees an interrupt.

**Threaded-IRQ: no driver/cdylib code runs in interrupt context.** The
top-half is a tiny fixed system-kernel routine, never loaded code:

```text
HW IRQ (GIC)
  -> system-kernel TOP-HALF  [IRQ context · WCET-bounded · no alloc · no policy]:
        ack -> identify source -> MASK source -> mark pending -> unpark -> EOI
  -> scheduler wakes (park_until returns)            [normal context]
  -> system-kernel BOTTOM-HALF under budget:
        drain device via the DRIVER VTABLE (normal-context poll/service)
        -> produce normalized event / Chunk
  -> editor/client tick() consumes via the SAME event/stream path as Over-OS
```

Consequences:

- **No interrupt-context driver entry.** Drivers are pure normal-context
  vtables; trusted-native cdylib code never runs with interrupts masked.
  This removes the worst hazard of hot-loaded native drivers.
- **Small, fixed top-half → verifiable TCB.** A hand-checked
  `asm!`-terminal axiom; the part that must be proven correct is minimal.
- **NAPI demotion.** Under flood, the bottom-half keeps the source masked
  and polls until drained, then re-unmasks — bounded work under load
  (converts unbounded interrupt rate to bounded drain work at the boundary).
- **Real-time.** Top-half WCET-bounded; bottom-half budgeted per source;
  deadline classes order the drain (input ≈ hard-deadline); priority
  inheritance in system-kernel locks (the Pathfinder lesson).

Code ownership:

| Piece | Owner |
|---|---|
| CPU IRQ primitives — vector-table install, DAIF mask/unmask, vector stub | `arch` |
| IRQ subsystem — GIC programming, source→driver routing, pending-set, threaded top-half, `unpark` | system kernel (board-specific GIC backend) |
| Per-device IRQ work — registers its IRQ line at bind | loadable driver, normal-context only |

**Boundary case.** The interrupt controller (GIC) and timer are **not
hot-unloadable** — you cannot unload the thing routing all interrupts.
Like the loader itself, they are part of the boot mechanism, bound
during machine boot (`irq`/`timer` BootProof requirements for
`appliance`), even though they use the same vtable contract. The "no
compiled-in driver" rule (§2, Machine-kernel subsystems) holds for
*device* drivers (UART/xHCI/GPIO/block); the IRQ controller is
system-kernel mechanism, not a hot-pluggable device.

## Conformance

| Behaviour | Fixture |
|---|---|
| Proof gate | `appliance` boot with a missing required proof (e.g. no framebuffer) aborts before the root daemon starts the editor/server/client payload and emits `machine.proof.fail`. |
| Root-daemon independence | A kernel-shell fixture starts `/bin/init` and `/bin/sh` through exec/process/scheduler/syscall, proves init requested `shell -> /bin/sh` through `init-service-start`, proves `/proc/services` exposes service state, reason, the init requester, and running service PID/task, proves `/bin/sh` requested interactive startup through `session-shell-start`, boots to a tty prompt, accepts basic `/bin` programs (`/bin/help`, `/bin/device`, `/bin/dmesg`, `/bin/launch` status), and has no `reovim-editor-core` dependency. |
| Interactive VNC smoke | The aarch64 framebuffer-console profile can be booted under QEMU VNC; a human or harness types a basic `/bin` program and observes the prompt/response before any editor payload is launched. |
| Degraded optional | An optional device entering `Degraded` boots under a profile that allows it and emits the allowing rule. |
| Generation fence | A keyboard reconnect bumps generation; queued old-generation key events are dropped and modifiers cleared. |
| Storage loss | State-root storage loss closes new writes (no fake save) and moves persistence to read-only or controlled shutdown per profile. |
| Tickless wake | An input event during idle wakes the loop via `unpark` rather than waiting for the next timer deadline. |
| IRQ isolation | No driver code runs in interrupt context; the top-half calls only fixed system-kernel primitives (ack, mask, unpark, EOI). |
