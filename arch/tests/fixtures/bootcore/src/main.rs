//! Bare-metal boot-core payload: boots the REAL reovim editor core on the floor.
//!
//! Where the splash payload proves the framebuffer pipeline with a demo, this
//! payload boots the real editor core on the freestanding floor: it discovers the
//! machine's hardware facts, pushes them into [`EditorInit::boot`] through
//! [`LauncherArgs`], and lets the editor core's boot-stage log stream to the console
//! live (UART, plus the framebuffer on aarch64). This is the `EditorInit` -> `EditorCore`
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
    reovim_editor_core::{EditorInit, LauncherArgs},
};

#[cfg(not(target_os = "linux"))]
use reovim_platform_stub_none as none_platform;

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
use {
    core::{arch::asm, cell::UnsafeCell},
    // The framebuffer (VideoCore MMIO) STAYS in the raw-mechanism crate; the
    // console / fonts / color render policy lives in the system-kernel bridge.
    reovim_arch::sys::framebuffer,
    reovim_system_kernel::{
        color::Color,
        console::{self, RenderSurface},
        fonts, inventory,
    },
};

#[cfg(target_os = "none")]
use reovim_system_kernel::boot_info::{self, BootFacts};

#[cfg(target_arch = "x86_64")]
use reovim_arch::sys::exit_group;

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
use {
    core::cell::UnsafeCell,
    reovim_arch::sys::{EMPTY_MEMORY_RANGE, MEMORY_STORAGE_ENTRIES, MemoryRange},
};

