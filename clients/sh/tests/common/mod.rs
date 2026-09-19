//! What the shell cases share. Each test binary compiles this on its
//! own.

use reovim_client_sh::{Shell, run};
use reovim_kernel_interface::Calls;

/// One typed line from where `shell` stands; its output, or the line a
/// Miss would print.
pub fn typed(ctx: &dyn Calls, shell: &mut Shell, line: &str) -> String {
    match run(ctx, shell, line) {
        Ok(ran) => String::from_utf8(ran.out).unwrap(),
        Err(miss) => miss.to_string(),
    }
}
