// Test file for LSP diagnostics (inside workspace)

fn main() {
    // Error: undefined variable
    let x = undefined_variable;

    // Error: type mismatch
    let y: i32 = "not a number";
}
