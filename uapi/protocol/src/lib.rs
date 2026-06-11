//! `reovim-uapi-protocol` — the sans-IO framed-protocol codec and the 36
//! hand-written message structs (7.3).
//!
//! This crate is the **message-protocol SSOT** (7.3 §1a): the deterministic
//! byte codec ([`codec`]), the message inventory ([`messages`]), the borrowing
//! decoded views ([`view`]), frame assembly ([`frame`]), and the
//! handshake/correlation/unknown-tag state machine ([`state`]).  The *carrier*
//! (whatever moves the bytes) is out of scope — it lives in the
//! kernel/runtime tier (SP16).
//!
//! **Layer:** uapi tier — `#![no_std]`, **no-alloc** (SP15: callers provide
//! every buffer), L11-pure (imports only `core` + `reovim-uapi-abi`).  Zero
//! `arch/` edge in the shipped graph (DAG6); the only dependency is the ABI
//! catalog crate.
//!
//! ## Codec contract (SP13)
//!
//! Every message type provides the codec triple:
//!
//! - `encode(&self, out: &mut [u8]) -> Result<usize, ErrorCode>` — writes the
//!   body into a caller buffer, returns the byte count.
//! - `decode(buf: &'a [u8]) -> Result<Self, ErrorCode>` — validates the whole
//!   body, then returns a borrowing view bound to the input lifetime.
//! - `encoded_size(&self) -> usize` — exact wire size of the body.
//!
//! Encoding is little-endian, deterministic, no padding; `decode(encode(x))`
//! reproduces `x` byte-for-byte.
//!
//! ## Borrowed decoded form (SP17)
//!
//! `decode` builds no owned collections.  Fixed-width fields decode by value;
//! variable-length fields are validated *views* over the input — a borrowed
//! slice (`bytes`/`str`/carrier content) or a count-prefixed accessor view
//! (`list<T>`, see [`view`]).  All structural validation completes before
//! `decode` returns `Ok`, so view traversal afterwards cannot fail.
//!
//! ## Example: encode then decode a `Detach`
//!
//! ```rust
//! use reovim_uapi_protocol::messages::{Detach, Message};
//!
//! let msg = Detach { reason: "bye" };
//! let mut buf = [0u8; 32];
//! let n = msg.encode(&mut buf).unwrap();
//! assert_eq!(n, msg.encoded_size());
//! let back = Detach::decode(&buf[..n]).unwrap();
//! assert_eq!(back.reason, "bye");
//! ```
#![no_std]

pub mod codec;
pub mod frame;
pub mod messages;
pub mod state;
pub mod view;
