//! `VideoCore` mailbox framebuffer (BCM2711 / QEMU raspi4b).
//!
//! The Pi 4 has no separate display controller the ARM cores program
//! directly: the framebuffer is owned by the `VideoCore` GPU, and the ARM
//! side requests one over the *mailbox* property interface (channel 8).
//! This module drives that interface to allocate a 1280x720x32 surface and
//! exposes a minimal draw surface ([`Framebuffer`]) over the returned
//! base/pitch — enough for the bare-metal splash to put unmistakable pixels
//! on screen. No font, no double-buffer, no GPU command stream: the request
//! is one property batch and the draw is direct stores to the surface.
//!
//! Mailbox MMIO lives at `peripheral_base` + `0xB880` (`0xFE00_B880`), in the
//! device block of the boot MMU map (`start.rs`), so volatile access is
//! correct without further setup. QEMU's `raspi4b` model honors the FB
//! allocate/pitch tags the same way real firmware does.

use core::{
    arch::asm,
    ptr::{read_volatile, write_volatile},
};

/// Mailbox base on BCM2711 (low-peripheral mode): `peripheral_base` `0xFE00_0000`
/// + `0xB880`.
const MBOX_BASE: usize = 0xFE00_B880;
/// `MBOX_READ` — ARM reads `VideoCore` responses here (offset 0x00).
const MBOX_READ: usize = 0x00;
/// `MBOX_STATUS` — full/empty flags (offset 0x18).
const MBOX_STATUS: usize = 0x18;
/// `MBOX_WRITE` — ARM posts requests here (offset 0x20).
const MBOX_WRITE: usize = 0x20;
/// `MBOX_STATUS` bit 31: the write FIFO is full (cannot post).
const MBOX_FULL: u32 = 1 << 31;
/// `MBOX_STATUS` bit 30: the read FIFO is empty (no response yet).
const MBOX_EMPTY: u32 = 1 << 30;
/// The property-tag channel (the low 4 bits of a mailbox message).
const MBOX_CH_PROP: u32 = 8;

/// Property request/response success marker the `VideoCore` writes back into
/// the buffer's request-code slot (`buffer[1]`).
const MBOX_RESP_SUCCESS: u32 = 0x8000_0000;

/// The GPU returns a *bus* address (the high `0xC000_0000` alias). Masking to
/// the low 30 bits yields the ARM-physical address the cores store through.
const BUS_TO_PHYS_MASK: u32 = 0x3FFF_FFFF;

/// Requested surface geometry. Fixed for the splash; the request is rejected
/// (or clamped) by firmware if unsupported, which the success check catches.
const FB_WIDTH: u32 = 1280;
const FB_HEIGHT: u32 = 720;
const FB_DEPTH: u32 = 32;
/// Pixel-order tag value that yields an RGB-ordered surface, so a
/// `0x00RRGGBB` pixel from the draw code displays with correct red and blue.
///
/// The Raspberry Pi firmware doc nominally assigns `0` = BGR, `1` = RGB, but
/// QEMU's `raspi4b` model honors the tag with **inverted** value semantics:
/// verified by QMP screendump, requesting `0` is what produces RGB output here
/// (with `1`, red and blue come out swapped). The fix is the tag value, not the
/// draw packing — the surface is genuinely RGB-ordered, so `put_pixel` keeps
/// its logical `0x00RRGGBB` contract and the color model needs no swap. If this
/// is ever run on real `VideoCore` firmware, re-confirm the value, as the
/// hardware convention may differ from QEMU's.
const FB_PIXEL_ORDER_RGB: u32 = 0;

// Property tag identifiers (`VideoCore` mailbox property interface).
const TAG_SET_PHYS_WH: u32 = 0x0004_8003;
const TAG_SET_VIRT_WH: u32 = 0x0004_8004;
const TAG_SET_DEPTH: u32 = 0x0004_8005;
const TAG_SET_PIXEL_ORDER: u32 = 0x0004_8006;
const TAG_ALLOCATE_BUFFER: u32 = 0x0004_0001;
const TAG_GET_PITCH: u32 = 0x0004_0008;
const TAG_END: u32 = 0x0000_0000;

/// Property tag: get the ARM-side memory region. Response is two words — base
/// address and size in bytes. The ARM-memory tag reports an ARM-physical base
/// (not a bus alias), so no bus→phys masking applies to its result.
const TAG_GET_ARM_MEMORY: u32 = 0x0001_0005;

/// Property tag: get a clock's rate in Hz. The two-word value slot carries the
/// clock id on the way in and the rate on the way out; a rate of 0 means the
/// clock does not exist or is not reported.
const TAG_GET_CLOCK_RATE: u32 = 0x0003_0002;

