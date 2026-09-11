fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address =
        std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:7878".into());
    reovim_server::service::serve(&address)?;
    Ok(())
}
