use reovim_host_reference::run_with;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut service = reovim_server::assemble()?;
    let result = run_with(
        reovim_server::client::heading(&service.client),
        &mut service.runtime,
        &service.driver,
    )??;
    println!(
        "revision={} node={} source={:?}",
        result.state.revision, result.node, result.value.data
    );
    for event in service.runtime.events(0) {
        eprintln!("{event:?}");
    }
    (service.driver.close)(&mut service.runtime)?;
    Ok(())
}
