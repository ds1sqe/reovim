// Test fixture for LSP diagnostic integration tests.
//
// This file contains intentional issues that produce predictable
// diagnostics from rust-analyzer:
//
//   1. Unused variable `unused` in `add()` — warning
//   2. Type mismatch in `broken()` — error
//
// Do NOT fix these issues — they are the test expectations.

pub fn add(a: i32, b: i32) -> i32 {
    let unused = 0;
    a + b
}

pub fn broken() -> i32 {
    "not an integer"
}
