# Linux Architecture Reference

This document describes the Linux system concepts that inform reovim's architecture. Reovim follows a **Linux kernel-inspired design** with clear separation between kernel mechanisms, drivers, and loadable modules.

## Overview

Reovim mirrors Linux's architecture to achieve:

- **Mechanism vs Policy separation** - Kernel provides WHAT (traits, APIs), modules decide HOW (keybindings, behavior)
- **Modularity** - Small, focused components that combine
- **Scalability** - Architecture designed for large files and complex operations
- **Minimal latency** - Prioritize instant response to user input

## Module/Driver Model

### Module Loading Architecture

Linux uses `modprobe` and `libkmod` to load kernel modules with automatic dependency resolution:

```
┌─────────────────────────────────────────────────────────────┐
│                    USERSPACE                                 │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ modprobe mymodule                                    │    │
│  │    │                                                 │    │
│  │    ▼                                                 │    │
│  │ libkmod                                              │    │
│  │  ├─► Read /lib/modules/$(uname -r)/modules.dep      │    │
│  │  ├─► Resolve dependencies (load deps first)         │    │
│  │  ├─► Read /etc/modprobe.d/*.conf for options        │    │
│  │  └─► Call init_module(2) syscall                    │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    KERNEL                                    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ init_module syscall handler                          │    │
│  │  ├─► Allocate memory for module                      │    │
│  │  ├─► Copy module image from userspace                │    │
│  │  ├─► Resolve symbols (EXPORT_SYMBOL)                 │    │
│  │  ├─► Call module_init() function                     │    │
│  │  └─► Add to loaded modules list                      │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

**Mapping to Reovim:**

| Linux Component | Reovim Equivalent |
|-----------------|-------------------|
| `modprobe` | Module loader |
| `libkmod` | `ModuleRegistry` |
| `modules.dep` | Dependency resolver |
| `EXPORT_SYMBOL` | Trait implementations |
| `init_module()` | `Module::init()` |
| `cleanup_module()` | `Module::cleanup()` |

### Driver-Device Binding

Linux drivers bind to devices through a match/probe mechanism:

```
┌─────────────────────────────────────────────────────────────┐
│                      BUS SUBSYSTEM                          │
│  ┌──────────────┐     ┌──────────────┐                     │
│  │ Device List  │     │ Driver List  │                     │
│  │  ├─ dev1     │     │  ├─ drv1     │                     │
│  │  ├─ dev2     │     │  ├─ drv2     │                     │
│  │  └─ dev3     │     │  └─ drv3     │                     │
│  └──────────────┘     └──────────────┘                     │
│         │                    │                              │
│         └───────┬────────────┘                              │
│                 ▼                                           │
│         ┌──────────────┐                                    │
│         │ MATCH LOGIC  │ (device_id ↔ driver_id_table)     │
│         └──────────────┘                                    │
│                 │                                           │
│                 ▼ match found                               │
│         ┌──────────────┐                                    │
│         │ driver.probe()│ → verify device, alloc resources │
│         └──────────────┘                                    │
│                 │                                           │
│                 ▼ success                                   │
│         Device bound to Driver                              │
└─────────────────────────────────────────────────────────────┘
```

**Binding Events:**

1. `device_register()` → Kernel iterates driver list, calls `match()` + `probe()`
2. `driver_register()` → Kernel iterates device list, calls `match()` + `probe()`
3. **Deferred Probe**: If resource not ready, return `-EPROBE_DEFER` → retry later

**Mapping to Reovim:**

| Linux | Reovim |
|-------|--------|
| Bus | Registry (e.g., `ModeRegistry`) |
| Device | Resource (e.g., Buffer, Window) |
| Driver | Handler (e.g., Mode, Command) |
| `probe()` | `Handler::attach()` |
| `remove()` | `Handler::detach()` |

### Symbol Export and Dependencies

Modules export symbols for other modules to use:

```
Module A (exports symbols):
┌─────────────────────────────────────────────────────────────┐
│ EXPORT_SYMBOL(helper_function);                             │
│ EXPORT_SYMBOL_GPL(internal_api);  // GPL-only              │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼ symbols added to kernel table

Module B (uses symbols):
┌─────────────────────────────────────────────────────────────┐
│ extern void helper_function(void);  // declared            │
│                                                             │
│ void my_init() {                                            │
│     helper_function();  // resolved at load time           │
│ }                                                           │
└─────────────────────────────────────────────────────────────┘
```

**Reovim Equivalent:**

- **Traits** = Exported symbols (contract)
- **Implementations** = Module provides functionality
- **Registry** = Symbol table (lookup by ID)
- **Load order** = Dependency resolution at startup

### Module Lifecycle

```c
// Linux Pattern
module_init(my_init);    // Entry point macro
module_exit(my_cleanup); // Exit point macro

