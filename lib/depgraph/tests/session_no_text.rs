use std::{fs, path::PathBuf};

#[test]
fn text_buffer_registry_boundary_lives_in_provider_text() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .ancestors()
        .nth(2)
        .expect("workspace root should be two levels above depgraph manifest");

    let session_lib = root.join("ext/server/drivers/text-session/src/lib.rs");
    let session_runtime = root.join("ext/server/drivers/text-session/src/runtime.rs");
    let provider_lib = root.join("server/lib/providers/text/src/lib.rs");

    let session_src = fs::read_to_string(&session_lib)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", session_lib.display()));
    let runtime_src = fs::read_to_string(&session_runtime)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", session_runtime.display()));
    let provider_src = fs::read_to_string(&provider_lib)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", provider_lib.display()));

    // Session driver should no longer define or re-export a local copy.
    assert!(
        !session_src.contains("mod text_buffer_registry"),
        "session/lib.rs should not declare a local text_buffer_registry module"
    );
    assert!(
        !session_src.contains("TextBufferRegistry"),
        "session/lib.rs should no longer re-export TextBufferRegistry"
    );

    // Provider-text should own and re-export TextBufferRegistry.
    assert!(
        provider_src.contains("mod text_buffer_registry"),
        "provider-text should declare text_buffer_registry module"
    );
    assert!(
        provider_src.contains("pub use text_buffer_registry::TextBufferRegistry"),
        "provider-text should re-export TextBufferRegistry"
    );

    // Session runtime should resolve the registry from provider-text directly.
    assert!(
        runtime_src.contains("reovim_provider_text::TextBufferRegistry"),
        "session runtime should import TextBufferRegistry from provider-text"
    );
}
