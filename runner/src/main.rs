//! Reovim - Linux kernel-inspired text editor
//!
//! This is a placeholder entry point. The full editor functionality
//! requires loading policy modules. See `examples/demo.rs` for a
//! complete demonstration of the kernel-driver-module architecture.
//!
//! # Running the Demo
//!
//! ```sh
//! cargo run --example demo
//! ```

fn main() {
    println!("Reovim v0.9.0-dev - Linux kernel-inspired text editor");
    println!();
    println!("The runner crate provides mechanism (event loop, registries).");
    println!("Policy modules (keymap, editor, etc.) provide the actual behavior.");
    println!();
    println!("Run the demo to see the architecture in action:");
    println!("  cargo run --example demo");
}
