//! Build script for reovim-protocol.
//!
//! When the `grpc` feature is enabled, this compiles protobuf schemas
//! into Rust code using `tonic-build`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "grpc")]
    {
        tonic_build::configure()
            .build_server(true)
            .build_client(true)
            .compile_protos(
                &[
                    "proto/reovim/v2/common.proto",
                    "proto/reovim/v2/input.proto",
                    "proto/reovim/v2/state.proto",
                    "proto/reovim/v2/buffer.proto",
                    "proto/reovim/v2/editor.proto",
                    "proto/reovim/v2/module.proto",
                    "proto/reovim/v2/server.proto",
                    "proto/reovim/v2/notification.proto",
                ],
                &["proto"],
            )?;
    }

    Ok(())
}
