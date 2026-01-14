//! Standard JSON-RPC 2.0 error codes.

/// Parse error: Invalid JSON was received by the server.
pub const PARSE_ERROR: i32 = -32700;

/// Invalid Request: The JSON sent is not a valid Request object.
pub const INVALID_REQUEST: i32 = -32600;

/// Method not found: The method does not exist / is not available.
pub const METHOD_NOT_FOUND: i32 = -32601;

/// Invalid params: Invalid method parameter(s).
pub const INVALID_PARAMS: i32 = -32602;

/// Internal error: Internal JSON-RPC error.
pub const INTERNAL_ERROR: i32 = -32603;

// Application-specific error code range: -32000 to -32099

/// Buffer not found (application-specific).
pub const BUFFER_NOT_FOUND: i32 = -32001;

/// Command not found (application-specific).
pub const COMMAND_NOT_FOUND: i32 = -32002;

/// Invalid key notation (application-specific).
pub const INVALID_KEY_NOTATION: i32 = -32003;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_error_codes() {
        assert_eq!(PARSE_ERROR, -32700);
        assert_eq!(INVALID_REQUEST, -32600);
        assert_eq!(METHOD_NOT_FOUND, -32601);
        assert_eq!(INVALID_PARAMS, -32602);
        assert_eq!(INTERNAL_ERROR, -32603);
    }

    #[test]
    fn test_application_error_codes_in_range() {
        // Application-specific codes must be in -32000 to -32099
        const { assert!(BUFFER_NOT_FOUND >= -32099 && BUFFER_NOT_FOUND <= -32000) };
        const { assert!(COMMAND_NOT_FOUND >= -32099 && COMMAND_NOT_FOUND <= -32000) };
        const { assert!(INVALID_KEY_NOTATION >= -32099 && INVALID_KEY_NOTATION <= -32000) };
    }
}
