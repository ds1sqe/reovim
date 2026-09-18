//! Writes the linked-module table from `modules` and `bin`: one name
//! per line, each the `reovim-module-<name>` or `reovim-bin-<name>`
//! crate Cargo.toml lists. A Driver under `/bin` loads as a Module
//! like any other, so both lists make rows of one table. The milestone
//! 3 loader replaces this producer, not the table's readers.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    println!("cargo::rerun-if-changed=modules");
    println!("cargo::rerun-if-changed=bin");
    println!("cargo::rerun-if-changed=Cargo.toml");
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    let mut table = String::from(
        "/// Builds one module over the root directory.\n\
         pub type Boot = fn(&str) -> Booted;\n\
         /// Every linked module by name, in `modules` then `bin` order.\n\
         pub const MODULES: &[(&str, Boot)] = &[\n",
    );
    for (list, prefix) in
        [("modules", "reovim-module-"), ("bin", "reovim-bin-")]
    {
        let names = std::fs::read_to_string(list).expect("a list file");
        rows(&mut table, &names, prefix, &manifest);
    }
    table.push_str("];\n");
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    std::fs::write(Path::new(&out).join("link.rs"), table).expect("link.rs");
}

/// One row per name in `names`, each the `prefix` crate of that name,
/// which `manifest` must depend on for the row to link.
fn rows(table: &mut String, names: &str, prefix: &str, manifest: &str) {
    for name in names.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let package = format!("{prefix}{name}");
        assert!(
            manifest.contains(&package),
            "a list names `{name}` but Cargo.toml has no `{package}`"
        );
        let krate = package.replace('-', "_");
        writeln!(table, "    ({name:?}, {krate}::boot),").unwrap();
    }
}