/// Packs an RGB triple into a `0x00RRGGBB` pixel (RGB pixel-order tag). Only the
/// aarch64 framebuffer path uses it.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
struct FbStore(UnsafeCell<Option<framebuffer::Framebuffer>>);

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
// SAFETY: the freestanding boot fixture is a single thread of control.
unsafe impl Sync for FbStore {}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
static FRAMEBUFFER: FbStore = FbStore(UnsafeCell::new(None));

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn fb_put_pixel(_ctx: usize, x: u32, y: u32, color: u32) {
    // SAFETY: the framebuffer is installed once before the callback is
    // registered; the boot fixture is single-threaded.
    if let Some(fb) = unsafe { &*FRAMEBUFFER.0.get() } {
        fb.put_pixel(x, y, color);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn fb_clear(_ctx: usize, color: u32) {
    // SAFETY: same one-shot framebuffer ownership as `fb_put_pixel`.
    if let Some(fb) = unsafe { &*FRAMEBUFFER.0.get() } {
        fb.clear(color);
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn install_framebuffer_console(fb: framebuffer::Framebuffer, grid: console::ScreenGrid<'static>) {
    let width = fb.width();
    let height = fb.height();
    // SAFETY: one-shot install before any callback use.
    unsafe {
        *FRAMEBUFFER.0.get() = Some(fb);
    }
    let surface = RenderSurface::new(width, height, 0, fb_put_pixel, fb_clear);
    console::install_console(
        surface,
        &fonts::JETBRAINS_MONO,
        Color::Rgb(rgb(0xC8, 0xE0, 0xFF)),
        Color::Rgb(rgb(0x0A, 0x14, 0x28)),
        grid,
    );
    reovim_arch::sys::install_write_sink(console::write_bytes);
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
struct DeviceStore(UnsafeCell<[inventory::DeviceEntry; inventory::DEVICE_STORAGE_ENTRIES]>);

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
// SAFETY: the boot fixture is single-threaded and hands out the storage once.
unsafe impl Sync for DeviceStore {}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
static DEVICES: DeviceStore = DeviceStore(UnsafeCell::new(
    [inventory::EMPTY_DEVICE_ENTRY; inventory::DEVICE_STORAGE_ENTRIES],
));

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn device_storage() -> &'static mut [inventory::DeviceEntry] {
    // SAFETY: one-shot boot inventory assembly; no aliasing after handoff.
    unsafe { &mut *DEVICES.0.get() }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
fn dtb_bytes() -> &'static [u8] {
    let ptr = reovim_arch::sys::dtb_ptr();
    if ptr == 0 {
        return &[];
    }
    let p = ptr as *const u8;
    // SAFETY: the firmware DTB pointer is identity-mapped and readable for the
    // process lifetime on this boot target; only the fixed header is read here.
    let total = unsafe {
        let hdr = core::slice::from_raw_parts(p, 8);
        u32::from_be_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize
    };
    if !(40..=inventory::MAX_DTB_BYTES).contains(&total) {
        return &[];
    }
    // SAFETY: `total` was read from the FDT header and capped above before the
    // full process-lifetime slice is formed.
    unsafe { core::slice::from_raw_parts(p, total) }
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
struct MemoryStore(UnsafeCell<[MemoryRange; MEMORY_STORAGE_ENTRIES]>);

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
// SAFETY: the freestanding x86 boot fixture is single-threaded.
unsafe impl Sync for MemoryStore {}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
static MEMORY: MemoryStore =
    MemoryStore(UnsafeCell::new([EMPTY_MEMORY_RANGE; MEMORY_STORAGE_ENTRIES]));

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
fn memory_storage() -> &'static mut [MemoryRange] {
    // SAFETY: one-shot boot-info assembly; no aliasing after handoff.
    unsafe { &mut *MEMORY.0.get() }
}

/// Ends the run after the editor core has booted.
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

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use reovim_arch_floor_linux_aarch64::entry;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use reovim_arch_floor_linux_x86_64::entry;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
use reovim_arch_floor_none_aarch64::entry;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
use reovim_arch_floor_none_x86_64::entry;

entry!(|_argc, _argv, _envp| {
    // Boot ordering — vtable-install → console-install → (editor-core boot →) render —
    // enforces the no-read-before-install invariant: every `kabi::handle` read
    // (the editor core's allocs + writes) and every fd 1/2 write runs only after the
    // handle and the console sink are standing.
    //
    // Step 1: install the platform handle as the closure's first statement,
    // before any handle read. The installer is cfg-split: the freestanding
    // stub-none provider on `*-unknown-none`, the real POSIX provider on a
    // hosted Linux build — exactly one links per target.
    #[cfg(not(target_os = "linux"))]
    let _ = none_platform::install_platform();
    #[cfg(target_os = "linux")]
    let _ = reovim_platform_linux_native::install_platform();
    let _ = reovim_system_kernel::mm::install_lib_ds_alloc_backend();
    let _ = reovim_system_kernel::sched::install_lib_ds_sync_backend();

    // Step 2 (aarch64): stand up the system kernel's framebuffer console as the
    // floor's fd 1/2 write sink, before the first `write` below. The editor core's
    // boot-stage log (its own fd-2 stderr echo included) then renders on the
    // HDMI surface as well as the UART, drawn through the JetBrains Mono coverage
    // font over the console's retained-content grid. The install is the system
    // system kernel's own boot action now — no fixture-side `console::install` glue. If
    // the mailbox alloc or the one-shot screen-grid hand-out fails, the floor
    // stays UART-only.
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    if let Some((fb, grid)) = framebuffer::init().zip(console::screen_grid()) {
        install_framebuffer_console(fb, grid);
    }

    // Name the installed provider in the boot log so the boot proves WHICH
    // vtable is standing — the freestanding stub-none provider on bare metal,
    // the linux-native provider on a hosted build.
    #[cfg(not(target_os = "linux"))]
    let _ = write(1, b"\nreovim editor-core boot on bare metal [provider: stub-none]\n");
    #[cfg(target_os = "linux")]
    let _ = write(1, b"\nreovim editor-core boot on bare metal [provider: linux-native]\n");

    // Discover the machine's real hardware facts (RAM / CPU) and push them into
    // the editor core at entry through `LauncherArgs.boot_info`. The boot-tail
    // diagnostics banner prints them on the live console.
    let boot_info = {
        #[cfg(target_os = "linux")]
        {
            Default::default()
        }
        #[cfg(all(target_os = "none", target_arch = "aarch64"))]
        {
            let cache = reovim_arch::sys::cache_geometry();
            boot_info::collect_boot_info(BootFacts {
                memory: reovim_arch::sys::discover_memory(),
                cpu_freq_hz: reovim_arch::sys::timer_frequency(),
                cpu_id: reovim_arch::sys::cpu_id(),
                cpu_count: 1,
                cache_line_bytes: cache.cache_line_bytes,
                l1d_bytes: cache.l1d_bytes,
                l1i_bytes: cache.l1i_bytes,
                l2_bytes: cache.l2_bytes,
                cpu_affinity: reovim_arch::sys::cpu_affinity(),
                mem_freq_hz: reovim_arch::sys::sdram_clock_hz().unwrap_or(0),
                heap_total_bytes: reovim_arch::sys::arena_capacity() as u64,
            })
        }
        #[cfg(all(target_os = "none", target_arch = "x86_64"))]
        {
            // SAFETY: the floor stashed the Multiboot pointer before entering
            // Rust; q35 keeps the info structure and mmap identity-mapped here.
            let memory = unsafe { reovim_arch::sys::discover_memory(memory_storage()) };
            boot_info::collect_boot_info(BootFacts {
                memory,
                cpu_id: reovim_arch::sys::cpu_id(),
                cpu_count: 1,
                cpu_freq_hz: reovim_arch::sys::timer_frequency(),
                ..BootFacts::default()
            })
        }
    };
    let device_inventory = {
        #[cfg(target_os = "linux")]
        {
            Default::default()
        }
        #[cfg(all(target_os = "none", target_arch = "aarch64"))]
        {
            inventory::collect_device_inventory(
                dtb_bytes(),
                device_storage(),
                reovim_arch::sys::classify_device_compatible,
            )
        }
        #[cfg(all(target_os = "none", target_arch = "x86_64"))]
        {
            Default::default()
        }
    };
    let args = LauncherArgs {
        boot_info,
        clock: reovim_system_kernel::sched::clock_control(),
        device_inventory,
        log: reovim_system_kernel::log::log_sink_control(),
        panic: reovim_system_kernel::panic::panic_control(),
        thread: reovim_system_kernel::sched::thread_control(),
        ..LauncherArgs::default()
    };

    // Boot the REAL reovim editor core on the freestanding floor. Its boot-stage log
    // streams to the console live via the editor core's own stderr echo (fd 2 ->
    // file_write -> the floor's write fan-out) — no manual ring drain.
    let Ok(_editor_core) = EditorInit::new(args).boot() else {
        let _ = write(1, b"editor core boot FAILED\n");
        finish(70);
    };

    let _ = write(1, b"editor core booted\n");
    finish(0);
});
