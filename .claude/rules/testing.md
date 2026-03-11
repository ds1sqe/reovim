---
paths:
  - "**/*test*"
  - "shared/testing/**/*.rs"
---

# Integration Test Infrastructure

Phase 7+ integration tests use a fluent builder API in `shared/testing/`:

```rust
// Single-client test example
let result = IntegrationTest::new()
    .await
    .with_buffer("hello world")
    .send_keys("dw")
    .run()
    .await;
result.assert_buffer_eq("world");

// Step-by-step test with per-key assertions (Issue #428)
let trace = StepTest::new()
    .await
    .with_buffer("hello world")
    .step("d")
        .expect_mode_contains("DELETE")
    .step("w")
        .expect_buffer("world")
    .run()
    .await;
trace.assert_ok();

// Multi-client test example
MultiClientTest::with_clients(2)
    .await
    .run(|mut clients| async move {
        clients[0].send_keys("ihello<Esc>").await.unwrap();
        let content = clients[1].get_buffer().await.unwrap();
        assert!(content.contains("hello"));
    })
    .await;
```

**Key components:**
- `TestServerHarness` - Spawns server on OS-assigned port, auto-cleanup via Drop
- `IntegrationTest` - Fluent builder for single-client tests, temp file cleanup
- `StepTest` - Per-keystroke state tracking with inline assertions
- `MultiClientTest` - Multi-client concurrent testing
- `TestResult` - Assertions: `assert_buffer_eq!`, `assert_cursor!`, `assert_register!`, `assert_mode!`

**Per-test log capture (Issue #428):**
- Logs are saved to `{module}/tmp/test-logs/{test_name}_{timestamp}.log`
- Example: `modules/vim/tmp/test-logs/test_dw_delete_word_20260124_140503.log`
- Failed assertions include log path hint: `Server log: tmp/test-logs/...`
- Access via `result.log_path()` or `trace.log_path()`

**Module loading troubleshooting:**
- If tests behave unexpectedly, check logs for `WARN module not found`
- This indicates `.so` files are outdated - run `./scripts/build-module.sh <module> --install`
- Verify FFI symbols with the install script output
