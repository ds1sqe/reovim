//! In-crate server harness for the CLI E2E tests.
//!
//! TODO: promote to reovim-testing. Duplicated from the Phase 1
//! integration test at
//! `server/lib/server/tests/client_debug_stream_roundtrip.rs` because
//! that `TestHarness` is a private test struct. Promoting requires
//! crossing `tools/testing → reovim-server` as a new dep, which is a
//! separate architectural decision and out of scope for #770 Phase 2.

#![allow(dead_code)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]

use {
    reovim_client_subsys_driver_loader::LoadedClientDebug,
    reovim_server::{ClientDebugRegistry, DebugDriverHandle, Server, ServerConfig, TransportMode},
    std::{env, path::PathBuf, sync::Arc},
    tokio::sync::oneshot,
};

/// Resolve the on-disk path of a cdylib built from the given
/// underscore-named crate. Mirrors the Phase 1.E test helper.
pub fn cdylib_path(crate_underscore_name: &str) -> PathBuf {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let target_dir = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        let workspace = PathBuf::from(&manifest)
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .to_path_buf();
        workspace.join("target").display().to_string()
    });
    let mut p = PathBuf::from(target_dir);
    p.push("debug");
    let filename = if cfg!(target_os = "windows") {
        format!("{crate_underscore_name}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{crate_underscore_name}.dylib")
    } else {
        format!("lib{crate_underscore_name}.so")
    };
    p.push(filename);
    p
}

/// Register the Phase 0 PoC cdylib into the given registry.
pub fn register_poc(registry: &ClientDebugRegistry, underscore: &str, name: &str) {
    let path = cdylib_path(underscore);
    assert!(
        path.exists(),
        "debug PoC cdylib not found at {}; run `cargo build -p {}` first",
        path.display(),
        underscore.replace('_', "-")
    );
    let driver = LoadedClientDebug::load_from_path(&path)
        .unwrap_or_else(|e| panic!("load {underscore}: {e:?}"));
    registry.register(name, Box::new(driver) as Box<dyn DebugDriverHandle>);
}

pub struct TestServer {
    pub port: u16,
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
}

impl TestServer {
    /// Start a server on port 0 with the given registry populated.
    pub async fn start(registry: Arc<ClientDebugRegistry>) -> Self {
        let mut config = ServerConfig::grpc(0);
        config.transport = TransportMode::Grpc { port: 0 };
        let server = Server::new(config).with_client_debug_registry(registry);
        let (port_tx, port_rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let join = tokio::spawn(async move {
            server
                .run_until(
                    async move {
                        let _ = shutdown_rx.await;
                    },
                    Some(port_tx),
                )
                .await
        });
        let port = port_rx.await.expect("server port reported");
        Self {
            port,
            shutdown: Some(shutdown_tx),
            join: Some(join),
        }
    }

    pub fn grpc_addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}
