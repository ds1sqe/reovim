//! Bare-metal boot-core payload: boots the REAL reovim kernel on the floor.
//!
//! Where the splash payload proves the framebuffer pipeline with a demo, this
//! payload boots the real kernel on the freestanding floor: it discovers the
//! machine's hardware facts, pushes them into [`Init::boot`] through
//! [`LauncherArgs`], and lets the kernel's boot-stage log stream to the console
//! live (UART, plus the framebuffer on aarch64). This is the `Init` -> `Kernel`
//! handoff running on bare metal instead of a hosted harness — the real-runtime
//! proof of life on the floor.
//!
//! Cross-arch (rule of three: two real arches now exist):
//!
//! - **aarch64** (QEMU raspi4b / BCM2711): installs the framebuffer as a console
//!   sink, then parks in `wfe` afterward so the rendered surface persists for a
//!   QEMU screendump (a manual proof).
//! - **x86_64** (QEMU q35, Multiboot1): UART-only — the boot banner rides
//!   fd 1/2 -> COM1. It exits through the `isa-debug-exit` channel after a
//!   successful boot, so the run is an automatable exit-code pilot.
//!
//! Boot-core needs the platform slots that are real on the freestanding floor —
//! clock, alloc, park, and `file_write` (fd 1/2 -> the floor's UART/console
//! sink); the socket/thread stubs are untouched.
#![no_std]
#![no_main]
// The `entry!` macro expands to `#[unsafe(no_mangle)]` symbols the
// `unsafe_code` lint flags, and the aarch64 park uses inline asm; a bare-metal
// payload is unsafe by nature. The allow is scoped to this fixture bin.
#![allow(unsafe_code)]

use {
    reovim_arch::sys::write,
    reovim_kernel::{Init, LauncherArgs},
    // The device-neutral boot assembly lifted into the system kernel (SP04 04a);
    // it reads the raw register/mailbox/DTB facts through the §11 impl edge. On
    // the freestanding targets the system kernel also supplies the platform
    // vtable installed below (SP04 04c).
    reovim_system_kernel::{collect_boot_info, collect_device_inventory},
};

// The freestanding system-kernel platform provider this composition root
// installs on the bare-metal targets (SP04 04c) — the real product-path
// successor to the `reovim-platform-stub-none` scaffold. Module-gated to
// `target_os = "none"`, matching the system kernel's own gate.
#[cfg(target_os = "none")]
use reovim_system_kernel::platform as system_platform;

#[cfg(target_arch = "aarch64")]
use {
    core::arch::asm,
    // The framebuffer (VideoCore MMIO) STAYS in the raw-mechanism crate; the
    // console / fonts / color that render through it lifted into the system
    // kernel with the rest of the device-neutral library.
    reovim_arch::sys::framebuffer,
    reovim_system_kernel::{color::Color, console, fonts},
};

#[cfg(target_arch = "x86_64")]
use reovim_arch::sys::exit_group;

/// Packs an RGB triple into a `0x00RRGGBB` pixel (RGB pixel-order tag). Only the
/// aarch64 framebuffer path uses it.
#[cfg(target_arch = "aarch64")]
const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Ends the run after the kernel has booted.
///
/// aarch64 parks the core in a `wfe` loop so the rendered framebuffer surface
/// persists for a QEMU screendump. x86 is UART-only — nothing to persist — so it
/// exits through the `isa-debug-exit` channel, making the boot an automatable
/// exit-code pilot. `code` is the exit status on the exiting arch; aarch64
/// ignores it (a parked image reports success by rendering, not by status).
#[cfg(target_arch = "aarch64")]
fn finish(_code: i32) -> ! {
    loop {
        // SAFETY: `wfe` is an unprivileged hint that parks the core until an
        // event; it touches no memory and has no architectural side effect.
        unsafe {
            asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn finish(code: i32) -> ! {
    exit_group(code)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use reovim_arch_floor_linux_x86_64::entry;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use reovim_arch_floor_linux_aarch64::entry;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
use reovim_arch_floor_none_aarch64::entry;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
use reovim_arch_floor_none_x86_64::entry;

entry!(|_argc, _argv, _envp| {
    // Boot ordering — vtable-install → console-install → (kernel boot →) render —
    // enforces the no-read-before-install invariant: every `kabi::handle` read
    // (the kernel's allocs + writes) and every fd 1/2 write runs only after the
    // handle and the console sink are standing.
    //
    // Step 1: install the platform handle as the closure's first statement,
    // before any handle read. The installer is cfg-split: the freestanding
    // SYSTEM-KERNEL provider on `*-unknown-none` (this fixture's real target,
    // where linux-native cannot link — the real product path superseding
    // stub-none, SP04 04c), the real POSIX provider on a hosted Linux build —
    // exactly one links per target.
    #[cfg(not(target_os = "linux"))]
    let _ = system_platform::install_platform();
    #[cfg(target_os = "linux")]
    let _ = reovim_platform_linux_native::install_platform();

    // Step 2 (aarch64): stand up the system kernel's framebuffer console as the
    // floor's fd 1/2 write sink, before the first `write` below. The kernel's
    // boot-stage log (its own fd-2 stderr echo included) then renders on the
    // HDMI surface as well as the UART, drawn through the JetBrains Mono coverage
    // font over the console's retained-content grid. The install is the system
    // kernel's own boot action now — no fixture-side `console::install` glue. If
    // the mailbox alloc or the one-shot screen-grid hand-out fails, the floor
    // stays UART-only.
    #[cfg(target_arch = "aarch64")]
    if let Some((fb, grid)) = framebuffer::init().zip(console::screen_grid()) {
        console::install_console(
            fb,
            &fonts::JETBRAINS_MONO,
            Color::Rgb(rgb(0xC8, 0xE0, 0xFF)),
            Color::Rgb(rgb(0x0A, 0x14, 0x28)),
            grid,
        );
    }

    // Name the installed provider in the boot log so the boot proves WHICH
    // vtable is standing — the freestanding system-kernel provider on bare metal
    // (the dual-path proof: bootcore installs the system-kernel vtable, not
    // stub-none), the linux-native provider on a hosted build.
    #[cfg(not(target_os = "linux"))]
    let _ = write(1, b"\nreovim kernel boot on bare metal [provider: system-kernel]\n");
    #[cfg(target_os = "linux")]
    let _ = write(1, b"\nreovim kernel boot on bare metal [provider: linux-native]\n");

    // Discover the machine's real hardware facts (RAM / CPU) and push them into
    // the kernel at entry through `LauncherArgs.boot_info`. The boot-tail
    // diagnostics banner prints them on the live console.
    let boot_info = collect_boot_info();
    let device_inventory = collect_device_inventory();
    let args = LauncherArgs {
        boot_info,
        device_inventory,
        ..LauncherArgs::default()
    };

    // Boot the REAL reovim kernel on the freestanding floor. Its boot-stage log
    // streams to the console live via the kernel's own stderr echo (fd 2 ->
    // file_write -> the floor's write fan-out) — no manual ring drain.
    let Ok(_kernel) = Init::new(args).boot() else {
        let _ = write(1, b"kernel boot FAILED\n");
        finish(70);
    };

    let _ = write(1, b"kernel booted\n");
    finish(0);
});
