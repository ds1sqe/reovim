//! `ServerHandle` adapter bridging sync client modules to the async gRPC client.
//!
//! Client modules call `ServerHandle` methods synchronously. This adapter
//! holds a `TuiGrpcClient` behind an `Arc<Mutex<>>` and uses
//! `tokio::task::block_in_place` + `runtime.block_on()` to bridge the gap.

use std::sync::Arc;

use reovim_arch::sync::Mutex;
use reovim_client_driver::{OptionValue, ServerHandle};

use crate::grpc_client::TuiGrpcClient;

/// Sync adapter wrapping `TuiGrpcClient` for `ServerHandle` trait.
///
/// Shared between `TuiApp` and client modules via `Arc`. The `Mutex`
/// serializes access to the gRPC client.
pub struct TuiServerHandle {
    client: Arc<Mutex<TuiGrpcClient>>,
    runtime: tokio::runtime::Handle,
}

impl TuiServerHandle {
    /// Create a new server handle adapter.
    #[must_use]
    pub const fn new(client: Arc<Mutex<TuiGrpcClient>>, runtime: tokio::runtime::Handle) -> Self {
        Self { client, runtime }
    }
}

impl ServerHandle for TuiServerHandle {
    #[allow(clippy::result_large_err)]
    fn get_options(&self, names: &[&str]) -> Vec<(String, OptionValue)> {
        let names_owned: Vec<String> = names.iter().map(|s| (*s).to_string()).collect();
        let mut client = self.client.lock();
        let result = tokio::task::block_in_place(|| {
            self.runtime.block_on(client.get_options(names_owned))
        });
        match result {
            Ok(resp) => convert_options(&resp.options),
            Err(e) => {
                tracing::warn!("ServerHandle::get_options failed: {e}");
                Vec::new()
            }
        }
    }

    fn execute_command(&self, command: &str) {
        let keys = format!(":{command}<CR>");
        let mut client = self.client.lock();
        tokio::task::block_in_place(|| {
            let _ = self.runtime.block_on(client.send_keys(&keys));
        });
    }
}

/// Convert gRPC `OptionValue` map to client-driver `OptionValue` pairs.
fn convert_options(
    options: &std::collections::HashMap<String, reovim_protocol::v2::OptionValue>,
) -> Vec<(String, OptionValue)> {
    use reovim_protocol::v2::option_value::Value;

    options
        .iter()
        .filter_map(|(name, proto_val)| {
            let value = match proto_val.value.as_ref()? {
                Value::BoolValue(b) => OptionValue::Bool(*b),
                Value::IntValue(i) => OptionValue::Integer(*i),
                Value::StringValue(s) => OptionValue::String(s.clone()),
            };
            Some((name.clone(), value))
        })
        .collect()
}

#[cfg(test)]
#[path = "server_handle_tests.rs"]
mod tests;