static int __init my_init(void) {
    // 1. Allocate resources
    // 2. Register with subsystems
    // 3. Return 0 on success, negative on error
    return 0;
}

static void __exit my_cleanup(void) {
    // 1. Unregister from subsystems
    // 2. Free resources
    // (reverse order of init)
}
```

**Reovim Module Lifecycle:**

```rust
pub trait Module: Send + Sync {
    fn id(&self) -> ModuleId;
    fn dependencies(&self) -> &[ModuleId];

    fn init(&mut self, ctx: &mut KernelContext) -> Result<()>;
    fn cleanup(&mut self, ctx: &mut KernelContext);
}
```

## Process & Service Model

### Syscall Mechanism (x86_64)

Modern Linux uses the `syscall` instruction for system calls:

```
Userspace                    Kernel
────────────────────────────────────────────────────

  syscall number → RAX
  args → RDI, RSI, RDX,
         R10, R8, R9
           │
           ▼
  ┌─────────────────┐
  │ SYSCALL instr   │────────► IA32_LSTAR MSR points to
  └─────────────────┘          entry_SYSCALL_64
           │                           │
           │                           ▼
           │                 ┌─────────────────────┐
           │                 │ sys_call_table[RAX] │
           │                 │ dispatch to handler │
           │                 └─────────────────────┘
           │                           │
           ◄───────────────────────────┘
      SYSRET returns
      result in RAX
