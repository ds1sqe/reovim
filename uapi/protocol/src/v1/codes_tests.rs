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