/// Clock id for the SDRAM controller, the [`TAG_GET_CLOCK_RATE`] selector whose
/// rate is the memory-bus frequency.
const CLOCK_ID_SDRAM: u32 = 0x0000_0008;

/// The property buffer, 16-byte aligned (the mailbox address carries the
/// channel in its low 4 bits, so the buffer must be 16-aligned for those bits
/// to be free). The GPU reads it by physical address, so it must sit at a
/// fixed, aligned location the message can name; it lives on the caller's
/// stack for the duration of [`init`].
#[repr(C, align(16))]
struct PropBuffer {
    words: [u32; WORD_COUNT],
}

// Buffer word layout (one contiguous batch). Each tag is
// [id, value_buffer_bytes, req/resp_code, value...].
//
//   [0]        total buffer size in bytes
//   [1]        request code (0)
//   [2..=6]    set physical w/h   (id, 8, 0, W, H)
//   [7..=11]   set virtual w/h    (id, 8, 0, W, H)
//   [12..=15]  set depth          (id, 4, 0, depth)
//   [16..=19]  set pixel order    (id, 4, 0, order)
//   [20..=24]  allocate buffer    (id, 8, 0, base/align, size/0)
//   [25..=28]  get pitch          (id, 4, 0, pitch)
//   [29]       end tag (0)
const WORD_COUNT: usize = 30;
/// Allocate-buffer value slot: holds the alignment request on the way out and
/// the FB base on the way back.
const ALLOC_BASE_INDEX: usize = 23;
/// Allocate-buffer second value slot: size on the way back.
const ALLOC_SIZE_INDEX: usize = 24;
/// Get-pitch value slot: holds the pitch on the way back.
const PITCH_VALUE_INDEX: usize = 28;

/// A handle to the allocated surface: base pointer (ARM-physical) plus the
/// geometry needed to address pixels.
///
/// `pitch` is the firmware-reported row stride in bytes and may exceed
/// `width * 4`, so it is carried separately.
#[derive(Clone, Copy)]
pub struct Framebuffer {
    base: usize,
    width: u32,
    height: u32,
    pitch: u32,
}

impl Framebuffer {
    /// Width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Row stride in bytes (firmware-reported; not necessarily `width * 4`).
    #[must_use]
    pub const fn pitch(&self) -> u32 {
        self.pitch
    }

    /// ARM-physical base address of the surface.
    #[must_use]
    pub const fn base(&self) -> usize {
        self.base
    }

    /// Stores one 32bpp pixel at `(x, y)`. Out-of-range coordinates are
    /// ignored. `color` is `0x00RRGGBB` (RGB order, matching the requested
    /// pixel-order tag).
    pub fn put_pixel(&self, x: u32, y: u32, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let offset = (y as usize) * (self.pitch as usize) + (x as usize) * 4;
        // SAFETY: `base` is the GPU-allocated, ARM-physical framebuffer in the
        // device-mapped MMU block; `offset` is bounded by the per-row pitch and
        // the height check above, so the target is inside the allocated
        // surface. The surface is not aliased by any Rust-managed allocation.
        unsafe {
            write_volatile((self.base + offset) as *mut u32, color);
        }
    }

    /// Fills the whole surface with a single color.
    pub fn clear(&self, color: u32) {
        for y in 0..self.height as usize {
            let row = (self.base + y * self.pitch as usize) as *mut u32;
            for x in 0..self.width as usize {
                // SAFETY: `row` is inside the framebuffer row selected by
                // `y`, and `x < width` keeps the store inside the visible
                // 32bpp region. This is the same address range as `put_pixel`,
                // without per-pixel bounds checks and repeated pitch math.
                unsafe {
                    write_volatile(row.add(x), color);
                }
            }
        }
        // Ensure the clear reaches the device-observed surface before callers
        // start drawing the next boot screen.
        unsafe {
            asm!("dsb sy", options(nostack, preserves_flags));
        }
    }
}

#[cfg(feature = "selftest")]
impl Framebuffer {
    /// Builds a framebuffer over a caller-owned memory region for selftests,
    /// so the console blit can render into a readable buffer instead of the
    /// MMIO surface. The same `put_pixel`/`clear` stores run; the only change
    /// is that `base` addresses ordinary RAM the test can read back.
    ///
    /// `base` must point to at least `pitch * height` writable bytes that
    /// outlive the returned handle, and `pitch >= width * 4`.
    ///
    /// `pub` (selftest-gated) so the migrated console selftests in
    /// `reovim-system-kernel` build a readable stub surface across the §11 impl
    /// edge — the console renderer moved up there (SP04 04a) but the
    /// `Framebuffer` type STAYS here (the Q2 seam).
    pub fn over_region(base: usize, width: u32, height: u32, pitch: u32) -> Self {
        Self {
            base,
            width,
            height,
            pitch,
        }
    }
}

