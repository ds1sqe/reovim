//! EditorCore log subsystem — LOG1 one mechanism (9.5 §1).
//!
//! The log subsystem is a rendering of the DS12 stream: every log line is the
//! deterministic rendering of exactly one DS12 event (LOG1). There is no
//! second pipeline.
//!
//! ## Module tree
//!
//! | Module | Purpose |
//! |---|---|
//! | [`render`] | LOG2 canonical line renderer (9.5 §2) |
//! | [`ring`]   | `LogRing` — the editor-core log ring (LOG6, 9.5 §8) |
//! | [`sink`]   | `FileSink` — the LOG7 file-backed subscriber (9.5 §9) |
//! | [`flush`]  | Panic-mirror region — `'static` append-time buffer (9.5 §9.1) |

pub mod flush;
pub mod render;
pub mod ring;
pub mod sink;

// L12 layout: sibling test files compiled under the `selftest` feature.

#[cfg(feature = "selftest")]
#[path = "flush_tests.rs"]
mod flush_tests;

#[cfg(feature = "selftest")]
#[path = "render_tests.rs"]
mod render_tests;

#[cfg(feature = "selftest")]
#[path = "ring_tests.rs"]
mod ring_tests;

#[cfg(feature = "selftest")]
#[path = "sink_tests.rs"]
mod sink_tests;
