use super::*;

#[test]
fn test_ok_is_zero() {
    assert_eq!(REOVIM_OK, 0);
}

#[test]
fn test_all_errors_are_negative() {
    let errors = [
        REOVIM_ERR_NO_RUNTIME,
        REOVIM_ERR_NULL_PTR,
        REOVIM_ERR_NOT_FOUND,
        REOVIM_ERR_INVALID_UTF8,
        REOVIM_ERR_OUT_OF_RANGE,
        REOVIM_ERR_LAST_BUFFER,
        REOVIM_ERR_LAST_WINDOW,
        REOVIM_ERR_FAILED,
        REOVIM_ERR_PANIC,
        REOVIM_ERR_NO_INIT_CTX,
    ];
    for err in errors {
        assert!(err < 0, "error code {err} should be negative");
    }
}

#[test]
fn test_error_codes_are_distinct() {
    let errors = [
        REOVIM_OK,
        REOVIM_ERR_NO_RUNTIME,
        REOVIM_ERR_NULL_PTR,
        REOVIM_ERR_NOT_FOUND,
        REOVIM_ERR_INVALID_UTF8,
        REOVIM_ERR_OUT_OF_RANGE,
        REOVIM_ERR_LAST_BUFFER,
        REOVIM_ERR_LAST_WINDOW,
        REOVIM_ERR_FAILED,
        REOVIM_ERR_PANIC,
        REOVIM_ERR_NO_INIT_CTX,
    ];
    for (i, &a) in errors.iter().enumerate() {
        for (j, &b) in errors.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "error codes at index {i} and {j} must be distinct");
            }
        }
    }
}
