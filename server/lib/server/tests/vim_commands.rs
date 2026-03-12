//! E2E tests for vim command execution.
//!
//! These tests verify that vim commands work correctly through the full
//! server/client architecture using gRPC v2 protocol.
//!
//! # Status
//!
//! - All 30 tests are enabled and passing
//! - Text object tests moved to `reovim-module-textobjects` (tests/e2e.rs)
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p reovim-server --test vim_commands
//! ```
//!
//! # Log Files
//!
//! Test logs are captured to `tmp/test-logs/{test_name}_{timestamp}.log`.
//! Check these files when debugging test failures.

use reovim_testing::IntegrationTest;

// ============================================================================
// Basic Motion Tests
// ============================================================================

/// Test `h` motion (move left).
#[tokio::test]
async fn test_h_move_left() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .with_cursor_at(0, 3) // On 'l'
        .send_keys("h")
        .run()
        .await;
    result.assert_cursor(0, 2); // Now on 'l' (first one)
}

/// Test `l` motion (move right).
#[tokio::test]
async fn test_l_move_right() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("l")
        .run()
        .await;
    result.assert_cursor(0, 1);
}

/// Test `j` motion (move down).
#[tokio::test]
async fn test_j_move_down() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line1\nline2\nline3")
        .send_keys("j")
        .run()
        .await;
    result.assert_cursor(1, 0);
}

/// Test `k` motion (move up).
#[tokio::test]
async fn test_k_move_up() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line1\nline2\nline3")
        .with_cursor_at(2, 0)
        .send_keys("k")
        .run()
        .await;
    result.assert_cursor(1, 0);
}

/// Test `w` motion (word forward).
#[tokio::test]
async fn test_w_word_forward() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world test")
        .send_keys("w")
        .run()
        .await;
    result.assert_cursor(0, 6); // Start of 'world'
}

/// Test `b` motion (word backward).
#[tokio::test]
async fn test_b_word_backward() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world test")
        .with_cursor_at(0, 6) // On 'w' of 'world'
        .send_keys("b")
        .run()
        .await;
    result.assert_cursor(0, 0); // Back to 'h'
}

/// Test `e` motion (end of word).
#[tokio::test]
async fn test_e_end_of_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("e")
        .run()
        .await;
    result.assert_cursor(0, 4); // On 'o' of 'hello'
}

/// Test `0` motion (start of line).
#[tokio::test]
async fn test_0_start_of_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .with_cursor_at(0, 5)
        .send_keys("0")
        .run()
        .await;
    result.assert_cursor(0, 0);
}

/// Test `$` motion (end of line).
#[tokio::test]
async fn test_dollar_end_of_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("$")
        .run()
        .await;
    result.assert_cursor(0, 10); // On 'd'
}

// ============================================================================
// Insert Mode Tests
// ============================================================================

/// Test `i` enters insert mode.
#[tokio::test]
async fn test_i_enters_insert_mode() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("i")
        .run()
        .await;
    result.assert_insert_mode();
}

/// Test `i` + text + `<Esc>` inserts text.
#[tokio::test]
async fn test_i_inserts_text() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("")
        .send_keys("ihello<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("hello");
    result.assert_normal_mode();
}

/// Test `a` appends after cursor.
#[tokio::test]
async fn test_a_appends() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hllo")
        .send_keys("ae<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("hello");
}

/// Test `A` appends at end of line.
#[tokio::test]
async fn test_capital_a_appends_eol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("A world<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("hello world");
}

/// Test `o` opens line below.
#[tokio::test]
async fn test_o_opens_line_below() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line1\nline3")
        .send_keys("oline2<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("line1\nline2\nline3");
}

/// Test `O` opens line above.
#[tokio::test]
async fn test_capital_o_opens_line_above() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line2")
        .send_keys("Oline1<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("line1\nline2");
}

// ============================================================================
// Delete Operator Tests
// ============================================================================

/// Test `dw` deletes word.
#[tokio::test]
async fn test_dw_delete_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("dw")
        .run()
        .await;
    result.assert_buffer_eq("world");
}

/// Test `dd` deletes line.
#[tokio::test]
async fn test_dd_delete_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line1\nline2\nline3")
        .send_keys("dd")
        .run()
        .await;
    result.assert_buffer_eq("line2\nline3");
}

/// Test `d$` deletes to end of line.
#[tokio::test]
async fn test_d_dollar_delete_to_eol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .with_cursor_at(0, 5)
        .send_keys("d$")
        .run()
        .await;
    result.assert_buffer_eq("hello");
}

/// Test `D` (alias for `d$`).
#[tokio::test]
async fn test_capital_d_delete_to_eol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .with_cursor_at(0, 5)
        .send_keys("D")
        .run()
        .await;
    result.assert_buffer_eq("hello");
}

/// Test `x` deletes character under cursor.
#[tokio::test]
async fn test_x_delete_char() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("x")
        .run()
        .await;
    result.assert_buffer_eq("ello");
}

// ============================================================================
// Change Operator Tests
// ============================================================================

/// Test `cw` changes word.
#[tokio::test]
async fn test_cw_change_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("cwgoodbye<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("goodbye world");
}

/// Test `cc` changes entire line.
#[tokio::test]
async fn test_cc_change_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("old line\nnext line")
        .send_keys("ccnew line<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("new line\nnext line");
}

/// Test `C` (change to end of line).
#[tokio::test]
async fn test_capital_c_change_to_eol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .with_cursor_at(0, 5)
        .send_keys("Cthere<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("hellothere");
}

// ============================================================================
// Yank/Put Tests
// ============================================================================

/// Test `yy` + `p` yanks and puts line.
#[tokio::test]
async fn test_yy_p_yank_put_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line1\nline2")
        .send_keys("yyp")
        .run()
        .await;
    result.assert_buffer_eq("line1\nline1\nline2");
}

