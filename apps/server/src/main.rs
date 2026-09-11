fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let address = args.next().unwrap_or_else(|| "127.0.0.1:7878".into());
    let config = reovim_server::boot::Config::parse(args)?;
    reovim_server::service::serve_configured(&address, config)?;
    Ok(())
}