/// Requests a 1280x720x32 framebuffer from the `VideoCore` over the mailbox
/// property interface and returns a [`Framebuffer`] over it.
///
/// Returns `None` if the firmware reports the property batch failed or the
/// allocate tag handed back a null base — the splash treats that as "no
/// surface" and stays on the UART path.
#[must_use]
pub fn init() -> Option<Framebuffer> {
    // Build the property batch (layout documented on the index constants).
    // value_buffer_bytes holds the LARGER of the request and response so the
    // response fits in place.
    let mut buf = PropBuffer {
        words: [0; WORD_COUNT],
    };
    let w = &mut buf.words;

    // [0] total size in bytes (filled below); [1] request code 0.
    // `WORD_COUNT * 4` is the const 120, well within u32; the cast is
    // value-preserving.
    #[allow(clippy::cast_possible_truncation)]
    let total_bytes = (WORD_COUNT * 4) as u32;
    w[0] = total_bytes;
    w[1] = 0;

    // set physical w/h.
    w[2] = TAG_SET_PHYS_WH;
    w[3] = 8;
    w[4] = 0;
    w[5] = FB_WIDTH;
    w[6] = FB_HEIGHT;

    // set virtual w/h.
    w[7] = TAG_SET_VIRT_WH;
    w[8] = 8;
    w[9] = 0;
    w[10] = FB_WIDTH;
    w[11] = FB_HEIGHT;

    // set depth.
    w[12] = TAG_SET_DEPTH;
    w[13] = 4;
    w[14] = 0;
    w[15] = FB_DEPTH;

    // set pixel order.
    w[16] = TAG_SET_PIXEL_ORDER;
    w[17] = 4;
    w[18] = 0;
    w[19] = FB_PIXEL_ORDER_RGB;

    // allocate buffer. In: [alignment=16, 0]; out: [fb_base, fb_size].
    w[20] = TAG_ALLOCATE_BUFFER;
    w[21] = 8;
    w[22] = 0;
    w[ALLOC_BASE_INDEX] = 16; // alignment request (overwritten by fb_base)
    w[ALLOC_SIZE_INDEX] = 0;

    // get pitch. Out: [pitch_bytes].
    w[25] = TAG_GET_PITCH;
    w[26] = 4;
    w[27] = 0;
    w[PITCH_VALUE_INDEX] = 0;

    // end tag.
    w[29] = TAG_END;

    // The mailbox message is a 32-bit address; the property buffer lives on
    // the boot stack, which the image layout (`link-none-aarch64.ld`,
    // base 0x80000) keeps well below 4 GiB, so the truncation to u32 cannot
    // lose address bits on this target.
    #[allow(clippy::cast_possible_truncation)]
    let addr = (&raw const buf.words).addr() as u32;

    // The buffer must be 16-aligned so the channel fits in the low 4 bits.
    // `#[repr(align(16))]` guarantees it; bail rather than corrupt the message
    // if the toolchain ever placed it otherwise.
    if addr & 0xF != 0 {
        return None;
    }

    let message = (addr & !0xF) | MBOX_CH_PROP;

    mailbox_call(message);

    // The `VideoCore` writes the response in place. Confirm the batch succeeded.
    if buf.words[1] != MBOX_RESP_SUCCESS {
        return None;
    }

    let fb_base = buf.words[ALLOC_BASE_INDEX] & BUS_TO_PHYS_MASK;
    let pitch = buf.words[PITCH_VALUE_INDEX];

    if fb_base == 0 || pitch == 0 {
        return None;
    }

    Some(Framebuffer {
        base: fb_base as usize,
        width: FB_WIDTH,
        height: FB_HEIGHT,
        pitch,
    })
}

