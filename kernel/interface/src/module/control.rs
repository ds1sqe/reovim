//! Operations on an exact module-instance Resource, discovered and bound like
//! any other control interface. Operation rights come from caller grants.

use crate::{Error, declaration::Declaration};

/// Module-instance lifecycle control API identity.
pub const API: [u8; 16] = [0xd1; 16];
/// Read status: Unit -> U32, requiring READ.
///
/// Values are Constructed=0, Starting=1, Ready=2, Closing=3, Closed=4.
/// Module or unfinished cleanup
/// failure returns its retained Error; it does not prove cleanup ended.
pub const STATUS: u32 = 1;
/// Request unload: Unit -> Unit, requiring EXEC. Success means cleanup ended.
/// Rejection due to retained use returns Busy without fencing the instance.
pub const UNLOAD: u32 = 2;

/// Declares lifecycle operations without constructing a provider.
/// # Errors
/// Allocation failure or invalid declaration encoding rejects.
pub fn declaration() -> Result<Declaration, Error> {
    let mut bytes = API.to_vec();
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&[
        2, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 6, 0, b's', b't', b'a', b't', b'u',
        b's', 0, 3, 0,
    ]);
    bytes.extend_from_slice(&[
        2, 2, 0, 0, 0, 0, 4, 0, 0, 0, 0, 6, 0, b'u', b'n', b'l', b'o', b'a',
        b'd', 0, 0, 0,
    ]);
    Declaration::decode(&bytes)
}
