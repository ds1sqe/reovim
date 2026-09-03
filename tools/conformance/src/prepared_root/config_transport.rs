//! Bounded config transport shared by both hidden dispatch hops.

use super::{
    linux::CONFIG_ARG_BYTES,
    model::{Config, ConfigTransportInspection, Error},
    protocol,
};

pub(super) fn encode(
    config: &Config,
) -> Result<(Vec<u8>, ConfigTransportInspection), Error> {
    let bytes = protocol::encode_config(config)?;
    let inspection =
        inspect_length(bytes.len(), config.dispatch_bounds.config_bytes)?;
    Ok((bytes, inspection))
}

pub(super) fn inspect_length(
    actual: usize,
    configured_limit: usize,
) -> Result<ConfigTransportInspection, Error> {
    let limit = configured_limit.min(CONFIG_ARG_BYTES);
    let Some(headroom) = limit.checked_sub(actual) else {
        return Err(limit_error(actual, limit));
    };
    Ok(ConfigTransportInspection {
        encoded_bytes: actual,
        limit_bytes: limit,
        headroom_bytes: headroom,
    })
}

fn limit_error(actual: usize, limit: usize) -> Error {
    Error::new(
        "PREPARED_CONFIG_ENV_LIMIT",
        format!("config transport actual={actual} bytes, limit={limit} bytes"),
    )
}
