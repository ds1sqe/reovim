# Reference assembly

```sh
cargo run --manifest-path product/Cargo.toml -p reovim-server --features std
# Reads source, creates a Buffer, changes its heading, prints the result.
cargo test --manifest-path product/Cargo.toml -p reovim-server --all-features
# Exercises Resource access, History and Linux native loading (requires cc).
```

Assembly selects Core and provider implementations. The client imports only
Interface; Runtime executes admitted work independently of its result Future.
The heading edit preserves the source's invalid UTF-8 suffix.

This is an embedded reference journey, not a network server or release-ready
SDK. Native Resource/model loading and caller ABI journeys are tested; full
module lifecycle and compatibility conformance remain incomplete. Rendering and user input belong to Clients.
