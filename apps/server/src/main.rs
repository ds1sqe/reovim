#[cfg(target_os = "linux")]
mod process;

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    process::run()
}

#[cfg(not(target_os = "linux"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err("the reference process requires the Linux Host adapter".into())
}