/// Queries the `VideoCore` for the ARM-visible RAM region (base + size in
/// bytes) over the mailbox property interface.
///
/// Returns `None` if the firmware reports the batch failed or a zero size — the
/// caller then reports memory as unknown rather than reporting a wrong range.
/// Rides the same `mailbox_call` transport as [`init`]; the request is a single
/// get-ARM-memory tag whose two-word value slot holds the base and size on the
/// way back.
#[must_use]
pub fn arm_memory() -> Option<(u64, u64)> {
    // One tag, two-word value slot:
    //   [0] total size bytes  [1] request code 0
    //   [2] tag id  [3] value bytes (8)  [4] req/resp code  [5] base  [6] size
    //   [7] end tag
    #[repr(C, align(16))]
    struct MemBuffer {
        words: [u32; 8],
    }

    let mut buf = MemBuffer { words: [0; 8] };
    let w = &mut buf.words;
    w[0] = 32; // 8 words * 4 bytes
    w[1] = 0;
    w[2] = TAG_GET_ARM_MEMORY;
    w[3] = 8; // value buffer bytes: base + size
    w[4] = 0;
    w[5] = 0; // base (out)
    w[6] = 0; // size (out)
    w[7] = TAG_END;

    // Same 16-aligned, sub-4-GiB stack-buffer reasoning as `init`: the address
    // truncates to u32 without losing bits on this target's image layout.
    #[allow(clippy::cast_possible_truncation)]
    let addr = (&raw const buf.words).addr() as u32;
    if addr & 0xF != 0 {
        return None;
    }
    let message = (addr & !0xF) | MBOX_CH_PROP;

    mailbox_call(message);

    if buf.words[1] != MBOX_RESP_SUCCESS {
        return None;
    }
    let base = buf.words[5];
    let size = buf.words[6];
    if size == 0 {
        return None;
    }
    Some((u64::from(base), u64::from(size)))
}

/// Queries the `VideoCore` for the SDRAM clock rate in Hz over the mailbox
/// property interface.
///
/// Returns `None` if the firmware reports the batch failed or a zero rate — the
/// caller then reports the memory frequency as unknown rather than a wrong
/// number. QEMU's BCM2711 model may not implement the clock and returns 0, which
/// is honestly surfaced as unknown. Rides the same `mailbox_call` transport as
/// [`arm_memory`]; the request is a single get-clock-rate tag whose two-word
/// value slot holds the clock id on the way in and the rate on the way back.
#[must_use]
pub fn sdram_clock_hz() -> Option<u64> {
    // One tag, two-word value slot:
    //   [0] total size bytes  [1] request code 0
    //   [2] tag id  [3] value bytes (8)  [4] req/resp code  [5] clock id  [6] rate
    //   [7] end tag
    #[repr(C, align(16))]
    struct ClockBuffer {
        words: [u32; 8],
    }

    let mut buf = ClockBuffer { words: [0; 8] };
    let w = &mut buf.words;
    w[0] = 32; // 8 words * 4 bytes
    w[1] = 0;
    w[2] = TAG_GET_CLOCK_RATE;
    w[3] = 8; // value buffer bytes: clock id + rate
    w[4] = 0;
    w[5] = CLOCK_ID_SDRAM; // clock id (in)
    w[6] = 0; // rate (out)
    w[7] = TAG_END;

    // Same 16-aligned, sub-4-GiB stack-buffer reasoning as `arm_memory`: the
    // address truncates to u32 without losing bits on this target's image layout.
    #[allow(clippy::cast_possible_truncation)]
    let addr = (&raw const buf.words).addr() as u32;
    if addr & 0xF != 0 {
        return None;
    }
    let message = (addr & !0xF) | MBOX_CH_PROP;

    mailbox_call(message);

    if buf.words[1] != MBOX_RESP_SUCCESS {
        return None;
    }
    let rate = buf.words[6];
    if rate == 0 {
        return None;
    }
    Some(u64::from(rate))
}

/// Posts `message` to the property channel and waits for its echo.
///
/// The protocol: barrier, poll until the write FIFO has room, post the
/// message, poll until a response arrives, drain responses until the one whose
/// low 4 bits select the property channel, barrier. The response value is the
/// same address we posted (the GPU edited the buffer in place), so the read
/// value is discarded after the channel check.
fn mailbox_call(message: u32) {
    // SAFETY: `MBOX_BASE + {STATUS,WRITE,READ}` are the BCM2711 mailbox MMIO
    // registers (device-mapped block of the boot MMU). Volatile u32 access is
    // the defined device-register path; `dsb sy` orders the buffer stores
    // before the GPU observes the message and orders the response read after
    // the GPU's writeback. No Rust-managed memory aliases these addresses.
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));

        while read_volatile((MBOX_BASE + MBOX_STATUS) as *const u32) & MBOX_FULL != 0 {
            core::hint::spin_loop();
        }
        write_volatile((MBOX_BASE + MBOX_WRITE) as *mut u32, message);

        loop {
            while read_volatile((MBOX_BASE + MBOX_STATUS) as *const u32) & MBOX_EMPTY != 0 {
                core::hint::spin_loop();
            }
            let response = read_volatile((MBOX_BASE + MBOX_READ) as *const u32);
            if response & 0xF == MBOX_CH_PROP {
                break;
            }
        }

        asm!("dsb sy", options(nostack, preserves_flags));
    }
}