```

**Key Points:**

- Modern `syscall` instruction (not legacy `int 0x80`) for x86_64
- Syscall number in RAX, up to 6 args in registers
- Kernel destroys RCX and R11 (saves RIP and RFLAGS)
- `sys_call_table` array dispatches to handler functions

### Boot Process & PID 1

```
┌─────────────────────────────────────────────────────────────┐
│ 1. BIOS/UEFI                                                │
│    └─► Load bootloader (GRUB)                               │
├─────────────────────────────────────────────────────────────┤
│ 2. Bootloader                                               │
│    └─► Load kernel + initramfs into memory                  │
├─────────────────────────────────────────────────────────────┤
│ 3. Kernel                                                   │
│    ├─► Decompress, initialize hardware                      │
│    ├─► Mount root filesystem                                │
│    └─► Execute /sbin/init (PID 1)                          │
├─────────────────────────────────────────────────────────────┤
│ 4. Init System (systemd)                                    │
│    ├─► Becomes PID 1, immortal root of process tree        │
│    ├─► Reads unit files, resolves dependencies              │
│    ├─► Starts services in parallel                          │
│    └─► Reaches target (multi-user.target, graphical.target)│
└─────────────────────────────────────────────────────────────┘
```

**PID 1 Special Properties:**

- Cannot be killed by normal signals (kernel protection)
- Automatically adopts orphaned processes
- Responsible for reaping zombie processes
- If PID 1 dies, kernel panics

### systemd Service Management

**Service Types:**

| Type | Description |
|------|-------------|
| `simple` | Service starts immediately, no forking |
| `forking` | Traditional daemon: forks, parent exits |
| `notify` | Uses `sd_notify()` to signal readiness |
| `oneshot` | Runs once, then exits (scripts) |

**Socket Activation:**

```
┌─────────────────────────────────────────────────────────────┐
│ Traditional Daemon                                          │
│   Service starts → binds socket → waits for connections    │
│   (Service running even with zero clients)                  │
├─────────────────────────────────────────────────────────────┤
│ Socket Activation                                           │
│   systemd binds socket → connection arrives →              │
│   systemd starts service → passes socket FD                │
│   (Zero-cost idle, on-demand startup)                      │
└─────────────────────────────────────────────────────────────┘
```

**Watchdog:**

- `WatchdogSec=` in unit file enables health monitoring
- Service must call `sd_notify("WATCHDOG=1")` periodically
- If heartbeat missed → service restarted

### Orphan Process Handling

```
Traditional SysV:
  bash shell (session leader)
      │
      └── daemon process (fork → parent exits)
              │
              └── Becomes orphan → adopted by PID 1
                  (But SysV can't track termination!)

systemd:
  systemd (PID 1)
      │
      └── service process (Type=simple or notify)
              │
              └── If crashes → systemd detects via cgroups
                  → Restart=on-failure triggers restart
```

**Why This Matters:**

- systemd tracks ALL processes in a service's cgroup
- Even "double-fork" daemon patterns are tracked
- Crash detection and automatic restart work reliably

## I/O & Communication

### epoll Multiplexing

Linux's `epoll` provides O(1) I/O multiplexing:

```
┌─────────────────────────────────────────────────────────────┐
│                      Kernel Space                           │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ eventpoll structure                                  │   │
│  │  ├── Red-Black Tree: all monitored FDs              │   │
│  │  ├── Ready List: FDs with pending I/O               │   │
│  │  └── Wait Queue: blocked epoll_wait() callers       │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
        ▲                              │
        │ epoll_ctl(ADD/MOD/DEL)       │ epoll_wait()
        │ O(log n) - RB tree           │ O(1) - ready list
        │                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      User Space                             │
│  tokio runtime uses epoll internally                        │
└─────────────────────────────────────────────────────────────┘
```

**Performance Comparison:**

| FDs | select/poll | epoll |
|-----|-------------|-------|
| 10 | 0.7ms | 0.4ms |
| 1,000 | 35ms | 0.5ms |
| 10,000 | 930ms | 0.7ms |

**Triggering Modes:**

- **Level-Triggered**: Returns ready while condition holds (simpler)
- **Edge-Triggered**: Returns ready only on state change (efficient, needs non-blocking I/O)
- tokio uses edge-triggered by default

### Signal Handling

**Key Signals for Servers:**

| Signal | Default | Server Should | Reovim Action |
|--------|---------|---------------|---------------|
| SIGTERM | Exit | Graceful stop | Set shutdown flag, drain |
| SIGINT | Exit | Graceful stop | Same as SIGTERM |
| SIGHUP | Exit | Reload config | (Future: reload) |
| SIGPIPE | Exit | Ignore | Ignore (client disconnect) |
| SIGCHLD | Ignore | Reap children | tokio handles |

**SA_RESTART Behavior:**

- Syscalls interrupted by signals can auto-restart or return EINTR
- With `SA_RESTART`: syscall restarts automatically (convenient)
- Without: returns EINTR, must retry (more control)
- `epoll_wait()` NEVER restarts (always returns EINTR on signal)

**Signal-Safe Functions:**

- Inside signal handler, only async-signal-safe functions allowed
- Safe: `write()`, `_exit()`, `sigaction()`, simple assignments
- UNSAFE: `printf()`, `malloc()`, anything with locks
- Pattern: Set `volatile sig_atomic_t` flag, check in main loop

### TTY/Session Management

**Why Server Model (not direct TTY):**

| Traditional Editor | Server Model |
|-------------------|--------------|
| Bound to controlling TTY | No controlling TTY (`setsid()`) |
| SIGHUP on disconnect → dies | Survives disconnect |
| Single client only | Multi-client support |
| Signals via TTY driver | Messages via socket |

**tmux-style Implementation Pattern:**

```
Client 1 ───socket───┐
                     ├──→ Server Daemon (no TTY) ───→ Sessions
Client 2 ───socket───┘        │
                              └─ setsid() at startup
                              └─ Explicit IPC, not signals
                              └─ State survives client disconnect
```

**Key Design Rules:**

1. **No controlling terminal** - Server must call `setsid()` or equivalent
2. **Socket communication** - All client-server interaction via TCP/Unix socket
3. **Explicit events** - Resize, focus, etc. sent as RPC messages (not SIGWINCH)
4. **Session persistence** - State lives in server, not tied to client lifetime

## Mapping to Reovim

### Component Mapping

| Linux Layer | Component | Reovim Equivalent |
|-------------|-----------|-------------------|
| **Kernel** | Core + syscalls | `lib/kernel/` + `api/` |
| **Subsystems** | VFS, Net, Block | Registries, EventBus |
| **Drivers** | Device drivers | `lib/drivers/` (traits) |
| **Modules** | Loadable .ko files | `modules/` (implementations) |
| **udev** | Device manager | Session event dispatcher |
| **modprobe** | Module loader | Module loader (future) |
| **sysfs** | /sys interface | State queries via RPC |

### Design Principles

| Principle | Linux | Reovim |
|-----------|-------|--------|
| **Mechanism vs Policy** | Kernel provides primitives | Kernel provides traits, modules implement |
| **Do one thing well** | Each subsystem focused | Each crate has single responsibility |
| **Composability** | Modules combine | Drivers + modules combine |
| **API purity** | Syscall interface | `api/` module boundary |

## References

### Syscall & Kernel Internals

- [The Definitive Guide to Linux System Calls](https://blog.packagecloud.io/the-definitive-guide-to-linux-system-calls/)
- [Linux Inside - How the kernel handles syscalls](https://0xax.gitbooks.io/linux-insides/content/SysCall/linux-syscall-2.html)
- [syscall(2) man page](https://man7.org/linux/man-pages/man2/syscall.2.html)
- [Linux System Call Table for x86_64](https://blog.rchapman.org/posts/Linux_System_Call_Table_for_x86_64/)

### Boot Process & Init Systems

- [Boot Process with Systemd in Linux](https://www.geeksforgeeks.org/linux-unix/boot-process-with-systemd-in-linux/)
- [The Linux Process Journey - PID 1](https://medium.com/@boutnaru/the-linux-process-journey-pid-1-init-60765a069f17)
- [Systemd: Zero to Hero](https://blog.alphabravo.io/systemd-zero-to-hero-part-1-understanding-the-modern-linux-init-system/)
- [Init vs Systemd](https://cycle.io/learn/init-vs-systemd)

### systemd Service Management

- [systemd.io - Official Documentation](https://systemd.io/)
- [systemd.service man page](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html)
- [systemd.socket man page](https://www.freedesktop.org/software/systemd/man/latest/systemd.socket.html)
- [Systemd Socket Activation Explained](https://ilmanzo.github.io/post/systemd-socket-activated-services/)
- [Daemon Management Under Systemd (USENIX)](https://www.usenix.org/system/files/login/articles/login_june_06_jedrzejewski-szmek.pdf)

### I/O Multiplexing (epoll)

- [Mastering epoll - High-Performance Networking](https://medium.com/@m-ibrahim.research/mastering-epoll-the-engine-behind-high-performance-linux-networking-85a15e6bde90)
- [epoll(7) man page](https://man7.org/linux/man-pages/man7/epoll.7.html)
- [The Implementation of epoll](https://idndx.com/the-implementation-of-epoll-1/)
- [Async IO on Linux - Julia Evans](https://jvns.ca/blog/2017/06/03/async-io-on-linux--select--poll--and-epoll/)

### Signal Handling

- [signal(7) man page](https://man7.org/linux/man-pages/man7/signal.7.html)
- [signal-safety(7) man page](https://man7.org/linux/man-pages/man7/signal-safety.7.html)
- [When Are System Calls Interrupted?](https://linuxvox.com/blog/when-and-how-are-system-calls-interrupted/)

### Module Loading & kmod

- [The Linux Kernel Module Programming Guide](https://sysprog21.github.io/lkmpg/)
- [Kernel module - ArchWiki](https://wiki.archlinux.org/title/Kernel_module)
- [kmod(8) man page](https://man7.org/linux/man-pages/man8/kmod.8.html)
- [depmod(8) man page](https://man7.org/linux/man-pages/man8/depmod.8.html)
- [Linux Loadable Kernel Module HOWTO](https://tldp.org/HOWTO/html_single/Module-HOWTO/)

### Driver Model & Device Binding

- [Linux Device Model Documentation](https://docs.kernel.org/driver-api/driver-model/overview.html)
- [Driver Binding Documentation](https://docs.kernel.org/driver-api/driver-model/binding.html)
- [Device Driver Design Patterns](https://docs.kernel.org/driver-api/driver-model/design-patterns.html)
- [Platform Devices and Drivers](https://docs.kernel.org/driver-api/driver-model/platform.html)

### Symbol Export & Dependencies

- [EXPORT_SYMBOL Tutorial](https://lkw.readthedocs.io/en/latest/doc/04_exporting_symbols.html)
- [Symbol Namespaces Documentation](https://docs.kernel.org/core-api/symbol-namespaces.html)
- [The Kernel Symbol Table](https://www.oreilly.com/library/view/linux-device-drivers/0596000081/ch02s03.html)

### udev & Hotplug

- [udev - ArchWiki](https://wiki.archlinux.org/title/Udev)
- [udev - Wikipedia](https://en.wikipedia.org/wiki/Udev)
- [How to Use Udev for Device Detection](https://www.tecmint.com/udev-for-device-detection-management-in-linux/)
- [Hotplugging with udev (Bootlin)](https://bootlin.com/doc/legacy/udev/udev.pdf)

### TTY & Session Management

- [setsid(2) man page](https://www.man7.org/linux/man-pages/man2/setsid.2.html)
- [When to Use setsid() in Linux](https://linuxvox.com/blog/when-is-setsid-useful-or-why-do-we-need-to-group-processes-in-linux/)
- [The TTY demystified](https://www.linusakesson.net/programming/tty/)
