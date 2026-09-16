//! Writes the linked-module table from `modules`: one module name per
//! line, each the `reovim-module-<name>` crate Cargo.toml lists. The
//! milestone 3 loader replaces this producer, not the table's readers.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    println!("cargo::rerun-if-changed=modules");
    println!("cargo::rerun-if-changed=Cargo.toml");
    let list = std::fs::read_to_string("modules").expect("apps/reovim/modules");
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    let mut table = String::from(
        "/// Builds one module over the root directory.\n\
         pub type Boot = fn(&str) -> Booted;\n\
         /// Every linked module by name, in `modules` order.\n\
         pub const MODULES: &[(&str, Boot)] = &[\n",
    );
    for name in list.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let package = format!("reovim-module-{name}");
        assert!(
            manifest.contains(&package),
            "modules lists `{name}` but Cargo.toml has no `{package}`"
        );
        let krate = package.replace('-', "_");
        writeln!(table, "    ({name:?}, {krate}::boot),").unwrap();
    }
    table.push_str("];\n");
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    std::fs::write(Path::new(&out).join("link.rs"), table).expect("link.rs");
}
