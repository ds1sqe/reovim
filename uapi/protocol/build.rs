//! Build script for reovim-protocol.
//!
//! When the `grpc` feature is enabled, this compiles protobuf schemas
//! into Rust code using `tonic-build`.

fn main() {
    #[cfg(feature = "grpc")]
    compile_protos();
}

#[cfg(feature = "grpc")]
fn compile_protos() {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                "proto/reovim/v3/common.proto",
                "proto/reovim/v3/input.proto",
                "proto/reovim/v3/state.proto",
                "proto/reovim/v3/buffer.proto",
                "proto/reovim/v3/editor.proto",
                "proto/reovim/v3/module.proto",
                "proto/reovim/v3/server.proto",
                "proto/reovim/v3/notification.proto",
                "proto/reovim/v3/presence.proto",
                "proto/reovim/v3/debug.proto",
                "proto/reovim/v3/extension.proto",
                "proto/reovim/v3/command.proto",
                "proto/reovim/v3/client_debug.proto",
            ],
            &["proto"],
        )
        .expect("Failed to compile protos");
}
