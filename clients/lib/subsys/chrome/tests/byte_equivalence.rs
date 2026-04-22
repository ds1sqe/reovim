//! Byte-equivalence tripwire: locks the landing chrome surface to a pinned
//! fixture so any future change to the chrome render path that perturbs a
//! single output byte fails the test and surfaces the exact divergence.
//!
//! Determinism invariant: `LandingModule` is time-free under
//! `with_clock(TestClock)`. No `rand::`, no `SystemTime::now()`, no
//! ambient-time sources; `SystemClock` is only reachable via the
//! production `LandingModule::new()` path, which this test does not use.

use std::sync::Arc;

use {
    reovim_arch::clock::TestClock,
    reovim_client_subsys_module::{ClientModule, Rect, testing::MockPlatformCapabilities},
    reovim_ext_client_tui_cap_cell::{
        CellCapability,
        style::{CellColor, CellStyle},
    },
    reovim_tui_mod_landing::LandingModule,
};

/// Width of the test surface.
const SURFACE_W: u16 = 80;
/// Height of the test surface.
const SURFACE_H: u16 = 24;

/// Canonical fixture: hex-encoded bytes of the landing module render output
/// at time zero (no clock ticks, first breathing frame).
///
/// Encoding: for each cell in row-major order (top-left to bottom-right):
/// 13 bytes — `[ch_u32_le, fg_variant, fg_r, fg_g, fg_b, bg_variant, bg_r, bg_g, bg_b, attrs]`.
/// Total = `SURFACE_W` * `SURFACE_H` * 13 bytes.
///
/// Color variant byte: `0` = None/Default, `1` = `Rgb`, `2` = `Ansi256`, `3` = `Named`.
const FIXTURE_HEX: &str = include_str!("byte_equivalence_fixture.hex");

/// Encode a single color into 4 bytes: `[variant, r_or_n, g_or_0, b_or_0]`.
///
/// Variant: `0` = `None`/`Default`, `1` = `Rgb`, `2` = `Ansi256`, `3` = `Named`.
fn encode_color(color: Option<CellColor>, buf: &mut [u8]) {
    match color {
        None | Some(CellColor::Default) => {
            buf[0] = 0;
            buf[1] = 0;
            buf[2] = 0;
            buf[3] = 0;
        }
        Some(CellColor::Rgb(r, g, b)) => {
            buf[0] = 1;
            buf[1] = r;
            buf[2] = g;
            buf[3] = b;
        }
        Some(CellColor::Ansi256(n)) => {
            buf[0] = 2;
            buf[1] = n;
            buf[2] = 0;
            buf[3] = 0;
        }
        Some(CellColor::Named(n)) => {
            buf[0] = 3;
            buf[1] = n;
            buf[2] = 0;
            buf[3] = 0;
        }
    }
}

/// Encode a single cell to 13 bytes.
fn encode_cell(ch: char, style: &CellStyle) -> [u8; 13] {
    let mut out = [0u8; 13];
    let ch_u32 = ch as u32;
    out[0..4].copy_from_slice(&ch_u32.to_le_bytes());
    encode_color(style.fg, &mut out[4..8]);
    encode_color(style.bg, &mut out[8..12]);
    out[12] = style.attrs.bits();
    out
}

/// Render the landing module at time zero and encode the result.
fn render_and_encode() -> Vec<u8> {
    let clock = Arc::new(TestClock::new());
    let module = LandingModule::with_clock(clock);

    let mut surface = CellCapability::new(SURFACE_W, SURFACE_H);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: SURFACE_W,
        height: SURFACE_H,
    };
    module.chrome_render(&mut surface, bounds, &MockPlatformCapabilities::new());

    let total = SURFACE_W as usize * SURFACE_H as usize;
    let mut bytes = Vec::with_capacity(total * 13);

    for y in 0..SURFACE_H {
        for x in 0..SURFACE_W {
            let cell = surface.get_cell(x, y).cloned().unwrap_or_default();
            let encoded = encode_cell(cell.ch, &cell.style);
            bytes.extend_from_slice(&encoded);
        }
    }

    bytes
}

/// Helper to generate the fixture hex (for bootstrap only; not part of CI).
///
/// Run with: `cargo test -p reovim-client-subsys-chrome --test byte_equivalence generate_fixture -- --nocapture`
/// Then paste the output into `byte_equivalence_fixture.hex`.
#[test]
#[ignore = "fixture generator — run manually to regenerate the fixture hex"]
fn generate_fixture() {
    use std::fmt::Write as _;
    let bytes = render_and_encode();
    let hex = bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            write!(s, "{b:02x}").expect("write to String is infallible");
            s
        });
    println!("{hex}");
}

#[test]
fn chrome_render_output_is_byte_identical_post_migration() {
    let fixture_hex = FIXTURE_HEX.trim();
    let fixture: Vec<u8> = (0..fixture_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&fixture_hex[i..i + 2], 16).expect("valid hex"))
        .collect();

    let actual = render_and_encode();

    if actual != fixture {
        // Find first diverging byte for diagnostic
        let diverge_idx = actual
            .iter()
            .zip(fixture.iter())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| actual.len().min(fixture.len()));

        let cell_idx = diverge_idx / 13;
        let byte_in_cell = diverge_idx % 13;
        let row = cell_idx / SURFACE_W as usize;
        let col = cell_idx % SURFACE_W as usize;

        panic!(
            "chrome render output diverges at byte {diverge_idx} \
             (cell ({col},{row}), byte_in_cell={byte_in_cell}); \
             actual[{diverge_idx}]={:#04x} fixture[{diverge_idx}]={:#04x}",
            actual.get(diverge_idx).copied().unwrap_or_default(),
            fixture.get(diverge_idx).copied().unwrap_or_default(),
        );
    }
}
