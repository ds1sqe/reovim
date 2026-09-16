//! Activation on an exact admitted package Resource. The bound declaration
//! selects Global scope; composition grants EXEC to authorize activation.

use crate::{Error, declaration::Declaration};

/// Global package activation API identity.
pub const API: [u8; 16] = [0xd2; 16];

/// Exec: Unit -> owned `Ref(module::control, READ | EXEC)`.
///
/// Success returns access to the exact Ready instance. Existing request custody
/// joins retries and retains cleanup after cancellation or lost delivery.
/// # Errors
/// Allocation failure or invalid declaration encoding rejects.
pub fn declaration() -> Result<Declaration, Error> {
    let mut bytes = API.to_vec();
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&[
        1, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 8, 0, b'a', b'c', b't', b'i', b'v',
        b'a', b't', b'e', 0, 12,
    ]);
    bytes.extend_from_slice(&super::control::API);
    bytes.extend_from_slice(&5_u32.to_le_bytes());
    bytes.push(0);
    Declaration::decode(&bytes)
}
