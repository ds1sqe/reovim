//! Runtime reovim-version helper.
//!
//! The workspace version carries a pre-release suffix (e.g.
//! `"0.15.0-dev"`). Cargo-style semver treats pre-release versions
//! as incompatible with bare major.minor constraints, so
//! `reovim-version = "^0.15"` in a manifest would not match
//! `0.15.0-dev`. For resolver purposes the stable triple is the
//! intended value — strip pre-release and build metadata.

use {
    anyhow::{Context, Result},
    semver::Version,
};

pub fn reovim_runtime_version() -> Result<Version> {
    let raw = Version::parse(env!("CARGO_PKG_VERSION"))
        .context("internal: workspace version is not valid semver")?;
    Ok(Version::new(raw.major, raw.minor, raw.patch))
}