/// Test `yw` + `p` yanks and puts word.
#[tokio::test]
async fn test_yw_p_yank_put_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("yw$p")
        .run()
        .await;
    result.assert_buffer_contains("hello");
}

// ============================================================================
// Visual Mode Tests
// ============================================================================

/// Test `v` enters visual mode.
#[tokio::test]
async fn test_v_enters_visual_mode() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("v")
        .run()
        .await;
    result.assert_visual_mode();
}

/// Test visual selection + `d` deletes.
#[tokio::test]
async fn test_visual_delete() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("vllld")
        .run()
        .await;
    result.assert_buffer_eq("o world");
}

/// Test `V` (visual line) + `d` deletes line.
#[tokio::test]
async fn test_visual_line_delete() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line1\nline2\nline3")
        .send_keys("Vd")
        .run()
        .await;
    result.assert_buffer_eq("line2\nline3");
}

// ============================================================================
// Repeat (Dot) Tests
// ============================================================================

/// Test `.` repeats last change.
#[tokio::test]
async fn test_dot_repeats_change() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("foo foo foo")
        .send_keys("cwbar<Esc>w.w.")
        .run()
        .await;
    result.assert_buffer_eq("bar bar bar");
}

/// Test `.` repeats delete.
#[tokio::test]
async fn test_dot_repeats_delete() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("one two three four")
        .send_keys("dw..")
        .run()
        .await;
    result.assert_buffer_eq("four");
}

// ============================================================================
// Health Check Tests (#610)
// ============================================================================

/// Test `:checkhealth` shows all 8 diagnostic sections including new #610 ones.
#[tokio::test]
async fn test_checkhealth_has_all_sections() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("test")
        .send_keys(":checkhealth<CR>")
        .with_delay(200)
        .run()
        .await;

    // All 8 sections from collect_all() (#610 added Modules, Dependencies, Configuration)
    result.assert_buffer_contains("=== System ===");
    result.assert_buffer_contains("=== Modules ===");
    result.assert_buffer_contains("=== Dependencies ===");
    result.assert_buffer_contains("=== Configuration ===");
    result.assert_buffer_contains("=== Language Servers ===");
    result.assert_buffer_contains("=== Syntax Highlighting ===");
    result.assert_buffer_contains("=== Clipboard ===");
    result.assert_buffer_contains("=== Options ===");
}

/// Test `:checkhealth` shows loaded modules from `ModuleLoadReport`.
#[tokio::test]
async fn test_checkhealth_shows_loaded_modules() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("test")
        .send_keys(":checkhealth<CR>")
        .with_delay(200)
        .run()
        .await;

    // Key modules should show as loaded
    result.assert_buffer_contains("[OK] vim: loaded");
    result.assert_buffer_contains("[OK] editor: loaded");
    result.assert_buffer_contains("[OK] completion: loaded");
    result.assert_buffer_contains("[OK] health-check: loaded");
}

/// Test `:checkhealth` shows dependencies are satisfied.
#[tokio::test]
async fn test_checkhealth_dependencies_satisfied() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("test")
        .send_keys(":checkhealth<CR>")
        .with_delay(200)
        .run()
        .await;

    result.assert_buffer_contains("all satisfied");
}

/// Test `:checkhealth` Configuration section shows config path when
/// `REOVIM_CONFIG_DIR` env var is set and `modules.toml` exists.
#[tokio::test]
async fn test_checkhealth_config_path_with_env_override() {
    // Create a temp config dir with a modules.toml
    let config_dir = format!("/tmp/reovim-e2e-610-{}", std::process::id());
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(format!("{config_dir}/modules.toml"), "[modules.completion]\nenabled = true\n")
        .unwrap();

    let result = IntegrationTest::with_env(&[("REOVIM_CONFIG_DIR", &config_dir)])
        .await
        .with_buffer("test")
        .send_keys(":checkhealth<CR>")
        .with_delay(200)
        .run()
        .await;

    // Configuration section should show the config file path
    result.assert_buffer_contains("=== Configuration ===");
    result.assert_buffer_contains("[OK] Config file");
    result.assert_buffer_contains("modules.toml");

    std::fs::remove_dir_all(&config_dir).ok();
}

/// Test that completion config consumer applies pumheight override from modules.toml.
/// Verifies the full L2 -> L3 pipeline: config file -> `ModuleConfigStore` -> option override.
#[tokio::test]
async fn test_config_consumer_pumheight_override() {
    // Create config with pumheight=25
    let config_dir = format!("/tmp/reovim-e2e-610-ph-{}", std::process::id());
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        format!("{config_dir}/modules.toml"),
        "[modules.completion]\nenabled = true\n\n[modules.completion.settings]\npumheight = 25\n",
    )
    .unwrap();

    // Run :checkhealth which shows "Changed from defaults" if options were overridden
    let result = IntegrationTest::with_env(&[("REOVIM_CONFIG_DIR", &config_dir)])
        .await
        .with_buffer("test")
        .send_keys(":checkhealth<CR>")
        .with_delay(200)
        .run()
        .await;

    // Options section should show that defaults were changed (pumheight=25 vs default 10)
    result.assert_buffer_contains("=== Options ===");
    result.assert_buffer_contains("Changed from defaults");

    std::fs::remove_dir_all(&config_dir).ok();
}

/// Test `:checkhealth` shows options count.
#[tokio::test]
async fn test_checkhealth_options_section() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("test")
        .send_keys(":checkhealth<CR>")
        .with_delay(200)
        .run()
        .await;

    result.assert_buffer_contains("=== Options ===");
    result.assert_buffer_contains("Options registered");
}
