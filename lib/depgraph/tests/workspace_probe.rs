//! The live depgraph probe (spec 1.2 §5): runs the full DAG check over the
//! real workspace on every `cargo test`.

use std::path::PathBuf;

use reovim_depgraph::{Category, ProbeConfig, enumerate_crates, run_probe};

fn workspace_root() -> PathBuf {
    // lib/depgraph -> workspace root (two levels up via CARGO_MANIFEST_DIR).
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("lib/depgraph has a workspace root two levels up")
        .to_path_buf()
}

#[test]
fn workspace_conforms_to_the_layer_dag() {
    let root = workspace_root();
    let config = ProbeConfig::default_for(&root).expect("probe config loads");
    let report = run_probe(&root, &config).expect("probe runs");
    assert!(report.is_clean(), "depgraph violations:\n{}", report.summary());
}

#[test]
fn workspace_classification_is_unambiguous_and_complete() {
    let root = workspace_root();
    let config = ProbeConfig::default_for(&root).expect("probe config loads");
    let report = run_probe(&root, &config).expect("probe runs");
    // §5 step 5: every enumerated crate is classified into exactly one
    // category (any ambiguity or unknown path would surface as a DAG1
    // violation and shrink the classified total below the enumerated count).
    let classified: usize = report.category_counts.values().sum();
    let enumerated = enumerate_crates(&root).expect("enumeration runs").len();
    assert_eq!(classified, enumerated, "{}", report.summary());
    assert!(
        report.category_counts.get(&Category::Foundation) >= Some(&1),
        "reovim-depgraph itself classifies as Foundation"
    );
}

#[test]
fn reovim_depgraph_classifies_as_foundation() {
    let root = workspace_root();
    let crates = enumerate_crates(&root).expect("enumeration runs");
    let depgraph = crates.iter().find(|c| c.name == "reovim-depgraph");
    assert!(depgraph.is_some(), "reovim-depgraph not found in workspace");
    let cat = reovim_depgraph::classify(
        &depgraph.unwrap().path,
        &reovim_depgraph::default_category_table(),
    )
    .expect("reovim-depgraph classifies");
    assert_eq!(cat, Category::Foundation);
}

#[test]
fn greenfield_allowlist_is_empty() {
    let root = workspace_root();
    let config = ProbeConfig::default_for(&root).expect("probe config loads");
    // 1.2 §8: the v0.16 allowlist starts empty; growth is an explicit,
    // issue-tracked decision — this test makes adding an entry a visible,
    // reviewed change.
    assert!(
        config.allowlist.entry.is_empty(),
        "transitional allowlist must stay empty unless an issue-tracked transition is adopted"
    );
}
