//! What a case over a Driver under `/bin` does: make the run, then
//! read the cell it answered with, the way a shell reads one.
//!
//! What stays beside the case is the record its own Driver takes.

use reovim_client::{Calls, Client, Error, Fault, Handle, Rd, Rights};
use reovim_sdk::{Encode, decode, encode, ops};

/// `call /bin/<name> run {..}`: the rd of the output cell.
///
/// # Errors
/// The fault the call answered with.
pub fn run<A: Encode>(
    client: &mut Client,
    name: &str,
    args: &A,
) -> Result<Rd, Fault> {
    let rd = client.resolve(&format!("/bin/{name}"))?;
    let out = client.call_by_name(rd, "run", encode(args))?;
    Ok(decode(&out)?)
}

/// The cell `rd` read at an offset to the empty read that ends it, and
/// closed, which is how a shell reads one.
///
/// # Panics
/// When the cell refuses a read, or holds bytes that are no text.
pub fn drain(client: &mut Client, rd: Rd) -> String {
    let h = client.open(rd, Rights::READ).unwrap();
    let mut bytes = Vec::new();
    loop {
        let at = bytes.len() as u64;
        let chunk = client.read(h, at, 64).unwrap();
        if chunk.is_empty() {
            break;
        }
        bytes.extend(chunk);
    }
    client.close(h).unwrap();
    String::from_utf8(bytes).unwrap()
}

/// The text one run's output cell holds, read to its end.
///
/// # Panics
/// When the run is refused, or as [`drain`].
pub fn text<A: Encode>(client: &mut Client, name: &str, args: &A) -> String {
    let cell = run(client, name, args).unwrap();
    drain(client, cell)
}

/// The cell read at `at`, as a shell reads a following one.
///
/// # Errors
/// The fault the read was sent back with.
///
/// # Panics
/// When the bytes read are no text.
pub fn next(client: &mut Client, h: Handle, at: u64) -> Result<String, Fault> {
    Ok(String::from_utf8(client.read(h, at, 64)?).unwrap())
}

/// Everything the cell holds from `at` on, read as a shell reads a
/// following one, to the `Busy` that says it has nothing more yet.
///
/// # Panics
/// When a read is refused for anything else, or answers nothing.
pub fn so_far(client: &mut Client, h: Handle, at: u64) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    loop {
        let read = client.read(h, at + bytes.len() as u64, 64);
        let Ok(chunk) = read else {
            assert_eq!(read, Err(Fault::Kernel(Error::Busy)));
            return String::from_utf8(bytes).unwrap();
        };
        assert!(!chunk.is_empty(), "a follower is never over");
        bytes.extend(chunk);
    }
}

/// `run` of the name `rd` with `args`: the rd of the output cell.
///
/// Over whatever `Calls` the caller holds, which is what a Driver
/// loaded at a Client is run through; the union is what lets a target
/// name the other side.
///
/// # Errors
/// What the call answered with.
pub fn ran<A: Encode>(ctx: &dyn Calls, rd: Rd, args: &A) -> Result<Rd, Error> {
    let out = ops::call_by_name(ctx, rd, "run", encode(args))?;
    decode(&out)
}

/// That run's output cell read whole, as text.
///
/// # Errors
/// As [`ran`], and what the read says.
///
/// # Panics
/// When the cell holds bytes that are no text.
pub fn said<A: Encode>(
    ctx: &dyn Calls,
    rd: Rd,
    args: &A,
) -> Result<String, Error> {
    let held = ops::bytes(ctx, ran(ctx, rd, args)?)?;
    Ok(String::from_utf8(held).unwrap())
}

/// The handles `/sys/core` says are watching `rd`.
///
/// # Panics
/// When `/sys/core` cannot be read.
pub fn watchers(client: &mut Client, rd: Rd) -> Vec<String> {
    let core = client.resolve("/sys/core").unwrap();
    let text = String::from_utf8(client.read_all(core).unwrap()).unwrap();
    let held = format!(" rd={} ", rd.0);
    text.lines()
        .filter(|line| line.starts_with("handle ") && line.contains(&held))
        .filter(|line| line.ends_with("watch=on"))
        .map(str::to_string)
        .collect()
}
